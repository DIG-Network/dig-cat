//! Greedy CAT coin selection — pure, largest-first, fail-closed on lineage.
//!
//! Selection is the first custody gate: it only ever hands back **spendable** coins of the **one**
//! requested asset. A coin without a lineage proof cannot be spent as a CAT, so it is excluded here
//! rather than failing deep inside the ring builder (INV: fail-closed lineage).

use crate::error::CatError;
use chia_protocol::Bytes32;
use chia_wallet_sdk::driver::Cat;

/// The maximum number of CAT input coins a single spend will combine. Keeps the spend size (and its
/// CLVM cost) bounded; a wallet that needs more should consolidate first.
pub const MAX_CAT_INPUTS: usize = 50;

/// Select CAT coins covering `amount` base units of `asset_id`, largest-first.
///
/// Only coins whose `info.asset_id == asset_id` **and** which carry a lineage proof are considered —
/// mixing asset ids in one ring is a value-creation bug, and a proofless coin is unspendable.
/// Returns the selected coins and the sum of their amounts (`>= amount`).
///
/// # Errors
/// - [`CatError::InsufficientFunds`] if the spendable coins of `asset_id` total less than `amount`.
/// - [`CatError::TooManyInputs`] if covering `amount` would need more than [`MAX_CAT_INPUTS`] coins.
pub fn select_cats(
    cats: &[Cat],
    asset_id: Bytes32,
    amount: u64,
) -> Result<(Vec<Cat>, u64), CatError> {
    // Only spendable coins of the requested asset are candidates (asset filter + lineage filter).
    let mut candidates: Vec<Cat> = cats
        .iter()
        .filter(|c| c.info.asset_id == asset_id && c.lineage_proof.is_some())
        .copied()
        .collect();
    // Largest-first keeps the input count — and thus the spend size — minimal.
    candidates.sort_by_key(|c| std::cmp::Reverse(c.coin.amount));

    let available: u64 = candidates.iter().map(|c| c.coin.amount).sum();
    if available < amount {
        return Err(CatError::InsufficientFunds {
            need: amount,
            have: available,
        });
    }

    let mut selected = Vec::new();
    let mut sum = 0u64;
    for cat in candidates {
        if sum >= amount {
            break;
        }
        selected.push(cat);
        sum += cat.coin.amount;
    }

    if selected.len() > MAX_CAT_INPUTS {
        return Err(CatError::TooManyInputs {
            needed: selected.len(),
            cap: MAX_CAT_INPUTS,
        });
    }

    Ok((selected, sum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_protocol::Coin;
    use chia_puzzle_types::LineageProof;
    use chia_wallet_sdk::driver::CatInfo;

    const ASSET: Bytes32 = Bytes32::new([0xABu8; 32]);
    const OTHER_ASSET: Bytes32 = Bytes32::new([0xCDu8; 32]);

    /// A synthetic spendable Cat of `ASSET` with the given amount and a (dummy) lineage proof.
    fn spendable(amount: u64, parent: u8) -> Cat {
        with_asset_and_proof(amount, parent, ASSET, true)
    }

    fn with_asset_and_proof(amount: u64, parent: u8, asset: Bytes32, has_proof: bool) -> Cat {
        let coin = Coin::new(
            Bytes32::from([parent; 32]),
            Bytes32::from([0xEEu8; 32]),
            amount,
        );
        let proof = has_proof.then_some(LineageProof {
            parent_parent_coin_info: Bytes32::from([parent; 32]),
            parent_inner_puzzle_hash: Bytes32::from([0x11u8; 32]),
            parent_amount: amount,
        });
        Cat::new(
            coin,
            proof,
            CatInfo::new(asset, None, Bytes32::from([0x11u8; 32])),
        )
    }

    #[test]
    fn selects_largest_first_and_stops_early() {
        let cats = vec![
            spendable(40_000, 1),
            spendable(70_000, 2),
            spendable(5_000, 3),
        ];
        let (sel, sum) = select_cats(&cats, ASSET, 100_000).unwrap();
        assert_eq!(sum, 110_000);
        assert_eq!(sel.len(), 2);
        assert_eq!(sel[0].coin.amount, 70_000);
        assert_eq!(sel[1].coin.amount, 40_000);
    }

    #[test]
    fn selects_exact_single_coin() {
        let cats = vec![spendable(100_000, 1)];
        let (sel, sum) = select_cats(&cats, ASSET, 100_000).unwrap();
        assert_eq!(sum, 100_000);
        assert_eq!(sel.len(), 1);
    }

    #[test]
    fn errors_when_insufficient() {
        let cats = vec![spendable(40_000, 1), spendable(30_000, 2)];
        let err = select_cats(&cats, ASSET, 100_000).unwrap_err();
        assert!(matches!(
            err,
            CatError::InsufficientFunds {
                need: 100_000,
                have: 70_000
            }
        ));
    }

    #[test]
    fn errors_on_empty() {
        let err = select_cats(&[], ASSET, 10_000).unwrap_err();
        assert!(matches!(
            err,
            CatError::InsufficientFunds {
                need: 10_000,
                have: 0
            }
        ));
    }

    #[test]
    fn excludes_other_assets() {
        let cats = vec![
            with_asset_and_proof(100_000, 1, OTHER_ASSET, true),
            spendable(60_000, 2),
        ];
        // Only the ASSET coin counts; the OTHER_ASSET coin is ignored.
        let err = select_cats(&cats, ASSET, 100_000).unwrap_err();
        assert!(matches!(
            err,
            CatError::InsufficientFunds { have: 60_000, .. }
        ));
    }

    #[test]
    fn excludes_coins_without_lineage_proof() {
        let cats = vec![with_asset_and_proof(100_000, 1, ASSET, false)];
        // The proofless coin is unspendable, so it does not count toward the balance.
        let err = select_cats(&cats, ASSET, 50_000).unwrap_err();
        assert!(matches!(err, CatError::InsufficientFunds { have: 0, .. }));
    }

    #[test]
    fn errors_when_too_many_inputs() {
        // 60 coins of 1 unit each, needing 55 → exceeds the 50-input cap.
        let cats: Vec<Cat> = (0..60).map(|i| spendable(1, i as u8)).collect();
        let err = select_cats(&cats, ASSET, 55).unwrap_err();
        assert!(matches!(
            err,
            CatError::TooManyInputs {
                needed: 55,
                cap: 50
            }
        ));
    }
}
