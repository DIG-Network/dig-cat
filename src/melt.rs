//! CAT melt — destroy supply by revealing and running the TAIL.
//!
//! Melting removes value from a CAT's supply. It is only possible when the CAT's TAIL can be re-run:
//! a multi-issuance (`EverythingWithSignature`) CAT, whose issuer can always authorize a run. A
//! single-issuance (`GenesisByCoinId`) CAT is unmeltable after genesis (its genesis coin is gone),
//! so [`build_cat_melt`] rejects it. The TAIL MUST curry to the coins' asset id or the melt is
//! unauthorized ([`CatError::TailMismatch`]). Negative subtotals are computed by [`Cat::spend_all`].

use crate::asset::multi_issuance_asset_id;
use crate::error::CatError;
use crate::selection::select_cats;
use crate::spend::{CatValuePlan, UnsignedCatSpend};
use crate::tail::{payment_conditions, CatPayment, TailKind};
use chia_protocol::Bytes32;
use chia_puzzle_types::cat::EverythingWithSignatureTailArgs;
use chia_puzzle_types::Memos;
use chia_wallet_sdk::driver::{
    Cat, CatSpend, SpendContext, SpendWithConditions, StandardLayer,
};
use chia_wallet_sdk::prelude::{NodePtr, PublicKey};
use chia_wallet_sdk::types::conditions::{CreateCoin, RunCatTail};
use chia_wallet_sdk::types::Conditions;

/// A request to melt (burn) `melt_amount` base units of a CAT.
#[derive(Debug, Clone)]
pub struct MeltCatRequest {
    /// The owner's spendable CAT coins (lineage proofs required).
    pub cats: Vec<Cat>,
    /// The public key controlling every input coin (its standard puzzle authorizes the spend).
    pub owner_pk: PublicKey,
    /// The asset id being melted. Must equal the TAIL's tree hash.
    pub asset_id: Bytes32,
    /// The TAIL to reveal and run. Only [`TailKind::MultiIssuance`] is meltable.
    pub tail: TailKind,
    /// Base units to destroy from the supply.
    pub melt_amount: u64,
    /// Value to keep (re-create) rather than melt.
    pub keep_payments: Vec<CatPayment>,
    /// Where change returns, hinted to this puzzle hash.
    pub change_p2_puzzle_hash: Bytes32,
}

/// Build the unsigned coin spends that melt `melt_amount` base units of the CAT.
///
/// Selected coins must cover `melt_amount` plus any `keep_payments`; the leftover returns as change.
/// The lead coin reveals the TAIL (via `RUN_CAT_TAIL`), which authorizes destroying value — for a
/// multi-issuance CAT this surfaces the issuer's `AGG_SIG` in [`crate::required_signatures`].
///
/// # Errors
/// - [`CatError::ZeroAmount`] if `melt_amount` is zero.
/// - [`CatError::TailMismatch`] if the TAIL does not curry to `asset_id`, or the CAT is
///   single-issuance (unmeltable).
/// - [`CatError::InsufficientFunds`] / [`CatError::TooManyInputs`] from selection.
pub fn build_cat_melt(req: MeltCatRequest) -> Result<UnsignedCatSpend, CatError> {
    if req.melt_amount == 0 {
        return Err(CatError::ZeroAmount);
    }

    // Only a multi-issuance TAIL is re-runnable; verify it curries to the coins' asset id.
    let issuer_pk = meltable_issuer(&req.tail, req.asset_id)?;

    let keep_total: u64 = req.keep_payments.iter().map(|p| p.amount).sum();
    let needed = keep_total + req.melt_amount;
    let (selected, sum) = select_cats(&req.cats, req.asset_id, needed)?;
    let change = sum - needed;

    let mut ctx = SpendContext::new();
    let standard = StandardLayer::new(req.owner_pk);

    // The lead coin keeps + changes value AND reveals the TAIL to authorize the melt.
    let tail = ctx.curry(EverythingWithSignatureTailArgs::new(issuer_pk))?;
    let mut lead_conditions = payment_conditions(&mut ctx, &req.keep_payments)?;
    if change > 0 {
        let change_memos: Memos<NodePtr> = ctx.memos(&vec![req.change_p2_puzzle_hash])?;
        lead_conditions =
            lead_conditions.with(CreateCoin::new(req.change_p2_puzzle_hash, change, change_memos));
    }
    lead_conditions = lead_conditions.with(RunCatTail::new(tail, NodePtr::NIL));

    let mut cat_spends = Vec::with_capacity(selected.len());
    for (index, cat) in selected.iter().enumerate() {
        let conditions = if index == 0 {
            lead_conditions.clone()
        } else {
            Conditions::new()
        };
        let inner = standard.spend_with_conditions(&mut ctx, conditions)?;
        cat_spends.push(CatSpend::new(*cat, inner));
    }
    let children = Cat::spend_all(&mut ctx, &cat_spends)?;

    Ok(UnsignedCatSpend {
        coin_spends: ctx.take(),
        children,
        plan: CatValuePlan::new(sum, keep_total, change),
    })
}

/// Verify the TAIL is meltable and curries to `asset_id`, returning the issuer key to run it.
fn meltable_issuer(tail: &TailKind, asset_id: Bytes32) -> Result<PublicKey, CatError> {
    match tail {
        TailKind::MultiIssuance { issuer_pk } => {
            let tail_asset = multi_issuance_asset_id(*issuer_pk);
            if tail_asset != asset_id {
                return Err(CatError::TailMismatch {
                    expected: asset_id,
                    got: tail_asset,
                });
            }
            Ok(*issuer_pk)
        }
        // A single-issuance CAT's genesis coin is spent at issuance, so its TAIL can never run
        // again — the CAT is permanently unmeltable.
        TailKind::SingleIssuance => Err(CatError::TailMismatch {
            expected: asset_id,
            got: Bytes32::default(),
        }),
    }
}
