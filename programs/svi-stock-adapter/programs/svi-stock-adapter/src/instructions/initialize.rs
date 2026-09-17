//! Create one symbol's config, and the shared signing authority on first use.

use anchor_lang::prelude::*;

use crate::constants::{AUTHORITY_SEED, CONFIG_SEED, SYMBOL_LEN};
use crate::error::StockAdapterError;
use crate::state::AdapterConfig;

/// Everything the refresh instruction is allowed to rely on, set once by the
/// admin and readable on-chain by anyone who wants to check the rules.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitSymbolParams {
    /// Ticker, right-padded with zeros: `b"AAPL\0\0\0\0"`. Also the config's
    /// PDA seed, so it must be supplied here as well as in the accounts.
    pub symbol: [u8; SYMBOL_LEN],

    pub svi_core_program: Pubkey,
    pub fair_descriptor: Pubkey,
    pub fair_quote: Pubkey,
    pub market_descriptor: Pubkey,
    pub market_quote: Pubkey,

    pub equity_feed_id: [u8; 32],
    pub token_feed_id: [u8; 32],

    pub market_closed_secs: u64,
    pub reference_stale_secs: u64,
    pub reference_max_age_secs: u64,
    pub token_stale_secs: u64,
    pub token_max_age_secs: u64,

    pub max_deviation_bps: u64,
    pub max_conf_bps: u64,

    pub methodology_hash: [u8; 32],
    pub base_decimals: u8,
}

/// Largest decimals we will size a whole token with. `pow10` tops out at 19,
/// and a mint with more than 18 decimals does not exist in practice.
const MAX_BASE_DECIMALS: u8 = 18;

/// A tolerance above this is not a tolerance. 10_000 bps is 100%.
const MAX_BPS: u64 = 10_000;

impl InitSymbolParams {
    /// Reject a config that could not produce a meaningful quote, at creation
    /// time rather than on every refresh afterwards.
    fn validate(&self) -> Result<()> {
        // Symbol: non-empty, printable ASCII, zero-padded only at the end.
        let len = self
            .symbol
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(SYMBOL_LEN);
        require!(len > 0, StockAdapterError::InvalidSymbol);
        require!(
            self.symbol[..len].iter().all(|b| b.is_ascii_alphanumeric()),
            StockAdapterError::InvalidSymbol
        );
        require!(
            self.symbol[len..].iter().all(|b| *b == 0),
            StockAdapterError::InvalidSymbol
        );

        // The two feeds must be different, or "market vs reference" compares a
        // number to itself and the deviation is always zero.
        require!(
            self.equity_feed_id != self.token_feed_id,
            StockAdapterError::FeedPairNotDistinct
        );
        require_keys_neq!(
            self.fair_quote,
            self.market_quote,
            StockAdapterError::FeedPairNotDistinct
        );
        require_keys_neq!(
            self.fair_descriptor,
            self.market_descriptor,
            StockAdapterError::FeedPairNotDistinct
        );

        // Thresholds must widen: mention it, then doubt it, then refuse it.
        // Out of order, a feed could be refused before it was ever flagged,
        // and the flags would be unreachable.
        require!(
            self.market_closed_secs <= self.reference_stale_secs
                && self.reference_stale_secs <= self.reference_max_age_secs,
            StockAdapterError::InvalidThresholds
        );
        require!(
            self.token_stale_secs <= self.token_max_age_secs,
            StockAdapterError::InvalidThresholds
        );
        require!(
            self.reference_max_age_secs > 0 && self.token_max_age_secs > 0,
            StockAdapterError::InvalidThresholds
        );

        require!(
            self.max_deviation_bps > 0 && self.max_deviation_bps <= MAX_BPS,
            StockAdapterError::InvalidTolerance
        );
        require!(
            self.max_conf_bps > 0 && self.max_conf_bps <= MAX_BPS,
            StockAdapterError::InvalidTolerance
        );

        require!(
            self.base_decimals <= MAX_BASE_DECIMALS,
            StockAdapterError::InvalidTolerance
        );
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(p: InitSymbolParams)]
pub struct InitializeSymbol<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + AdapterConfig::INIT_SPACE,
        seeds = [CONFIG_SEED, p.symbol.as_ref()],
        bump
    )]
    pub config: Account<'info, AdapterConfig>,

    /// CHECK: The shared signing PDA. Never read or written — it exists only
    /// to be signed for. Validated by its seeds.
    #[account(seeds = [AUTHORITY_SEED], bump)]
    pub adapter_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_symbol(
    ctx: Context<InitializeSymbol>,
    p: InitSymbolParams,
) -> Result<()> {
    p.validate()?;

    let c = &mut ctx.accounts.config;
    c.authority = ctx.accounts.authority.key();
    c.svi_core_program = p.svi_core_program;
    c.fair_descriptor = p.fair_descriptor;
    c.fair_quote = p.fair_quote;
    c.market_descriptor = p.market_descriptor;
    c.market_quote = p.market_quote;
    c.equity_feed_id = p.equity_feed_id;
    c.token_feed_id = p.token_feed_id;
    c.market_closed_secs = p.market_closed_secs;
    c.reference_stale_secs = p.reference_stale_secs;
    c.reference_max_age_secs = p.reference_max_age_secs;
    c.token_stale_secs = p.token_stale_secs;
    c.token_max_age_secs = p.token_max_age_secs;
    c.max_deviation_bps = p.max_deviation_bps;
    c.max_conf_bps = p.max_conf_bps;
    c.methodology_hash = p.methodology_hash;
    c.symbol = p.symbol;
    c.base_decimals = p.base_decimals;
    c.authority_bump = ctx.bumps.adapter_authority;
    c.bump = ctx.bumps.config;

    msg!(
        "svi-stock-adapter: {} initialized, deviation<={}bps, conf<={}bps",
        c.symbol_str().unwrap_or("?"),
        c.max_deviation_bps,
        c.max_conf_bps
    );
    Ok(())
}
