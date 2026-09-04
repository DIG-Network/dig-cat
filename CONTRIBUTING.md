# Contributing to dig-cat

Thanks for your interest in improving dig-cat. This is a pure, key-free, network-free builder for
Chia Asset Tokens (CAT) — please read this before opening a PR.

## What is dig-cat?

dig-cat is the DIG Network canonical Chia CAT (Colored Coin, CHIP-0002) expert crate. It constructs
the exact `CoinSpend`s for every CAT operation (issuance, send, melt), decodes CATs from on-chain
spends (CHIP-0026), and reports the exact signatures a caller must produce. It never holds a secret
key, never signs, and never touches the network.

See [`README.md`](./README.md) and [`SPEC.md`](./SPEC.md) for full details.

## Reporting an issue

Found a bug or have a feature request? Open an issue on
[GitHub](https://github.com/DIG-Network/dig-cat/issues). Please include:

- The exact command or code that triggered the issue
- The error message or unexpected behavior
- Your Rust version (`rustc --version`)
- Any relevant output from running the command with `RUST_BACKTRACE=1`

An issue is most useful when it names a specific behavior and states what you expected instead.

## Prerequisites

- [Rust](https://rustup.rs), via the stable toolchain as specified by the CI workflow
  (`dtolnay/rust-toolchain@stable`). `rustup` picks it up automatically if you have a
  `rust-toolchain.toml` in the checkout.

## Build & test

```sh
# Build the crate
cargo build

# Run the full test suite
cargo test --all
```

## The gate (must pass before a PR is merged)

CI runs these on every PR (`.github/workflows/ci.yml`); run them locally first:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --all-features
cargo doc --no-deps --all-features
cargo llvm-cov nextest --all --retries 2 --fail-under-lines 80 --lcov
```

The coverage gate requires **≥80% lines** — `cargo test --all` counts toward this locally. All
features must lint and build cleanly (including the optional `dexie` feature) so features never rot.

## Commit conventions

Use clear, imperative commit subjects in Conventional Commits format: `type(scope): summary`.
Valid types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.
Keep one logical change per commit where practical.

Examples:
- `feat(send): add support for...`
- `fix(decode): handle...`
- `docs: update prerequisites`

End every commit with `Co-Authored-By: Claude <noreply@anthropic.com>` if the loop authored or
co-authored it; this is how authorship is tracked across sessions.

## Pull requests

1. Branch from `main`.
2. Make the gate green locally (run the commands above).
3. Commit and push your changes.
4. Open a PR with a clear description of what changed and why; reference any related issue.
5. Keep the diff focused — one logical change per PR where practical.
6. Expect a review before merge; address any feedback and re-run the gate if needed.

main is a protected branch — a PR is required before any merge.

**Version bump:** Increase the patch version in `Cargo.toml` (docs-only → patch; new feature → minor;
breaking change → major). Keep `Cargo.lock` in sync if it exists.
