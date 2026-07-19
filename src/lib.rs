//! # dig-cat — the DIG Network canonical Chia CAT (Colored Coin) expert crate
//!
//! `dig-cat` is a **pure, key-free, network-free** SpendBundle-builder for Chia Asset Tokens
//! (CATs, CHIP-0002). It constructs the exact [`chia_protocol::CoinSpend`]s for every CAT operation
//! — issuance, send, melt, combine, split — and reports the exact signatures a caller must produce.
//! It never holds a secret key, never signs, and never touches the network. The consumer signs the
//! reported messages, assembles the `SpendBundle`, and broadcasts.
//!
//! ## Invariants
//!
//! These four invariants hold across the entire crate and are the contract every unit is built to:
//!
//! - **INV-1 — No network.** dig-cat performs NO network or chain I/O. Every function is a pure
//!   transform of its inputs; the caller fetches coins and broadcasts bundles.
//! - **INV-2 — No keys.** dig-cat never accepts, holds, derives, or logs a secret key. It computes
//!   what must be signed; the caller's signer produces the signatures.
//! - **INV-3 — Unsigned output.** Every operation returns unsigned coin spends. Signatures are
//!   always the caller's responsibility.
//! - **INV-4 — SDK byte-source-of-truth.** Every CAT puzzle, layer, and coin-spend byte is produced
//!   by `chia-wallet-sdk` (pinned to the 0.30 / chia-protocol 0.26 family). dig-cat adds CAT-workflow
//!   ergonomics on top; it never re-implements a puzzle or hand-rolls a spend bundle.
//!
//! ## Status
//!
//! This is the **v0.0.0 bootstrap**. It establishes the crate + release pipeline and proves the
//! pinned chia-wallet-sdk 0.30 dependency tree resolves and compiles. The complete CAT builder
//! surface (issuance / send / melt / combine / split / coin-selection / lineage-proof
//! reconstruction / multi-TAIL safety) lands as **v0.1.0** in a single triple-gated feature PR
//! against this foundation. See `SPEC.md` for the normative contract.

use thiserror::Error;

// Re-exported here so the bootstrap references the pinned chia-protocol type surface the CAT builders
// will be built on (proving the dependency compiles). The full curated public surface lands in
// v0.1.0.
pub use chia_protocol::Bytes32;

/// The `dig-cat` error taxonomy (placeholder).
///
/// The complete [`CatError`] surface — driver/signer wrapping, CAT parse failures, fail-closed
/// lineage guards, and TAIL/melt authorization errors — lands with the CAT builders in v0.1.0.
#[derive(Debug, Error)]
pub enum CatError {
    /// The requested CAT operation is not yet implemented in this bootstrap.
    #[error("dig-cat operation not yet implemented (v0.0.0 bootstrap)")]
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_error_displays_the_bootstrap_message() {
        assert_eq!(
            CatError::NotImplemented.to_string(),
            "dig-cat operation not yet implemented (v0.0.0 bootstrap)"
        );
    }

    #[test]
    fn bytes32_reexport_is_the_pinned_chia_protocol_type() {
        // A default Bytes32 is 32 zero bytes — asserts the re-exported chia-protocol type resolves.
        let zero = Bytes32::default();
        assert_eq!(zero.to_bytes().len(), 32);
    }
}
