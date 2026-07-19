//! End-to-end CAT round-trips on the in-process Chia Simulator.
//!
//! These prove the unsigned spends dig-cat builds are real, broadcast-ready transactions: they
//! validate on the simulator once the caller signs the [`required_signatures`], and value routes
//! exactly where the plan says (custody invariants 1–4). The simulator validates against TESTNET11,
//! so signatures are requested for [`Network::Testnet11`].

use dig_cat::{
    build_cat_melt, build_cat_spend, build_cat_spend_with_inner, cat_puzzle_hash, decode_cat_spend,
    hydrate_cat, issue_cat, multi_issuance_asset_id, reconstruct_children, required_signatures,
    Cat, CatPayment, IssueCatRequest, MeltCatRequest, Network, RequiredSignature, SendCatRequest,
    TailKind,
};

use chia_protocol::Coin;
use chia_wallet_sdk::driver::{SpendContext, SpendWithConditions, StandardLayer};
use chia_wallet_sdk::test::{BlsPair, Simulator};
use chia_wallet_sdk::types::Conditions;

/// Total unspent base units of `asset_id` owned by `p2_puzzle_hash` on the simulator.
fn balance(
    sim: &Simulator,
    p2_puzzle_hash: chia_protocol::Bytes32,
    asset_id: chia_protocol::Bytes32,
) -> u64 {
    sim.unspent_coins(cat_puzzle_hash(p2_puzzle_hash, asset_id), false)
        .iter()
        .map(|c| c.amount)
        .sum()
}

/// Issue `amount` of a fresh CAT to `owner`, submit it, and return the (asset_id, minted Cat).
fn issue_to(
    sim: &mut Simulator,
    owner: &BlsPair,
    amount: u64,
    tail: TailKind,
) -> anyhow::Result<(chia_protocol::Bytes32, Cat)> {
    let funding = sim.new_coin(owner.puzzle_hash, amount);
    let result = issue_cat(IssueCatRequest {
        funding_coin: funding,
        funder_pk: owner.pk,
        amount,
        recipients: vec![CatPayment::new(owner.puzzle_hash, amount)],
        tail,
    })?;
    sim.spend_coins(result.unsigned.coin_spends, std::slice::from_ref(&owner.sk))?;
    Ok((result.asset_id, result.unsigned.children[0]))
}

#[test]
fn issue_single_then_send_with_change_conserves_value() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let alice = BlsPair::new(1);
    let bob = BlsPair::new(2);
    let minted = 100_000u64;

    let (asset_id, cat) = issue_to(&mut sim, &alice, minted, TailKind::SingleIssuance)?;
    assert_eq!(balance(&sim, alice.puzzle_hash, asset_id), minted);

    let send = build_cat_spend(SendCatRequest {
        cats: vec![cat],
        owner_pk: alice.pk,
        asset_id,
        payments: vec![CatPayment::new(bob.puzzle_hash, 30_000)],
        change_p2_puzzle_hash: alice.puzzle_hash,
    })?;
    assert_eq!(send.plan.delta, 0, "a send must conserve value");
    assert_eq!(send.plan.change, 70_000);
    assert_eq!(send.plan.outputs, 30_000);

    // The owner's key must be a required signer; signing exactly the reported sigs validates.
    let reqs = required_signatures(&send.coin_spends, &Network::Testnet11)?;
    assert!(has_bls_signer(&reqs, alice.pk), "owner must sign the send");
    sim.spend_coins(send.coin_spends, std::slice::from_ref(&alice.sk))?;

    // Value routed exactly: recipient 30k, owner change 70k, and nothing leaked elsewhere.
    assert_eq!(balance(&sim, bob.puzzle_hash, asset_id), 30_000);
    assert_eq!(balance(&sim, alice.puzzle_hash, asset_id), 70_000);
    Ok(())
}

#[test]
fn multi_input_ring_conserves_value() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let alice = BlsPair::new(3);
    let bob = BlsPair::new(4);

    // Two separate coins of the SAME asset: issue once, then split into two owner coins.
    let (asset_id, cat) = issue_to(&mut sim, &alice, 100_000, TailKind::SingleIssuance)?;
    let split = build_cat_spend(SendCatRequest {
        cats: vec![cat],
        owner_pk: alice.pk,
        asset_id,
        payments: vec![CatPayment::new(alice.puzzle_hash, 40_000)],
        change_p2_puzzle_hash: alice.puzzle_hash,
    })?;
    sim.spend_coins(split.coin_spends, std::slice::from_ref(&alice.sk))?;

    // Now alice has two coins (40k + 60k). Reconstruct them and send 90k in one ring.
    let coins = sim.unspent_coins(cat_puzzle_hash(alice.puzzle_hash, asset_id), false);
    assert_eq!(coins.len(), 2, "expected two owner coins after the split");
    let cats = hydrate_all(&sim, &coins, asset_id)?;

    let send = build_cat_spend(SendCatRequest {
        cats,
        owner_pk: alice.pk,
        asset_id,
        payments: vec![CatPayment::new(bob.puzzle_hash, 90_000)],
        change_p2_puzzle_hash: alice.puzzle_hash,
    })?;
    assert_eq!(send.plan.inputs, 100_000);
    assert_eq!(send.plan.delta, 0);
    sim.spend_coins(send.coin_spends, std::slice::from_ref(&alice.sk))?;

    assert_eq!(balance(&sim, bob.puzzle_hash, asset_id), 90_000);
    assert_eq!(balance(&sim, alice.puzzle_hash, asset_id), 10_000);
    Ok(())
}

#[test]
fn multi_issuance_melt_destroys_supply_with_issuer_signature() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let issuer = BlsPair::new(5);
    let minted = 100_000u64;

    let tail = TailKind::MultiIssuance {
        issuer_pk: issuer.pk,
    };
    let (asset_id, cat) = issue_to(&mut sim, &issuer, minted, tail.clone())?;
    assert_eq!(asset_id, multi_issuance_asset_id(issuer.pk));

    let melt = build_cat_melt(MeltCatRequest {
        cats: vec![cat],
        owner_pk: issuer.pk,
        asset_id,
        tail,
        melt_amount: 40_000,
        keep_payments: vec![CatPayment::new(issuer.puzzle_hash, 60_000)],
        change_p2_puzzle_hash: issuer.puzzle_hash,
    })?;
    assert_eq!(melt.plan.delta, -40_000, "melt destroys 40k of supply");

    // The TAIL run surfaces the issuer's AGG_SIG; it must be a required signer.
    let reqs = required_signatures(&melt.coin_spends, &Network::Testnet11)?;
    assert!(
        has_bls_signer(&reqs, issuer.pk),
        "issuer must authorize the melt"
    );
    sim.spend_coins(melt.coin_spends, std::slice::from_ref(&issuer.sk))?;

    // Supply dropped from 100k to 60k.
    assert_eq!(balance(&sim, issuer.puzzle_hash, asset_id), 60_000);
    Ok(())
}

#[test]
fn single_issuance_is_unmeltable() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let alice = BlsPair::new(6);
    let (asset_id, cat) = issue_to(&mut sim, &alice, 50_000, TailKind::SingleIssuance)?;

    let err = build_cat_melt(MeltCatRequest {
        cats: vec![cat],
        owner_pk: alice.pk,
        asset_id,
        tail: TailKind::SingleIssuance,
        melt_amount: 10_000,
        keep_payments: vec![],
        change_p2_puzzle_hash: alice.puzzle_hash,
    })
    .unwrap_err();
    assert!(
        matches!(err, dig_cat::CatError::TailMismatch { .. }),
        "single-issuance CATs cannot be melted, got {err:?}"
    );
    Ok(())
}

#[test]
fn lineage_reconstruction_matches_the_minted_child() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let alice = BlsPair::new(7);
    let (asset_id, cat) = issue_to(&mut sim, &alice, 25_000, TailKind::SingleIssuance)?;

    // The child's parent is the eve coin; its spend lets us recompute the child (CHIP-0026).
    let eve_spend = sim
        .coin_spend(cat.coin.parent_coin_info)
        .expect("eve spend");
    let children = reconstruct_children(&eve_spend)?;
    let recon = children
        .iter()
        .find(|c| c.coin.coin_id() == cat.coin.coin_id())
        .expect("child present");
    assert_eq!(recon.coin, cat.coin);
    assert_eq!(
        recon.lineage_proof, cat.lineage_proof,
        "lineage proof must match"
    );

    let hydrated = hydrate_cat(&eve_spend, cat.coin.coin_id(), asset_id)?;
    assert_eq!(hydrated.coin, cat.coin);

    // decode_cat_spend recognizes the eve spend as a CAT...
    let decoded = decode_cat_spend(
        eve_spend.coin,
        &eve_spend.puzzle_reveal,
        &eve_spend.solution,
    )?;
    assert_eq!(decoded.expect("is a CAT").cat.info.asset_id, asset_id);

    // ...but a plain standard coin spend is NOT a CAT.
    let funding_spend = sim
        .coin_spend(eve_spend.coin.parent_coin_info)
        .expect("funding spend");
    assert!(
        decode_cat_spend(
            funding_spend.coin,
            &funding_spend.puzzle_reveal,
            &funding_spend.solution
        )?
        .is_none(),
        "a standard coin is not a CAT"
    );
    Ok(())
}

#[test]
fn build_cat_spend_with_inner_sends_via_custom_p2() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let alice = BlsPair::new(8);
    let bob = BlsPair::new(9);
    let (asset_id, cat) = issue_to(&mut sim, &alice, 80_000, TailKind::SingleIssuance)?;

    // Build the inner p2 spend by hand (the escape-hatch path) and send the full 80k to bob.
    let mut ctx = SpendContext::new();
    let standard = StandardLayer::new(alice.pk);
    let hint = ctx.hint(bob.puzzle_hash)?;
    let conditions = Conditions::new().create_coin(bob.puzzle_hash, 80_000, hint);
    let inner = standard.spend_with_conditions(&mut ctx, conditions)?;

    let unsigned = build_cat_spend_with_inner(&mut ctx, &[(cat, inner)], asset_id)?;
    assert_eq!(unsigned.plan.inputs, 80_000);
    assert_eq!(unsigned.plan.delta, 0);
    sim.spend_coins(unsigned.coin_spends, std::slice::from_ref(&alice.sk))?;

    assert_eq!(balance(&sim, bob.puzzle_hash, asset_id), 80_000);
    Ok(())
}

// --- helpers ---

/// Whether `reqs` contains a BLS signature required from `public_key`.
fn has_bls_signer(
    reqs: &[RequiredSignature],
    public_key: chia_wallet_sdk::prelude::PublicKey,
) -> bool {
    reqs.iter().any(|r| match r {
        RequiredSignature::Bls(b) => b.public_key == public_key,
        RequiredSignature::Secp(_) => false,
    })
}

/// Hydrate each on-chain coin into a spendable [`Cat`] via its parent (eve/prior) spend.
fn hydrate_all(
    sim: &Simulator,
    coins: &[Coin],
    asset_id: chia_protocol::Bytes32,
) -> anyhow::Result<Vec<Cat>> {
    let mut cats = Vec::new();
    for coin in coins {
        let parent = sim
            .coin_spend(coin.parent_coin_info)
            .expect("parent spend present");
        cats.push(hydrate_cat(&parent, coin.coin_id(), asset_id)?);
    }
    Ok(cats)
}
