# dig-cat — Specification

Normative contract for the DIG Network canonical Chia CAT (Colored Coin, CHIP-0002) expert crate. An
independent reimplementation MUST satisfy every requirement below. Key words MUST / SHOULD / MUST NOT
are used in the RFC 2119 sense.

dig-cat is a pure builder library: it constructs the unsigned `CoinSpend`s for CAT operations and
reports what must be signed. It does not sign, hold keys, or touch the chain (except one optional,
feature-gated metadata lookup). Every on-chain byte is produced by `chia-wallet-sdk` 0.34
(`chia-protocol` 0.36) — the byte-source-of-truth.

## 1. Invariants

- **INV-1 — No network.** Every core function is a pure transform of its inputs. The ONLY network
  call in the crate is `resolve_metadata`, behind the default-OFF `dexie` feature.
- **INV-2 — No keys.** dig-cat MUST NOT accept, hold, derive, log, or serialize a secret key. Builders
  take public keys, coins, puzzle hashes, amounts, and parent spends only. No `SecretKey` and no
  signing call appears in non-test code.
- **INV-3 — Unsigned output.** Every builder returns unsigned `CoinSpend`s in an
  `UnsignedCatSpend`. The caller obtains the signatures via `required_signatures`, aggregates them,
  assembles the `SpendBundle`, and broadcasts.
- **INV-4 — SDK byte-source-of-truth.** Every CAT puzzle, layer, curry hash, and ring subtotal MUST
  come from `chia-wallet-sdk`. dig-cat MUST NOT re-implement a puzzle, hand-roll a spend, or
  hand-compute a TAIL/curry hash. Ring construction (announcements, `prev_subtotal`, `extra_delta`)
  MUST be delegated to `Cat::spend_all`.

## 2. Asset identity (`asset.rs`)

- The **asset id** of a CAT is the tree hash of its TAIL program.
  - `single_issuance_asset_id(genesis_coin_id)` = `GenesisByCoinIdTailArgs::curry_tree_hash`.
  - `multi_issuance_asset_id(issuer_pk)` = `EverythingWithSignatureTailArgs::curry_tree_hash`.
- The **outer puzzle hash** where a CAT coin lives is
  `cat_puzzle_hash(p2_puzzle_hash, asset_id) = CatArgs::curry_tree_hash(asset_id, p2_puzzle_hash)`.
- These functions are pure and deterministic; the asset id MUST change when the genesis coin / issuer
  key changes, and the outer puzzle hash MUST change when either the owner or the asset changes.

## 3. Issuance (`tail.rs`)

`issue_cat` mints a new CAT from a funding coin.

- `amount` MUST be > 0 (else `CatError::ZeroAmount`).
- The recipients' amounts MUST sum exactly to `amount` (else `CatError::AmountMismatch`).
- The funding coin MUST cover `amount` (else `CatError::InsufficientFunds`); any excess XCH is
  returned to the funder's standard puzzle as change.
- `TailKind::SingleIssuance` fixes the supply forever (`GenesisByCoinId`); `TailKind::MultiIssuance`
  lets the issuer key mint/melt later (`EverythingWithSignature`).
- The returned `asset_id` MUST equal the curried TAIL hash for the chosen kind. Each minted child is
  returned in `children` with its lineage proof.

## 4. Send (`send.rs`)

`build_cat_spend` moves CAT value conservatively.

- The payment total MUST be > 0 (else `CatError::ZeroAmount`).
- Coins are selected largest-first (§6). The lead coin carries all outputs (recipient coins +
  change); the remaining coins are spent with empty conditions and balanced through the ring by
  `Cat::spend_all`.
- Each recipient coin is a `CREATE_COIN` to the recipient's p2 puzzle hash, hinted to that puzzle
  hash (hint first) followed by any caller memos.
- Change (`selected_total − payments`) is re-created at `change_p2_puzzle_hash`, hinted.
- Value is conserved: `plan.delta == 0`, and `plan.inputs == plan.outputs + plan.change`.
- Every selected input MUST carry a lineage proof (enforced by selection — fail-closed).
- `build_cat_spend_with_inner` is the escape hatch for non-standard p2 puzzles: all inputs MUST share
  `asset_id` (else `CatError::TailMismatch`) and carry a lineage proof (else
  `CatError::MissingLineageProof`).

## 5. Melt (`melt.rs`)

`build_cat_melt` destroys supply by revealing the TAIL.

- `melt_amount` MUST be > 0 (else `CatError::ZeroAmount`).
- Only a multi-issuance CAT is meltable. The revealed TAIL MUST curry to the coins' `asset_id` (else
  `CatError::TailMismatch`). A single-issuance CAT is unmeltable (its genesis coin is spent at
  issuance) and MUST be rejected with `CatError::TailMismatch`.
- Selected coins MUST cover `keep_payments + melt_amount`; the leftover returns as change.
- The lead coin reveals the TAIL via `RUN_CAT_TAIL`; `Cat::spend_all` computes the negative
  `extra_delta` automatically. For a multi-issuance CAT the issuer's `AGG_SIG` MUST appear in
  `required_signatures`.
- `plan.delta == -(melt_amount)`.

## 6. Coin selection (`selection.rs`)

- `select_cats(cats, asset_id, amount)` considers ONLY coins whose `asset_id` matches AND which carry
  a lineage proof (fail-closed; mixing assets in a ring is a value-creation bug).
- Selection is largest-first (deterministic), stopping once the running sum ≥ `amount`.
- Errors: `CatError::InsufficientFunds { need, have }` if the spendable balance is short;
  `CatError::TooManyInputs { needed, cap }` if covering `amount` needs more than `MAX_CAT_INPUTS`
  (50) coins.

## 7. Lineage / CHIP-0026 decode (`lineage.rs`)

- `decode_cat_spend(coin, puzzle_reveal, solution)` returns the decoded CAT + inner puzzle/solution,
  or `Ok(None)` if the puzzle is not a CAT.
- `reconstruct_children(parent)` returns every child CAT a parent spend created (via
  `Cat::parse_children`), or `CatError::NotACat` if the parent is not a CAT.
- `hydrate_cat(parent, child_coin_id, asset_id)` returns the single spendable child matching both the
  coin id and asset id; `CatError::LineageMismatch` if none matches;
  `CatError::MissingLineageProof` if the match has no proof. All decoders are pure.

## 8. Signing boundary (`signing.rs`)

- `required_signatures(spends, network)` returns the `RequiredSignature`s (BLS/secp) the caller must
  produce for `spends` to validate on `network` (`Mainnet` / `Testnet11` / `Custom(constants)`).
  Aggregating the corresponding signatures yields the bundle signature. dig-cat performs no signing.

## 9. Metadata (`metadata.rs`, feature `dexie`)

- `CatMetadata` (asset id, name, code, decimals, logo_url, description) is always available as a data
  type; `decimals` defaults to 3 (the Chia CAT convention) when a registry omits it.
- `resolve_metadata(asset_id)` (feature `dexie`) performs exactly ONE HTTPS GET to a fixed host +
  fixed path with the 64-char hex asset id as the only dynamic segment (SSRF-safe: no free-text, no
  cross-host redirects, short timeout). It returns `Ok(None)` for an unknown asset (any non-success
  response) and `CatError::Dexie` only on a transport or JSON-parse failure. Metadata is display-only
  and never affects a spend.
- Registry note: the dexie public API exposes no by-id asset lookup at the time of writing; the
  resolver targets the fixed by-id path shape and its response parsing is contract-tested against a
  captured fixture. Consumers MUST treat metadata as best-effort (`Ok(None)` is a valid answer).

## 10. Error taxonomy (`error.rs`)

`CatError` variants: `Driver` (`#[from] DriverError`), `Signer` (`#[from] SignerError`), `Clvm`,
`InsufficientFunds { need, have }`, `TooManyInputs { needed, cap }`,
`AmountMismatch { expected, got }`, `MissingLineageProof(Bytes32)`,
`TailMismatch { expected, got }`, `LineageMismatch`, `NotACat`, `ZeroAmount`, and (feature `dexie`)
`Dexie(String)`.

## 11. Security properties & conformance

- **Conservation.** A send produces `plan.delta == 0` and validates on the simulator; a melt produces
  `plan.delta == -(melt_amount)`.
- **Melt authorization.** Value leaves the supply ONLY via a TAIL that curries to the coin's asset
  id; a mismatched TAIL is rejected; multi-issuance melt requires the issuer's `AGG_SIG`.
- **No untracked leakage.** `Σ payments + change == Σ selected inputs`; change goes to the caller's
  `change_p2_puzzle_hash`; no value routes to an unexpected puzzle hash.
- **Correct ring.** Subtotals and `extra_delta` are never hand-built — always `Cat::spend_all`. A ring
  spends exactly one asset id.
- **Key-free.** No `SecretKey` or signing call in non-test code.
- **Fail-closed lineage.** A coin without a lineage proof is never spent.
- **Byte conformance.** Every puzzle/spend byte is byte-identical to `chia-wallet-sdk` 0.34, and the
  builders are simulator-validated end to end.
