//! SVI adapter for tokenized stocks.
//!
//! A tokenized stock such as AAPLx trades on Solana every hour of every day.
//! The share it represents trades for about six and a half hours on a weekday.
//! So for most of the week the on-chain price is the only price moving, and it
//! drifts from the value of the thing it tracks — overnight, over weekends,
//! over holidays — with nothing on-chain saying by how much, or even that the
//! underlying market is shut.
//!
//! This adapter publishes both numbers and the gap between them. For each
//! symbol it reads two Pyth feeds in a single transaction — the real equity
//! and the token — verifies each one is the feed this symbol is entitled to
//! use, and writes two `svi-core` quote accounts:
//!
//! | feed | value type | what it is |
//! |---|---|---|
//! | `stock-<sym>-fair-value-v1` | `ReferenceFairValue` | the equity's price |
//! | `stock-<sym>-market-v1` | `MarketSpot` | the token's own price |
//!
//! Both carry Pyth's confidence interval as their bounds, and both carry flags
//! for the states that make the pair hard to interpret: `MARKET_CLOSED`,
//! `REFERENCE_STALE`, `TOKEN_FEED_STALE`, `DEVIATION_HIGH`.
//!
//! Nobody is asked for anything. The price accounts are public, the math is
//! integer arithmetic in a crate with no dependencies, and anyone can re-run
//! it against the same accounts and get the same bytes.
//!
//! Design rule, applied without exception: **fail stale, never fail wrong.**
//! An input we cannot verify aborts the refresh with no write, and the
//! existing quotes age visibly past their `valid_until_slot`. A condition we
//! *can* verify but that makes the number hard to use — a closed market, a
//! wide premium — is a flag on a published quote, not a refusal.

pub mod constants;
pub mod cpi;
pub mod error;
pub mod instructions;
pub mod pyth;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3RmdZomoBXkWwGwvdjWK8ELYB4XeqHxTmrzedcmucHxq");

#[program]
pub mod svi_stock_adapter {
    use super::*;

    /// Create one symbol's config and, on first use, the shared signing
    /// authority. Admin-only.
    pub fn initialize_symbol(
        ctx: Context<InitializeSymbol>,
        p: InitSymbolParams,
    ) -> Result<()> {
        instructions::initialize::handle_initialize_symbol(ctx, p)
    }

    /// Recompute this symbol's fair value and market price and publish both.
    /// Permissionless: anyone may crank.
    ///
    /// The caller pays the fee and cannot influence either value. Both come
    /// from Pyth accounts this program verifies against the feed ids recorded
    /// in the symbol's config, and only this program's PDA can authorize the
    /// writes into `svi-core`.
    pub fn refresh_stock(ctx: Context<RefreshStock>) -> Result<()> {
        instructions::refresh_stock::handle_refresh_stock(ctx)
    }
}
