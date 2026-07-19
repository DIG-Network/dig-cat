//! The key-free signing boundary (INV-2/INV-3).
//!
//! dig-cat never signs. Instead, [`required_signatures`] tells the caller exactly which public keys
//! must sign which messages for the unsigned coin spends to become a valid transaction. The caller's
//! own signer (holding the secret keys dig-cat never sees) produces the signatures and assembles the
//! `SpendBundle`.

use crate::error::CatError;
use chia_protocol::CoinSpend;
use chia_wallet_sdk::prelude::Allocator;
use chia_wallet_sdk::signer::{AggSigConstants, RequiredSignature};
use chia_wallet_sdk::types::{MAINNET_CONSTANTS, TESTNET11_CONSTANTS};

/// The consensus network a set of coin spends will be validated on. This only affects the
/// `AGG_SIG_*` additional data mixed into each signed message — it changes nothing about the spends
/// themselves.
#[derive(Debug, Clone)]
pub enum Network {
    /// Chia mainnet.
    Mainnet,
    /// The public testnet11.
    Testnet11,
    /// A custom network, identified by its aggregate-signature constants.
    Custom(AggSigConstants),
}

impl Network {
    /// The aggregate-signature constants used to derive the exact messages that must be signed.
    fn agg_sig_constants(&self) -> AggSigConstants {
        match self {
            Network::Mainnet => AggSigConstants::from(&*MAINNET_CONSTANTS),
            Network::Testnet11 => AggSigConstants::from(&*TESTNET11_CONSTANTS),
            Network::Custom(constants) => constants.clone(),
        }
    }
}

/// Compute the signatures the caller must produce for `spends` to be a valid transaction on
/// `network`.
///
/// Each returned [`RequiredSignature`] names a public key and the exact message that key must sign
/// (BLS `AGG_SIG_*` conditions surfaced by running the puzzles). Aggregating all of these — plus any
/// secp signatures — yields the bundle's signature. dig-cat performs no signing itself (INV-3).
pub fn required_signatures(
    spends: &[CoinSpend],
    network: &Network,
) -> Result<Vec<RequiredSignature>, CatError> {
    let mut allocator = Allocator::new();
    let constants = network.agg_sig_constants();
    RequiredSignature::from_coin_spends(&mut allocator, spends, &constants).map_err(CatError::Signer)
}
