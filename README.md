# dig-cat

The DIG Network canonical **Chia CAT (Colored Coin, CHIP-0002) expert crate**.

`dig-cat` is a **pure, key-free, network-free** builder for Chia Asset Tokens. It constructs the exact
`CoinSpend`s for every CAT operation — issuance, send, melt — decodes CATs from on-chain spends
(CHIP-0026), and reports the exact signatures a caller must produce. It **never holds a secret key,
never signs, and never touches the network**: the consumer signs the reported messages, assembles the
`SpendBundle`, and broadcasts. Every on-chain byte comes from
[`chia-wallet-sdk`](https://crates.io/crates/chia-wallet-sdk) 0.34.

## Custody guarantee (INV-1..4)

- **INV-1 — No network.** No chain I/O in the core; every function is a pure transform. The only
  network call is `resolve_metadata`, behind the default-OFF `dexie` feature.
- **INV-2 — No keys.** Never accepts, holds, derives, or logs a secret key — builds from public keys,
  coins, puzzle hashes, amounts, and parent spends only.
- **INV-3 — Unsigned output.** Every builder returns unsigned coin spends; `required_signatures`
  reports what to sign.
- **INV-4 — SDK byte-source-of-truth.** Every puzzle/curry/ring byte is produced by
  `chia-wallet-sdk`; dig-cat never hand-rolls a puzzle or subtotal.

See [`SPEC.md`](./SPEC.md) for the normative contract.

## Public API

| Area | Items |
|------|-------|
| Asset math | `single_issuance_asset_id`, `multi_issuance_asset_id`, `cat_puzzle_hash` |
| Issue | `issue_cat`, `IssueCatRequest`, `IssueCatResult`, `TailKind`, `CatPayment` |
| Send | `build_cat_spend`, `SendCatRequest`, `build_cat_spend_with_inner` |
| Melt | `build_cat_melt`, `MeltCatRequest` |
| Decode (CHIP-0026) | `decode_cat_spend`, `reconstruct_children`, `hydrate_cat`, `DecodedCat` |
| Selection | `select_cats`, `MAX_CAT_INPUTS` |
| Signing boundary | `required_signatures`, `Network`, `RequiredSignature` |
| Result types | `UnsignedCatSpend`, `CatValuePlan`, `CatError` |
| Metadata (`dexie`) | `CatMetadata`, `resolve_metadata` |

Re-exported chia types the API speaks in: `Cat`, `CatInfo`, `Puzzle`, `Spend`, `Bytes32`, `Coin`,
`CoinSpend`, `Program`, `LineageProof`, `PublicKey`, `AggSigConstants`.

## Example — build a send, then sign it yourself

```rust,ignore
use dig_cat::{build_cat_spend, required_signatures, CatPayment, Network, SendCatRequest};

let unsigned = build_cat_spend(SendCatRequest {
    cats,                          // owner's spendable Cats (lineage proofs required)
    owner_pk,                      // public key only — no secret key ever
    asset_id,
    payments: vec![CatPayment::new(recipient_p2, 30_000)],
    change_p2_puzzle_hash: owner_p2,
})?;
assert_eq!(unsigned.plan.delta, 0); // sends conserve value

// dig-cat does NOT sign. Ask it what to sign, then your signer produces the signatures.
let required = required_signatures(&unsigned.coin_spends, &Network::Mainnet)?;
// ... sign `required`, aggregate, build a SpendBundle, broadcast.
```

## Feature flags

- `dexie` (default OFF) — enables `resolve_metadata`, the crate's ONLY network call: a single,
  SSRF-safe HTTPS GET for off-chain CAT display metadata (name/ticker/decimals). Metadata never
  affects a spend.

## Testing

Pure/offline unit tests cover asset-id golden vectors, selection, and every guard branch; simulator
round-trips (`chia-sdk-test`) prove issue → send → melt validate on-chain and route value exactly.
Default-feature coverage is network-free and CI-gated ≥80% lines.

## License

Licensed under either of [Apache-2.0](./LICENSE-APACHE) or [MIT](./LICENSE-MIT) at your option.
