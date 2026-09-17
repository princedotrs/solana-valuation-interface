//! Read both Pyth feeds for one symbol and publish both quotes.
//!
//! The pair is written in a single instruction on purpose. A fair value and a
//! market price observed at different moments cannot be subtracted honestly —
//! the premium you would compute is partly just the time between the two
//! reads. Writing both from one verified snapshot makes the comparison exact
//! by construction rather than by luck, and it means the `DEVIATION_HIGH` flag
//! on the two quotes can never disagree.

use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use svi_math::{deviation_bps, pow10};

use crate::constants::{flags, AUTHORITY_SEED, CONFIG_SEED};
use crate::cpi::{publish_quote, QuoteUpdate};
use crate::error::StockAdapterError;
use crate::pyth::{load_verified, VerifiedPrice};
use crate::state::AdapterConfig;

#[derive(Accounts)]
pub struct RefreshStock<'info> {
    /// Anyone. Pays for the transaction and gets nothing else: this account is
    /// never checked against anything, because the keeper must not be able to
    /// influence the values it publishes.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [CONFIG_SEED, config.symbol.as_ref()], bump = config.bump)]
    pub config: Account<'info, AdapterConfig>,

    /// CHECK: The underlying equity's Pyth price account. Verified in the
    /// handler against owner, verification level, feed id and age before a
    /// single field is trusted. See `pyth::load_verified`.
    pub equity_price: UncheckedAccount<'info>,

    /// CHECK: The tokenized stock's Pyth price account. Same verification.
    pub token_price: UncheckedAccount<'info>,

    /// CHECK: Passed through to svi-core, which validates it against its own
    /// descriptor PDA. Pinned here to the address recorded in config.
    #[account(address = config.fair_descriptor @ StockAdapterError::WrongFeedAccount)]
    pub fair_descriptor: UncheckedAccount<'info>,

    /// CHECK: As above; svi-core owns and validates this account.
    #[account(mut, address = config.fair_quote @ StockAdapterError::WrongFeedAccount)]
    pub fair_quote: UncheckedAccount<'info>,

    /// CHECK: As above.
    #[account(address = config.market_descriptor @ StockAdapterError::WrongFeedAccount)]
    pub market_descriptor: UncheckedAccount<'info>,

    /// CHECK: As above.
    #[account(mut, address = config.market_quote @ StockAdapterError::WrongFeedAccount)]
    pub market_quote: UncheckedAccount<'info>,

    /// CHECK: Signing PDA. Never read; only signed for.
    #[account(seeds = [AUTHORITY_SEED], bump = config.authority_bump)]
    pub adapter_authority: UncheckedAccount<'info>,

    /// CHECK: Address-pinned to the core program recorded in config.
    #[account(address = config.svi_core_program @ StockAdapterError::WrongCoreProgram)]
    pub svi_core_program: UncheckedAccount<'info>,
}

pub fn handle_refresh_stock(ctx: Context<RefreshStock>) -> Result<()> {
    let clock = Clock::get()?;
    let a = &ctx.accounts;
    let config = &a.config;

    // ---- Verify both inputs before trusting either -------------------------
    //
    // The two feeds get different age limits, and the difference is the whole
    // point. A token trades continuously, so a stale token feed means the
    // oracle is broken and we stop. An equity feed is *expected* to be hours
    // old overnight; refusing then would take the feed dark exactly when a
    // consumer most needs to be told the market is shut, so its limit is
    // generous and its age is reported through flags instead.
    let equity = load_verified(
        &a.equity_price.to_account_info(),
        &config.equity_feed_id,
        clock.unix_timestamp,
        config.reference_max_age_secs,
        config.max_conf_bps,
        config.min_verification_level,
    )?;
    let token = load_verified(
        &a.token_price.to_account_info(),
        &config.token_feed_id,
        clock.unix_timestamp,
        config.token_max_age_secs,
        config.max_conf_bps,
        config.min_verification_level,
    )?;

    // ---- What the pair says about itself -----------------------------------
    let status_flags = status_flags(&equity, &token, config)?;

    // One whole token of the tokenized stock. Publishing "one whole token is
    // worth N" keeps the conversion exact for a consumer holding any amount.
    let base_amount = pow10(u32::from(config.base_decimals))
        .ok_or(error!(StockAdapterError::MathOverflow))?;

    // ---- Provenance --------------------------------------------------------
    //
    // Binds each quote to the exact inputs that produced it: the two price
    // accounts, the two feed ids they were required to carry, and the config
    // whose thresholds decided the flags. A reader who fetches these can
    // reproduce the published bytes or prove they could not have come from
    // this state.
    let source_accounts_hash = hashv(&[
        a.equity_price.key().as_ref(),
        a.token_price.key().as_ref(),
        config.equity_feed_id.as_ref(),
        config.token_feed_id.as_ref(),
        config.key().as_ref(),
    ])
    .to_bytes();

    // `observed_slot` is when THIS PROGRAM read and verified the inputs, not
    // when Pyth observed the price. That is deliberate: it drives svi-core's
    // ordering and `valid_until_slot`, which are about the freshness of the
    // publication. Tying it to an overnight equity publish time would make
    // every fair-value quote born already expired, destroying the one case
    // this adapter exists for. How old the underlying data is, is reported by
    // MARKET_CLOSED / REFERENCE_STALE / TOKEN_FEED_STALE.
    let observed_slot = clock.slot;
    let observed_unix_ts = clock.unix_timestamp;

    let authority_seeds: &[&[u8]] = &[AUTHORITY_SEED, &[config.authority_bump]];

    // ---- Publish the reference fair value ----------------------------------
    let fair = QuoteUpdate {
        base_amount,
        quote_amount: equity.band.mid,
        lower_quote_amount: equity.band.lower,
        upper_quote_amount: equity.band.upper,
        observed_slot,
        observed_unix_ts,
        source_accounts_hash,
        status_flags,
    };
    publish_quote(
        &a.svi_core_program.to_account_info(),
        &a.fair_descriptor.to_account_info(),
        &a.fair_quote.to_account_info(),
        &a.adapter_authority.to_account_info(),
        authority_seeds,
        &fair,
    )?;

    // ---- Publish the token's own market price ------------------------------
    let market = QuoteUpdate {
        base_amount,
        quote_amount: token.band.mid,
        lower_quote_amount: token.band.lower,
        upper_quote_amount: token.band.upper,
        observed_slot,
        observed_unix_ts,
        source_accounts_hash,
        status_flags,
    };
    publish_quote(
        &a.svi_core_program.to_account_info(),
        &a.market_descriptor.to_account_info(),
        &a.market_quote.to_account_info(),
        &a.adapter_authority.to_account_info(),
        authority_seeds,
        &market,
    )?;

    msg!(
        "{}: fair {} market {} ({}bps) flags {:#x} | ref age {}s, token age {}s",
        config.symbol_str().unwrap_or("?"),
        equity.band.mid,
        token.band.mid,
        deviation_bps(token.band.mid, equity.band.mid).unwrap_or(u64::MAX),
        status_flags,
        equity.age_secs,
        token.age_secs,
    );
    Ok(())
}

/// Decide which states this pair is in.
///
/// Pure: every input is already verified, so this is testable on the host with
/// no accounts, no clock and no runtime. Every flag path below has a unit test.
fn status_flags(
    equity: &VerifiedPrice,
    token: &VerifiedPrice,
    config: &AdapterConfig,
) -> Result<u64> {
    let mut f = 0u64;

    // The equity oracle stops updating when the exchange is shut. There is no
    // "is the market open" bit in the price account to read, so this is
    // INFERRED from the age of the last update against a configured window.
    // It is a good inference — the feed updates constantly while open — but it
    // is an inference, and a publisher outage during market hours would look
    // the same. Documented as inferred in the methodology, not presented as
    // exchange-reported truth.
    if equity.older_than(config.market_closed_secs) {
        f |= flags::MARKET_CLOSED;
    }
    // Older than an ordinary closure explains. A long weekend should not trip
    // this; a publisher that stopped on Tuesday should.
    if equity.older_than(config.reference_stale_secs) {
        f |= flags::REFERENCE_STALE;
    }
    // Always abnormal: the token trades continuously.
    if token.older_than(config.token_stale_secs) {
        f |= flags::TOKEN_FEED_STALE;
    }

    // The gap between what the token trades at and what it tracks.
    let deviation = deviation_bps(token.band.mid, equity.band.mid)
        .ok_or(error!(StockAdapterError::MathOverflow))?;
    if deviation > config.max_deviation_bps {
        f |= flags::DEVIATION_HIGH;
    }

    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use svi_math::Band;

    /// Build a config with round thresholds, so a test can say what it means.
    fn cfg() -> AdapterConfig {
        AdapterConfig {
            authority: Pubkey::default(),
            svi_core_program: Pubkey::default(),
            fair_descriptor: Pubkey::default(),
            fair_quote: Pubkey::default(),
            market_descriptor: Pubkey::default(),
            market_quote: Pubkey::default(),
            equity_feed_id: [1u8; 32],
            token_feed_id: [2u8; 32],
            market_closed_secs: 900,           // 15 min
            reference_stale_secs: 345_600,     // 4 days
            reference_max_age_secs: 1_209_600, // 14 days
            token_stale_secs: 120,
            token_max_age_secs: 600,
            max_deviation_bps: 200, // 2%
            max_conf_bps: 500,
            methodology_hash: [0u8; 32],
            min_verification_level: crate::pyth::FULL_VERIFICATION,
            symbol: *b"AAPL\0\0\0\0",
            base_decimals: 8,
            authority_bump: 254,
            bump: 255,
        }
    }

    fn price(mid: u64, age_secs: u64) -> VerifiedPrice {
        VerifiedPrice {
            band: Band::new(mid, mid, mid).unwrap(),
            publish_time: 1_700_000_000,
            posted_slot: 1,
            age_secs,
            conf_bps: 0,
        }
    }

    #[test]
    fn open_market_in_line_sets_nothing() {
        let f = status_flags(&price(100_000, 10), &price(100_000, 5), &cfg()).unwrap();
        assert_eq!(f, 0);
    }

    /// The flag the product exists to raise: equity feed gone quiet overnight
    /// while the token keeps trading.
    #[test]
    fn overnight_sets_market_closed_only() {
        let f = status_flags(&price(100_000, 40_000), &price(100_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::MARKET_CLOSED);
    }

    /// A long weekend is still just a closed market — it must not also claim
    /// the reference is unusable, or the flag would fire every Sunday and mean
    /// nothing.
    #[test]
    fn long_weekend_does_not_set_reference_stale() {
        let three_days = 3 * 86_400;
        let f = status_flags(&price(100_000, three_days), &price(100_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::MARKET_CLOSED);
        assert_eq!(f & flags::REFERENCE_STALE, 0);
    }

    #[test]
    fn a_week_old_reference_sets_both() {
        let week = 7 * 86_400;
        let f = status_flags(&price(100_000, week), &price(100_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::MARKET_CLOSED | flags::REFERENCE_STALE);
    }

    #[test]
    fn stale_token_feed_is_flagged_on_its_own() {
        let f = status_flags(&price(100_000, 10), &price(100_000, 300), &cfg()).unwrap();
        assert_eq!(f, flags::TOKEN_FEED_STALE);
    }

    #[test]
    fn premium_over_tolerance_sets_deviation_high() {
        // 3% premium against a 2% tolerance.
        let f = status_flags(&price(100_000, 10), &price(103_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::DEVIATION_HIGH);
    }

    #[test]
    fn discount_over_tolerance_sets_deviation_high_too() {
        let f = status_flags(&price(100_000, 10), &price(97_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::DEVIATION_HIGH);
    }

    /// Exactly on the tolerance is not over it. Off-by-one here would make
    /// every quote at the boundary look alarming.
    #[test]
    fn deviation_exactly_at_tolerance_is_not_flagged() {
        let f = status_flags(&price(100_000, 10), &price(102_000, 5), &cfg()).unwrap();
        assert_eq!(f & flags::DEVIATION_HIGH, 0);
    }

    /// One basis point past it is.
    #[test]
    fn deviation_one_bp_over_tolerance_is_flagged() {
        let f = status_flags(&price(1_000_000, 10), &price(1_020_100, 5), &cfg()).unwrap();
        assert_eq!(f & flags::DEVIATION_HIGH, flags::DEVIATION_HIGH);
    }

    /// The realistic weekend state: market shut AND the token has drifted.
    /// Both must show, because a consumer seeing only one would draw the wrong
    /// conclusion about the other.
    #[test]
    fn weekend_drift_sets_closed_and_deviation() {
        let f = status_flags(&price(100_000, 40_000), &price(105_000, 5), &cfg()).unwrap();
        assert_eq!(f, flags::MARKET_CLOSED | flags::DEVIATION_HIGH);
    }

    #[test]
    fn every_flag_at_once_is_representable() {
        let f = status_flags(&price(100_000, 999_999), &price(150_000, 999), &cfg()).unwrap();
        assert_eq!(
            f,
            flags::MARKET_CLOSED
                | flags::REFERENCE_STALE
                | flags::TOKEN_FEED_STALE
                | flags::DEVIATION_HIGH
        );
        assert_eq!(f & !flags::ALL, 0, "set a bit outside this adapter's range");
    }

    /// A zero reference price cannot produce a deviation, and must fail rather
    /// than divide by zero or silently report no drift.
    #[test]
    fn zero_reference_price_is_an_error_not_a_zero_deviation() {
        assert!(status_flags(&price(0, 10), &price(100_000, 5), &cfg()).is_err());
    }

    /// A worthless token against a live reference is a 100% deviation, which
    /// is over any tolerance — it must flag, not overflow.
    #[test]
    fn worthless_token_flags_rather_than_overflowing() {
        let f = status_flags(&price(100_000, 10), &price(0, 5), &cfg()).unwrap();
        assert_eq!(f & flags::DEVIATION_HIGH, flags::DEVIATION_HIGH);
    }

    /// Nothing this adapter sets may collide with the protocol-NAV adapters'
    /// bits 0-5. The registry lives in svi-core; this checks our half of it.
    #[test]
    fn flags_stay_inside_the_allocated_range() {
        assert_eq!(flags::ALL & flags::ALLOCATED_RANGE, flags::ALL);
        assert_eq!(flags::ALL & 0xFFFF, 0);
    }

    #[test]
    fn symbol_round_trips_to_text() {
        assert_eq!(cfg().symbol_str(), Some("AAPL"));
        let mut c = cfg();
        c.symbol = *b"NVDA0000";
        assert_eq!(c.symbol_str(), Some("NVDA0000"));
    }
}
