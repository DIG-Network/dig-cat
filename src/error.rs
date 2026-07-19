//! The `dig-cat` error taxonomy.
//!
//! Every fallible operation in the crate returns [`CatError`]. Errors from the underlying
//! `chia-wallet-sdk` (the byte-source-of-truth, INV-4) are wrapped verbatim so the caller keeps the
//! precise SDK cause; the crate's own variants name the CAT-specific failure conditions the SPEC
//! defines (fail-closed lineage, TAIL/melt authorization, coin selection) so callers can branch on
//! them without string matching.

use chia_protocol::Bytes32;
use chia_wallet_sdk::driver::DriverError;
use chia_wallet_sdk::signer::SignerError;
use thiserror::Error;

/// The single error type for every `dig-cat` operation.
#[derive(Debug, Error)]
pub enum CatError {
    /// A puzzle/layer/spend construction error from the chia-wallet-sdk driver (INV-4).
    #[error("chia-wallet-sdk driver error: {0}")]
    Driver(#[from] DriverError),

    /// A required-signature calculation error from the chia-wallet-sdk signer.
    #[error("chia-wallet-sdk signer error: {0}")]
    Signer(#[from] SignerError),

    /// A CLVM (de)serialization/allocation failure while translating wire programs to node pointers.
    #[error("CLVM error: {0}")]
    Clvm(String),

    /// Coin selection could not cover the requested amount from the available CAT coins.
    #[error("insufficient CAT funds: need {need} base units, have {have}")]
    InsufficientFunds {
        /// Base units required.
        need: u64,
        /// Base units available across the candidate coins.
        have: u64,
    },

    /// Covering the amount would require more input coins than a single spend permits.
    #[error("too many CAT inputs: {needed} required, cap is {cap}")]
    TooManyInputs {
        /// Inputs the selection would need.
        needed: usize,
        /// The hard cap ([`crate::MAX_CAT_INPUTS`]).
        cap: usize,
    },

    /// The sum of requested outputs did not match the amount the caller declared.
    #[error("amount mismatch: expected {expected} base units, got {got}")]
    AmountMismatch {
        /// The amount the caller declared.
        expected: u64,
        /// The amount the outputs actually sum to.
        got: u64,
    },

    /// A CAT coin selected for spending had no lineage proof, so it cannot be spent (fail-closed).
    #[error("CAT coin {0} has no lineage proof and cannot be spent")]
    MissingLineageProof(Bytes32),

    /// The revealed TAIL program does not curry to the coin's asset id — melt is unauthorized.
    #[error("TAIL mismatch: expected asset id {expected}, TAIL curries to {got}")]
    TailMismatch {
        /// The asset id the coins carry.
        expected: Bytes32,
        /// The asset id the supplied TAIL actually produces.
        got: Bytes32,
    },

    /// A decoded child coin did not match the requested target (coin id / asset id).
    #[error("lineage mismatch: no child coin matched the requested target")]
    LineageMismatch,

    /// The parent spend being decoded is not a CAT.
    #[error("coin is not a CAT")]
    NotACat,

    /// An operation was asked to move zero base units.
    #[error("amount must be greater than zero")]
    ZeroAmount,

    /// A dexie metadata lookup failed at the transport or parse layer (feature `dexie`).
    #[cfg(feature = "dexie")]
    #[error("dexie metadata lookup failed: {0}")]
    Dexie(String),
}
