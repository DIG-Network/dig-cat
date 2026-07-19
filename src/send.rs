//! Conservative CAT sends — value in equals value out (no supply change).
//!
//! A send spends the owner's CAT coins and re-creates the same total value as recipient coins plus
//! change. All the ring bookkeeping (announcements, subtotals, per-coin `extra_delta`) is delegated
//! to [`Cat::spend_all`] (INV-4) — dig-cat never hand-builds a subtotal.

use crate::error::CatError;
use crate::selection::select_cats;
use crate::spend::{CatValuePlan, UnsignedCatSpend};
use crate::tail::{payment_conditions, CatPayment};
use chia_protocol::Bytes32;
use chia_puzzle_types::Memos;
use chia_wallet_sdk::driver::{
    Cat, CatSpend, Spend, SpendContext, SpendWithConditions, StandardLayer,
};
use chia_wallet_sdk::prelude::PublicKey;
use chia_wallet_sdk::types::conditions::CreateCoin;
use chia_wallet_sdk::types::Conditions;

/// A request to send CAT value to one or more recipients, returning change to the owner.
#[derive(Debug, Clone)]
pub struct SendCatRequest {
    /// The owner's spendable CAT coins (lineage proofs required; other assets are ignored).
    pub cats: Vec<Cat>,
    /// The public key controlling every input coin (its standard puzzle authorizes the spend).
    pub owner_pk: PublicKey,
    /// The asset id being sent. All spent coins must carry this asset id.
    pub asset_id: Bytes32,
    /// The recipient payments.
    pub payments: Vec<CatPayment>,
    /// Where change (selected total minus payments) returns, hinted to this puzzle hash.
    pub change_p2_puzzle_hash: Bytes32,
}

/// Build the unsigned coin spends that pay `payments` and return change to the owner.
///
/// Coins are selected largest-first to cover the payment total; the change (if any) is re-created at
/// `change_p2_puzzle_hash`. Value is conserved: `plan.delta == 0`.
///
/// # Errors
/// - [`CatError::ZeroAmount`] if the payments sum to zero.
/// - [`CatError::InsufficientFunds`] / [`CatError::TooManyInputs`] from selection.
pub fn build_cat_spend(req: SendCatRequest) -> Result<UnsignedCatSpend, CatError> {
    let total_out: u64 = req.payments.iter().map(|p| p.amount).sum();
    if total_out == 0 {
        return Err(CatError::ZeroAmount);
    }

    let (selected, sum) = select_cats(&req.cats, req.asset_id, total_out)?;
    let change = sum - total_out;

    let mut ctx = SpendContext::new();
    let standard = StandardLayer::new(req.owner_pk);

    // The lead coin carries every output (payments + change); the remaining coins are spent with an
    // empty condition set — their value flows through the ring, which `spend_all` balances.
    let mut lead_conditions = payment_conditions(&mut ctx, &req.payments)?;
    if change > 0 {
        let change_memos: Memos<clvmr::NodePtr> = ctx.memos(&vec![req.change_p2_puzzle_hash])?;
        lead_conditions =
            lead_conditions.with(CreateCoin::new(req.change_p2_puzzle_hash, change, change_memos));
    }

    let cat_spends = build_ring_spends(&mut ctx, &standard, &selected, lead_conditions)?;
    let children = Cat::spend_all(&mut ctx, &cat_spends)?;

    Ok(UnsignedCatSpend {
        coin_spends: ctx.take(),
        children,
        plan: CatValuePlan::new(sum, total_out, change),
    })
}

/// Build the ring of [`CatSpend`]s for `selected`, putting `lead_conditions` on the first coin and an
/// empty condition set on the rest, each authorized by the owner's standard layer.
fn build_ring_spends(
    ctx: &mut SpendContext,
    standard: &StandardLayer,
    selected: &[Cat],
    lead_conditions: Conditions,
) -> Result<Vec<CatSpend>, CatError> {
    let mut cat_spends = Vec::with_capacity(selected.len());
    for (index, cat) in selected.iter().enumerate() {
        let conditions = if index == 0 {
            lead_conditions.clone()
        } else {
            Conditions::new()
        };
        let inner = standard.spend_with_conditions(ctx, conditions)?;
        cat_spends.push(CatSpend::new(*cat, inner));
    }
    Ok(cat_spends)
}

/// Build an unsigned CAT ring from caller-supplied inner spends — the escape hatch for non-standard
/// p2 puzzles.
///
/// Each `(Cat, Spend)` pairs a spendable CAT with its already-constructed inner p2 spend (built in
/// `ctx`). All CATs MUST share `asset_id` and carry a lineage proof (fail-closed). The ring's
/// announcements/subtotals are computed by [`Cat::spend_all`] (INV-4). The returned plan reports the
/// total created value as `outputs`; `change` is not distinguishable here and is reported as `0`.
///
/// # Errors
/// - [`CatError::ZeroAmount`] if `cats` is empty.
/// - [`CatError::TailMismatch`] if a coin's asset id differs from `asset_id`.
/// - [`CatError::MissingLineageProof`] if a coin has no lineage proof.
pub fn build_cat_spend_with_inner(
    ctx: &mut SpendContext,
    cats: &[(Cat, Spend)],
    asset_id: Bytes32,
) -> Result<UnsignedCatSpend, CatError> {
    if cats.is_empty() {
        return Err(CatError::ZeroAmount);
    }
    for (cat, _) in cats {
        if cat.info.asset_id != asset_id {
            return Err(CatError::TailMismatch {
                expected: asset_id,
                got: cat.info.asset_id,
            });
        }
        if cat.lineage_proof.is_none() {
            return Err(CatError::MissingLineageProof(cat.coin.coin_id()));
        }
    }

    let inputs: u64 = cats.iter().map(|(cat, _)| cat.coin.amount).sum();
    let cat_spends: Vec<CatSpend> = cats
        .iter()
        .map(|(cat, spend)| CatSpend::new(*cat, *spend))
        .collect();
    let children = Cat::spend_all(ctx, &cat_spends)?;
    let created: u64 = children.iter().map(|c| c.coin.amount).sum();

    Ok(UnsignedCatSpend {
        coin_spends: ctx.take(),
        children,
        plan: CatValuePlan::new(inputs, created, 0),
    })
}
