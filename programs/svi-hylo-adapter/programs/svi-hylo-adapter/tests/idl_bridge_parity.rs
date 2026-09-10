//! Proves `idl_bridge` reproduces `hylo-core`'s own conversions exactly.
//!
//! The program build deliberately omits `hylo-core/offchain`, because that
//! feature drags `solana-sdk` and `solana-account-decoder` into an on-chain
//! binary. The four conversions it would have provided are reproduced in
//! `src/idl_bridge.rs` instead.
//!
//! Reproduced code drifts. This test enables `offchain` as a dev-dependency
//! only — resolver v2 keeps that out of the program build — so both
//! implementations exist here at once and can be compared directly. If Hylo
//! changes a field, this fails rather than the adapter silently mispricing.

use hylo_idl::exchange::types as idl;
use svi_hylo_adapter::idl_bridge;

fn ufix(bits: u64, exp: i8) -> idl::UFixValue64 {
    idl::UFixValue64 { bits, exp }
}

#[test]
fn total_sol_cache_matches_hylo_core() {
    // Deliberately not round numbers: a transposition between the epoch and
    // the amount would survive matching zeros.
    let v = idl::TotalSolCache {
        current_update_epoch: 1032,
        total_sol: ufix(247_488_123_456_789, -9),
    };
    let ours = idl_bridge::total_sol_cache(v.clone());
    let theirs: hylo_core::lst::total_sol_cache::TotalSolCache = v.into();

    assert_eq!(ours.current_update_epoch, theirs.current_update_epoch);
    assert_eq!(ours.total_sol.bits, theirs.total_sol.bits);
    assert_eq!(ours.total_sol.exp, theirs.total_sol.exp);
}

#[test]
fn virtual_stablecoin_matches_hylo_core() {
    let v = idl::VirtualStablecoin {
        supply: ufix(15_414_285_049_123, -6),
    };
    let ours = idl_bridge::virtual_stablecoin(v.clone());
    let theirs: hylo_core::virtual_stablecoin::VirtualStablecoin = v.into();

    assert_eq!(ours.supply.bits, theirs.supply.bits);
    assert_eq!(ours.supply.exp, theirs.supply.exp);
}

#[test]
fn rebalance_curve_config_matches_hylo_core() {
    // floor and ceil differ, so a swapped pair is visible.
    let v = idl::RebalanceCurveConfig {
        floor_pct: ufix(1_100_000_000, -9),
        ceil_pct: ufix(1_500_000_000, -9),
    };
    let ours = idl_bridge::rebalance_curve_config(v.clone());
    let theirs: hylo_core::rebalance::pricing::RebalanceCurveConfig = v.into();

    assert_eq!(ours.floor_pct.bits, theirs.floor_pct.bits);
    assert_eq!(ours.ceil_pct.bits, theirs.ceil_pct.bits);
}

#[test]
fn levercoin_fees_match_hylo_core() {
    // Six distinct values: any mint/redeem swap, or any zone transposition,
    // changes the result.
    let pair = |m: u64, r: u64| idl::FeePair {
        mint: ufix(m, -9),
        redeem: ufix(r, -9),
    };
    let v = idl::LevercoinFees {
        normal: pair(1_000_000, 2_000_000),
        sell_zone_1: pair(3_000_000, 4_000_000),
        sell_zone_2: pair(5_000_000, 6_000_000),
    };
    let ours = idl_bridge::levercoin_fees(v.clone());
    let theirs: hylo_core::fees::controller::LevercoinFees = v.into();

    // LevercoinFees derives PartialEq, so this compares every zone and both
    // sides of every pair structurally rather than through a formatter.
    assert!(ours == theirs, "levercoin fee conversion diverged from hylo-core");
}
