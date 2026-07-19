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
// `Custom` carries the full ~224-byte constants inline (the public API speaks in `AggSigConstants`,
// not a boxed handle). The type is `Copy` and networks are short-lived, so the size gap is fine.
#[allow(clippy::large_enum_variant)]
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
            Network::Custom(constants) => *constants,
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
    RequiredSignature::from_coin_spends(&mut allocator, spends, &constants)
        .map_err(CatError::Signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_spends_require_no_signatures() {
        for network in [Network::Mainnet, Network::Testnet11] {
            assert!(required_signatures(&[], &network).unwrap().is_empty());
        }
    }

    #[test]
    fn custom_network_uses_supplied_constants() {
        let constants = AggSigConstants::from(&*MAINNET_CONSTANTS);
        let network = Network::Custom(constants);
        // Mainnet and a Custom-with-mainnet-constants derive the same (empty) requirement set.
        assert_eq!(
            required_signatures(&[], &network).unwrap().len(),
            required_signatures(&[], &Network::Mainnet).unwrap().len()
        );
        // The custom constants match mainnet's agg_sig_me data.
        assert_eq!(
            constants.me(),
            AggSigConstants::from(&*MAINNET_CONSTANTS).me()
        );
    }
}
