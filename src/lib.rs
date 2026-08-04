//! # dig-cat — the DIG Network canonical Chia CAT (Colored Coin) expert crate
//!
//! `dig-cat` is a **pure, key-free, network-free** builder for Chia Asset Tokens (CATs, CHIP-0002).
//! It constructs the exact [`chia_protocol::CoinSpend`]s for every CAT operation — issuance, send,
//! melt — decodes CATs from on-chain spends (CHIP-0026), and reports the exact signatures a caller
//! must produce. It never holds a secret key, never signs, and never touches the network. The
//! consumer signs the reported messages, assembles the `SpendBundle`, and broadcasts.
//!
//! ## Invariants
//!
//! - **INV-1 — No network.** Core operations perform NO network or chain I/O; each is a pure
//!   transform of its inputs. The only network call is [`resolve_metadata`], behind the default-OFF
//!   `dexie` feature.
//! - **INV-2 — No keys.** dig-cat never accepts, holds, derives, or logs a secret key. It builds from
//!   public keys, coins, puzzle hashes, amounts, and parent spends only.
//! - **INV-3 — Unsigned output.** Every builder returns unsigned [`chia_protocol::CoinSpend`]s;
//!   [`required_signatures`] reports what the caller must sign.
//! - **INV-4 — SDK byte-source-of-truth.** Every CAT puzzle/layer/curry/spend byte comes from
//!   `chia-wallet-sdk` (0.34 / chia-protocol 0.36). dig-cat never re-implements a puzzle, hand-rolls a
//!   spend, or hand-computes a TAIL/curry hash; ring construction is delegated to [`Cat::spend_all`].
//!
//! ## Operations
//!
//! - [`issue_cat`] — mint a new CAT (single- or multi-issuance TAIL).
//! - [`build_cat_spend`] — send CAT value with change (value-conserving).
//! - [`build_cat_melt`] — destroy supply by revealing the TAIL (multi-issuance only).
//! - [`decode_cat_spend`] / [`reconstruct_children`] / [`hydrate_cat`] — CHIP-0026 decoding.
//! - [`select_cats`] — greedy, fail-closed coin selection.
//! - [`required_signatures`] — the key-free signing boundary.
//! - [`CatMetadata`] / [`resolve_metadata`] — off-chain display metadata (feature `dexie`).
//!
//! See `SPEC.md` for the normative contract.

mod asset;
mod error;
mod lineage;
mod melt;
mod metadata;
mod selection;
mod send;
mod signing;
mod spend;
mod tail;

// --- dig-cat public surface ---
pub use asset::{cat_puzzle_hash, multi_issuance_asset_id, single_issuance_asset_id};
pub use error::CatError;
pub use lineage::{decode_cat_spend, hydrate_cat, reconstruct_children, DecodedCat};
pub use melt::{build_cat_melt, MeltCatRequest};
#[cfg(feature = "dexie")]
pub use metadata::resolve_metadata;
pub use metadata::CatMetadata;
pub use selection::{select_cats, MAX_CAT_INPUTS};
pub use send::{build_cat_spend, build_cat_spend_with_inner, SendCatRequest};
pub use signing::{required_signatures, Network};
pub use spend::{CatValuePlan, UnsignedCatSpend};
pub use tail::{issue_cat, CatPayment, IssueCatRequest, IssueCatResult, TailKind};

// --- curated re-exports of the underlying chia types the API speaks in ---
pub use chia_protocol::{Bytes32, Coin, CoinSpend, Program};
pub use chia_puzzle_types::LineageProof;
pub use chia_wallet_sdk::driver::{Cat, CatInfo, Puzzle, Spend};
pub use chia_wallet_sdk::prelude::PublicKey;
pub use chia_wallet_sdk::signer::{AggSigConstants, RequiredSignature};
