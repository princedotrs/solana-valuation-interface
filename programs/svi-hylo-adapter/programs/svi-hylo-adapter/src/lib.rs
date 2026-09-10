//! SVI adapter for Hylo's xSOL.
//!
//! Reads Hylo's public account state, computes xSOL NAV using Hylo's own
//! `hylo-core` (pinned to an exact revision, reimplementing nothing), and
//! publishes the result into a canonical `svi-core` quote account.
//!
//! Hylo is not asked for anything: account data is public bytes, and every
//! read in a transaction is same-slot consistent by construction. No CPI into
//! Hylo, no cooperation, no program changes on their side.
//!
//! Design rule, applied without exception: **fail stale, never fail wrong.**

pub mod constants;
pub mod cpi;
pub mod error;
pub mod idl_bridge;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr");

#[program]
pub mod svi_hylo_adapter {
    use super::*;

    /// Create the adapter config and its signing authority. Admin-only, once.
    pub fn initialize(ctx: Context<Initialize>, p: InitConfigParams) -> Result<()> {
        instructions::initialize::handle_initialize(ctx, p)
    }

    /// Recompute xSOL NAV and publish it. Permissionless: anyone may crank.
    ///
    /// The caller pays the fee and cannot influence the value — it is derived
    /// entirely from verified account state by this code, and only this
    /// program's PDA can authorize the write into `svi-core`.
    pub fn refresh_xsol_nav(ctx: Context<RefreshXsolNav>) -> Result<()> {
        instructions::refresh_xsol_nav::handle_refresh_xsol_nav(ctx)
    }
}
