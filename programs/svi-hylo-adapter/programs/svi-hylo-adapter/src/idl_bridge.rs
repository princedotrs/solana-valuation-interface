//! IDL account types → `hylo-core` computation types.
//!
//! # Why this exists rather than `hylo-core/offchain`
//!
//! `hylo-core` ships these conversions in its `idl_type_bridge` module, gated
//! behind its `offchain` feature. Enabling that feature also pulls in
//! `hylo-jupiter-amm-interface`, and with it `solana-sdk` and
//! `solana-account-decoder` — off-chain client machinery that has no business
//! being linked into a program that runs on-chain. It inflates the binary and
//! is unlikely to cross-compile for SBF at all.
//!
//! So the four conversions this adapter needs are reproduced here. They are
//! field-for-field copies with no arithmetic in them: the risk is
//! transcription, not correctness of math, and `tests/idl_bridge_parity.rs`
//! removes even that by asserting each one produces exactly what `hylo-core`'s
//! own implementation produces. That test enables `offchain` as a
//! **dev-dependency only**, which under resolver v2 keeps it out of the
//! program build entirely.
//!
//! `UFixValue64` is not reproduced: `hylo-idl` exposes those conversions
//! ungated, so they are used directly.

use hylo_core::fees::controller::{FeePair, LevercoinFees};
use hylo_core::lst::total_sol_cache::TotalSolCache;
use hylo_core::rebalance::pricing::RebalanceCurveConfig;
use hylo_core::virtual_stablecoin::VirtualStablecoin;
use hylo_idl::exchange::types as idl;

/// Mirrors `hylo_core::idl_type_bridge`'s impl for `FeePair`.
#[must_use]
pub fn fee_pair(v: idl::FeePair) -> FeePair {
    FeePair::new(v.mint.into(), v.redeem.into())
}

/// Mirrors `hylo_core::idl_type_bridge`'s impl for `LevercoinFees`.
#[must_use]
pub fn levercoin_fees(v: idl::LevercoinFees) -> LevercoinFees {
    LevercoinFees::new(
        fee_pair(v.normal),
        fee_pair(v.sell_zone_1),
        fee_pair(v.sell_zone_2),
    )
}

/// Mirrors `hylo_core::idl_type_bridge`'s impl for `TotalSolCache`.
///
/// `current_update_epoch` is the field `get_validated` compares against the
/// clock, so a mistake here would defeat the epoch-staleness guard rather than
/// merely mis-scale a number. Hence the parity test.
#[must_use]
pub fn total_sol_cache(v: idl::TotalSolCache) -> TotalSolCache {
    TotalSolCache {
        current_update_epoch: v.current_update_epoch,
        total_sol: v.total_sol.into(),
    }
}

/// Mirrors `hylo_core::idl_type_bridge`'s impl for `VirtualStablecoin`.
#[must_use]
pub fn virtual_stablecoin(v: idl::VirtualStablecoin) -> VirtualStablecoin {
    VirtualStablecoin {
        supply: v.supply.into(),
    }
}

/// Mirrors `hylo_core::idl_type_bridge`'s impl for `RebalanceCurveConfig`.
#[must_use]
pub fn rebalance_curve_config(v: idl::RebalanceCurveConfig) -> RebalanceCurveConfig {
    RebalanceCurveConfig::new(v.floor_pct.into(), v.ceil_pct.into())
}
