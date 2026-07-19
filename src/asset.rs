//! CAT asset-id and puzzle-hash math — pure, key-free, no allocator.
//!
//! A CAT's **asset id** is the tree hash of its TAIL program; the coin lives on-chain at the
//! **outer puzzle hash** that curries that asset id around the owner's inner (p2) puzzle hash. Every
//! hash here is produced by the `chia-puzzle-types` curry helpers (INV-4) so it is byte-identical to
//! what a full puzzle build would emit.

use chia_protocol::Bytes32;
use chia_puzzle_types::cat::{
    CatArgs, EverythingWithSignatureTailArgs, GenesisByCoinIdTailArgs,
};
use chia_wallet_sdk::prelude::{PublicKey, TreeHash};

/// The asset id of a **single-issuance** CAT (the `GenesisByCoinId` TAIL).
///
/// This TAIL fixes the total supply to the coins created when `genesis_coin_id` was spent: after
/// genesis the coin is gone, so no more of this CAT can ever be minted. The asset id is the tree
/// hash of the TAIL curried with the genesis coin id.
pub fn single_issuance_asset_id(genesis_coin_id: Bytes32) -> Bytes32 {
    GenesisByCoinIdTailArgs::curry_tree_hash(genesis_coin_id).into()
}

/// The asset id of a **multi-issuance** CAT (the `EverythingWithSignature` TAIL).
///
/// This TAIL lets the holder of `issuer_pk` mint or melt the CAT at will (each such spend requires
/// the issuer's signature). The asset id is the tree hash of the TAIL curried with the issuer's
/// public key.
pub fn multi_issuance_asset_id(issuer_pk: PublicKey) -> Bytes32 {
    EverythingWithSignatureTailArgs::curry_tree_hash(issuer_pk).into()
}

/// The outer (on-chain) puzzle hash of a CAT coin: the CAT layer curried with `asset_id` around the
/// owner's inner puzzle hash `p2_puzzle_hash`. This is the puzzle hash to query the chain for and to
/// send funds to for the owner to receive them as this CAT.
pub fn cat_puzzle_hash(p2_puzzle_hash: Bytes32, asset_id: Bytes32) -> Bytes32 {
    CatArgs::curry_tree_hash(asset_id, TreeHash::from(p2_puzzle_hash)).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_wallet_sdk::test::BlsPair;

    #[test]
    fn single_issuance_asset_id_matches_sdk_and_is_deterministic() {
        let genesis = Bytes32::from([7u8; 32]);
        let a = single_issuance_asset_id(genesis);
        let b = single_issuance_asset_id(genesis);
        assert_eq!(a, b, "asset id must be deterministic");
        // Golden: equals the SDK curry helper directly (INV-4 byte-source-of-truth).
        let sdk: Bytes32 = GenesisByCoinIdTailArgs::curry_tree_hash(genesis).into();
        assert_eq!(a, sdk);
    }

    #[test]
    fn single_issuance_asset_id_is_genesis_sensitive() {
        let a = single_issuance_asset_id(Bytes32::from([1u8; 32]));
        let b = single_issuance_asset_id(Bytes32::from([2u8; 32]));
        assert_ne!(a, b, "different genesis coins must yield different asset ids");
    }

    #[test]
    fn multi_issuance_asset_id_matches_sdk() {
        let pk = BlsPair::new(1).pk;
        let got = multi_issuance_asset_id(pk);
        let sdk: Bytes32 = EverythingWithSignatureTailArgs::curry_tree_hash(pk).into();
        assert_eq!(got, sdk);
    }

    #[test]
    fn cat_puzzle_hash_is_stable_and_asset_sensitive() {
        let p2 = Bytes32::from([0x11u8; 32]);
        let asset = Bytes32::from([0x42u8; 32]);
        let a = cat_puzzle_hash(p2, asset);
        assert_eq!(a, cat_puzzle_hash(p2, asset), "stable");
        assert_eq!(a.to_bytes().len(), 32);
        // Different asset id → different outer puzzle hash (asset id is curried in).
        let other = cat_puzzle_hash(p2, Bytes32::from([0x43u8; 32]));
        assert_ne!(a, other);
        // Different owner → different outer puzzle hash.
        let other_owner = cat_puzzle_hash(Bytes32::from([0x12u8; 32]), asset);
        assert_ne!(a, other_owner);
    }
}
