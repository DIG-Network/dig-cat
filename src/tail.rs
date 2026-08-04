//! CAT payments and issuance (TAIL selection).
//!
//! A CAT is defined by its **TAIL** (Token and Asset Issuance Limitation) program: the asset id is
//! the TAIL's tree hash, and issuance runs the TAIL once to mint the initial supply. dig-cat
//! supports the two canonical TAILs — single-issuance (`GenesisByCoinId`, fixed supply) and
//! multi-issuance (`EverythingWithSignature`, issuer-controlled) — via the SDK helpers (INV-4).

use crate::error::CatError;
use crate::spend::{CatValuePlan, UnsignedCatSpend};
use chia_protocol::{Bytes32, Coin};
use chia_puzzle_types::standard::StandardArgs;
use chia_puzzle_types::Memos;
use chia_wallet_sdk::driver::{Cat, SpendContext, StandardLayer};
use chia_wallet_sdk::prelude::PublicKey;
use chia_wallet_sdk::types::conditions::CreateCoin;
use chia_wallet_sdk::types::Conditions;

/// Which TAIL an issuance uses — the choice that fixes a CAT's supply policy.
#[derive(Debug, Clone)]
pub enum TailKind {
    /// `GenesisByCoinId`: the supply is minted once from the genesis (funding) coin and can never be
    /// increased or melted afterwards — a fixed-supply CAT.
    SingleIssuance,
    /// `EverythingWithSignature`: the holder of `issuer_pk` may mint or melt the CAT at will (each
    /// such spend requires the issuer's signature) — an issuer-controlled CAT.
    MultiIssuance {
        /// The public key authorized to run the TAIL (mint / melt).
        issuer_pk: PublicKey,
    },
}

/// A single CAT output: `amount` base units to the recipient's inner puzzle hash `p2_puzzle_hash`.
///
/// The recipient's puzzle hash is auto-prepended to `memos` as the coin **hint** so the receiving
/// wallet can discover the coin; any further `memos` follow the hint.
#[derive(Debug, Clone)]
pub struct CatPayment {
    /// The recipient's inner (p2) puzzle hash.
    pub p2_puzzle_hash: Bytes32,
    /// Base units to send to the recipient.
    pub amount: u64,
    /// Extra memos to attach after the auto-prepended recipient hint.
    pub memos: Vec<Bytes32>,
}

impl CatPayment {
    /// A payment of `amount` to `p2_puzzle_hash` with no extra memos.
    pub fn new(p2_puzzle_hash: Bytes32, amount: u64) -> Self {
        Self {
            p2_puzzle_hash,
            amount,
            memos: Vec::new(),
        }
    }
}

/// Build the `CREATE_COIN` conditions for a list of CAT payments, each hinted to its recipient.
pub(crate) fn payment_conditions(
    ctx: &mut SpendContext,
    payments: &[CatPayment],
) -> Result<Conditions, CatError> {
    let mut conditions = Conditions::new();
    for payment in payments {
        conditions = conditions.with(payment_create_coin(ctx, payment)?);
    }
    Ok(conditions)
}

/// Build a single hinted `CREATE_COIN` condition for one CAT payment.
pub(crate) fn payment_create_coin(
    ctx: &mut SpendContext,
    payment: &CatPayment,
) -> Result<CreateCoin<clvmr::NodePtr>, CatError> {
    let mut memo_items = Vec::with_capacity(1 + payment.memos.len());
    memo_items.push(payment.p2_puzzle_hash); // recipient hint first
    memo_items.extend_from_slice(&payment.memos);
    let memos: Memos<clvmr::NodePtr> = ctx.memos(&memo_items)?;
    Ok(CreateCoin::new(
        payment.p2_puzzle_hash,
        payment.amount,
        memos,
    ))
}

/// A request to issue (mint) a new CAT from a funding coin.
#[derive(Debug, Clone)]
pub struct IssueCatRequest {
    /// The XCH coin funding the mint; its full value backs the new CAT supply (1 mojo = 1 base unit).
    pub funding_coin: Coin,
    /// The public key controlling `funding_coin` (its standard puzzle) — used to spend it and to
    /// receive any XCH change.
    pub funder_pk: PublicKey,
    /// The total CAT supply to mint. MUST equal the sum of `recipients` amounts.
    pub amount: u64,
    /// Who receives the minted CAT (must sum to `amount`).
    pub recipients: Vec<CatPayment>,
    /// The TAIL that defines the CAT.
    pub tail: TailKind,
}

/// The result of a successful issuance: the new CAT's asset id and the unsigned spend.
#[derive(Debug, Clone)]
pub struct IssueCatResult {
    /// The asset id (TAIL tree hash) of the newly issued CAT.
    pub asset_id: Bytes32,
    /// The unsigned coin spends, minted child CATs, and value plan.
    pub unsigned: UnsignedCatSpend,
}

/// Issue (mint) a new CAT from `funding_coin`, distributing the supply to `recipients`.
///
/// The funding coin is spent (via its standard puzzle) to create the CAT's eve coin, which runs the
/// chosen TAIL to mint `amount` base units routed to `recipients`. Any XCH left over
/// (`funding_coin.amount - amount`) is returned to the funder's standard puzzle as change.
///
/// # Errors
/// - [`CatError::ZeroAmount`] if `amount` is zero.
/// - [`CatError::AmountMismatch`] if the recipients' amounts do not sum to `amount`.
/// - [`CatError::InsufficientFunds`] if `funding_coin` cannot cover `amount`.
pub fn issue_cat(req: IssueCatRequest) -> Result<IssueCatResult, CatError> {
    if req.amount == 0 {
        return Err(CatError::ZeroAmount);
    }
    let recipients_total: u64 = req.recipients.iter().map(|p| p.amount).sum();
    if recipients_total != req.amount {
        return Err(CatError::AmountMismatch {
            expected: req.amount,
            got: recipients_total,
        });
    }
    if req.funding_coin.amount < req.amount {
        return Err(CatError::InsufficientFunds {
            need: req.amount,
            have: req.funding_coin.amount,
        });
    }

    let mut ctx = SpendContext::new();

    // The eve coin's inner conditions mint the CAT to each recipient (hinted).
    let mint_conditions = payment_conditions(&mut ctx, &req.recipients)?;

    let parent_coin_id = req.funding_coin.coin_id();
    // chia-sdk-driver 0.34 renamed the issuance helpers (`issue_with_coin`/`issue_with_key` →
    // `single_issuance`/`multi_issuance`) and added a `hidden_puzzle_hash: Option<Bytes32>` argument
    // for the optional revocation layer. dig-cat issues plain, non-revocable CATs, so we pass `None`
    // — byte-identical to the 0.30 issuance (no revocation layer curried in).
    let (issue_conditions, children) = match &req.tail {
        TailKind::SingleIssuance => {
            Cat::single_issuance(&mut ctx, parent_coin_id, None, req.amount, mint_conditions)?
        }
        TailKind::MultiIssuance { issuer_pk } => Cat::multi_issuance(
            &mut ctx,
            parent_coin_id,
            *issuer_pk,
            None,
            req.amount,
            mint_conditions,
        )?,
    };

    // Return XCH change (if any) to the funder's standard puzzle, then spend the funding coin.
    let funder_puzzle_hash: Bytes32 = StandardArgs::curry_tree_hash(req.funder_pk).into();
    let xch_change = req.funding_coin.amount - req.amount;
    let mut funding_conditions = issue_conditions;
    if xch_change > 0 {
        funding_conditions =
            funding_conditions.with(CreateCoin::new(funder_puzzle_hash, xch_change, Memos::None));
    }
    StandardLayer::new(req.funder_pk).spend(&mut ctx, req.funding_coin, funding_conditions)?;

    let asset_id = children
        .first()
        .map(|c| c.info.asset_id)
        .ok_or(CatError::NotACat)?;

    let unsigned = UnsignedCatSpend {
        coin_spends: ctx.take(),
        children,
        plan: CatValuePlan::new(0, req.amount, 0),
    };
    Ok(IssueCatResult { asset_id, unsigned })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_wallet_sdk::test::BlsPair;

    fn request(amount: u64, recipients: Vec<CatPayment>, funding: u64) -> IssueCatRequest {
        let funder = BlsPair::new(1);
        IssueCatRequest {
            funding_coin: Coin::new(Bytes32::from([1u8; 32]), funder.puzzle_hash, funding),
            funder_pk: funder.pk,
            amount,
            recipients,
            tail: TailKind::SingleIssuance,
        }
    }

    #[test]
    fn zero_amount_is_rejected() {
        let err = issue_cat(request(0, vec![], 0)).unwrap_err();
        assert!(matches!(err, CatError::ZeroAmount));
    }

    #[test]
    fn recipients_must_sum_to_amount() {
        let recipients = vec![CatPayment::new(Bytes32::from([2u8; 32]), 40_000)];
        let err = issue_cat(request(100_000, recipients, 100_000)).unwrap_err();
        assert!(matches!(
            err,
            CatError::AmountMismatch {
                expected: 100_000,
                got: 40_000
            }
        ));
    }

    #[test]
    fn funding_must_cover_the_mint() {
        let recipients = vec![CatPayment::new(Bytes32::from([2u8; 32]), 100_000)];
        let err = issue_cat(request(100_000, recipients, 50_000)).unwrap_err();
        assert!(matches!(
            err,
            CatError::InsufficientFunds {
                need: 100_000,
                have: 50_000
            }
        ));
    }

    #[test]
    fn cat_payment_new_has_no_extra_memos() {
        let p = CatPayment::new(Bytes32::from([3u8; 32]), 5);
        assert!(p.memos.is_empty());
        assert_eq!(p.amount, 5);
    }
}
