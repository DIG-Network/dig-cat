//! CHIP-0026 CAT decoding — reconstruct spendable CATs from on-chain spends (pure).
//!
//! Given a parent coin's spend (puzzle reveal + solution), the SDK can recompute every child CAT it
//! created, complete with the lineage proof needed to spend it (INV-4 via [`Cat::parse_children`]).
//! This is how a wallet turns a hinted coin it found on-chain into something it can spend. All
//! functions here are pure decoders — no network, no keys.

use crate::error::CatError;
use chia_protocol::{Bytes32, CoinSpend, Program};
use chia_wallet_sdk::driver::{Cat, Puzzle};
use chia_wallet_sdk::prelude::{Allocator, NodePtr};
use clvm_traits::ToClvm;

/// A decoded CAT spend: the CAT itself plus the inner (p2) puzzle and solution revealed inside it.
#[derive(Debug, Clone)]
pub struct DecodedCat {
    /// The decoded CAT (with its lineage proof, if the solution carried one).
    pub cat: Cat,
    /// The inner p2 puzzle revealed by the CAT layer.
    pub inner_puzzle: Puzzle,
    /// The inner p2 puzzle's solution.
    pub inner_solution: NodePtr,
}

/// Decode a single CAT coin spend into its CAT + inner puzzle/solution.
///
/// Returns `Ok(None)` if `puzzle_reveal` is not a CAT (per the SDK's parse contract). Returns an
/// error only if the puzzle looked like a CAT but failed to parse.
pub fn decode_cat_spend(
    coin: chia_protocol::Coin,
    puzzle_reveal: &Program,
    solution: &Program,
) -> Result<Option<DecodedCat>, CatError> {
    let mut allocator = Allocator::new();
    let puzzle_ptr = alloc_program(&mut allocator, puzzle_reveal)?;
    let solution_ptr = alloc_program(&mut allocator, solution)?;
    let puzzle = Puzzle::parse(&allocator, puzzle_ptr);

    let Some((cat, inner_puzzle, inner_solution)) =
        Cat::parse(&allocator, coin, puzzle, solution_ptr)?
    else {
        return Ok(None);
    };
    Ok(Some(DecodedCat {
        cat,
        inner_puzzle,
        inner_solution,
    }))
}

/// Reconstruct every child CAT created by spending `parent`.
///
/// # Errors
/// [`CatError::NotACat`] if the parent spend is not a CAT.
pub fn reconstruct_children(parent: &CoinSpend) -> Result<Vec<Cat>, CatError> {
    let mut allocator = Allocator::new();
    let puzzle_ptr = alloc_program(&mut allocator, &parent.puzzle_reveal)?;
    let solution_ptr = alloc_program(&mut allocator, &parent.solution)?;
    let puzzle = Puzzle::parse(&allocator, puzzle_ptr);

    Cat::parse_children(&mut allocator, parent.coin, puzzle, solution_ptr)?.ok_or(CatError::NotACat)
}

/// Reconstruct the single spendable child CAT of `parent` matching `child_coin_id` and `asset_id`.
///
/// This is how a wallet hydrates a hinted coin it discovered on-chain into a spendable [`Cat`].
///
/// # Errors
/// - [`CatError::NotACat`] if `parent` is not a CAT.
/// - [`CatError::LineageMismatch`] if no child matches `child_coin_id` + `asset_id`.
/// - [`CatError::MissingLineageProof`] if the matched child has no lineage proof.
pub fn hydrate_cat(
    parent: &CoinSpend,
    child_coin_id: Bytes32,
    asset_id: Bytes32,
) -> Result<Cat, CatError> {
    let children = reconstruct_children(parent)?;
    let cat = children
        .into_iter()
        .find(|c| c.coin.coin_id() == child_coin_id && c.info.asset_id == asset_id)
        .ok_or(CatError::LineageMismatch)?;
    if cat.lineage_proof.is_none() {
        return Err(CatError::MissingLineageProof(child_coin_id));
    }
    Ok(cat)
}

/// Allocate a wire [`Program`] into `allocator`, mapping (de)serialization failures to [`CatError`].
fn alloc_program(allocator: &mut Allocator, program: &Program) -> Result<NodePtr, CatError> {
    program
        .to_clvm(allocator)
        .map_err(|e| CatError::Clvm(e.to_string()))
}
