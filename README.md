# dig-cat

The DIG Network canonical **Chia CAT (Colored Coin, CHIP-0002) expert crate**.

`dig-cat` is a **pure, key-free, network-free** SpendBundle-builder for Chia Asset Tokens. It
constructs the exact `CoinSpend`s for every CAT operation — issuance, send, melt, combine, split —
and reports the exact signatures a caller must produce. It **never holds a secret key, never signs,
and never touches the network**: the consumer signs the reported messages, assembles the
`SpendBundle`, and broadcasts.

## Security model (the load-bearing custody guarantee)

- **INV-1 — No network.** No network or chain I/O; every function is a pure transform of its inputs.
- **INV-2 — No keys.** Never accepts, holds, derives, or logs a secret key. It computes what must be
  signed; the caller's signer produces the signatures.
- **INV-3 — Unsigned output.** Every operation returns unsigned coin spends. Signatures are always
  the caller's responsibility.
- **INV-4 — SDK byte-source-of-truth.** Every CAT puzzle, layer, and coin-spend byte is produced by
  [`chia-wallet-sdk`](https://crates.io/crates/chia-wallet-sdk); dig-cat never hand-rolls a puzzle.

See [`SPEC.md`](./SPEC.md) for the normative contract.

## Status

**v0.0.0 bootstrap.** The complete CAT builder surface (issuance / send / melt / combine / split /
coin-selection / lineage-proof reconstruction / multi-TAIL safety) lands as **v0.1.0**.

## License

Licensed under either of [Apache-2.0](./LICENSE-APACHE) or [MIT](./LICENSE-MIT) at your option.
