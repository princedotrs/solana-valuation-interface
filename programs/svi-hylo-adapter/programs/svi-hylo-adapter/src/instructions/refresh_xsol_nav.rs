//! Compute Hylo's xSOL NAV from verified account state and publish it.
//!
//! Implements §4 (computation), §5 (failure states) and §6 (output mapping) of
//! the `hylo-xsol-nav-v1` methodology spec. Every number here comes out of
//! `hylo-core`; this file verifies inputs, chooses which of Hylo's own numbers
//! to publish, and refuses to publish when anything is off.

use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use anchor_spl::token::Mint;
use hylo_core::exchange_context::{ExchangeContext, LstExchangeContext};
use hylo_core::rebalance::mode::RebalanceMode;
use hylo_idl::exchange::accounts::Hylo;
use hylo_idl::tokens::{TokenMint, XSOL};
use hylo_idl::{exchange, pda};

use crate::constants::{flags, CONFIG_SEED, QUOTE_DECIMALS, XSOL_BASE_AMOUNT};
use crate::cpi::{publish_quote, QuoteUpdate};
use crate::error::AdapterError;
use crate::idl_bridge;
use crate::state::AdapterConfig;

#[derive(Accounts)]
pub struct RefreshXsolNav<'info> {
    /// Anyone. Pays for the transaction and gets nothing else: this account is
    /// never checked against anything, because the keeper must not be able to
    /// influence the value it publishes.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AdapterConfig>,

    /// CHECK: Verified in the handler against owner and canonical PDA before a
    /// single byte is deserialized. See `verify_and_load`.
    pub hylo_state: UncheckedAccount<'info>,

    /// xSOL mint. Address-checked against the constant Hylo itself derives.
    #[account(address = XSOL::MINT @ AdapterError::WrongLevercoinMint)]
    pub xsol_mint: Account<'info, Mint>,

    /// CHECK: Must equal the feed Hylo has configured in its own state. We do
    /// not carry our own opinion about which oracle is correct — see handler.
    pub sol_usd_pyth: UncheckedAccount<'info>,

    /// CHECK: Passed through to svi-core, which validates it against its own
    /// descriptor PDA. Pinned here to the address recorded in our config.
    #[account(address = config.descriptor @ AdapterError::WrongCoreProgram)]
    pub descriptor: UncheckedAccount<'info>,

    /// CHECK: As above; svi-core owns and validates this account.
    #[account(mut, address = config.quote @ AdapterError::WrongCoreProgram)]
    pub quote: UncheckedAccount<'info>,

    /// CHECK: Signing PDA. Never read; only signed for.
    #[account(seeds = [crate::constants::AUTHORITY_SEED], bump = config.authority_bump)]
    pub adapter_authority: UncheckedAccount<'info>,

    /// CHECK: Address-pinned to the core program recorded in config.
    #[account(address = config.svi_core_program @ AdapterError::WrongCoreProgram)]
    pub svi_core_program: UncheckedAccount<'info>,
}

/// What we decided to publish, and why.
#[derive(Debug, PartialEq, Eq)]
struct Valuation {
    quote_amount: u64,
    lower: u64,
    upper: u64,
    status_flags: u64,
}

pub fn handle_refresh_xsol_nav(ctx: Context<RefreshXsolNav>) -> Result<()> {
    let clock = Clock::get()?;
    let accounts = &ctx.accounts;
    let config = &accounts.config;

    // ---- Verify identity before trusting any byte (spec §3) ----------------
    // A forged account stuffed with plausible numbers is the whole attack, and
    // it is defeated here rather than by anything downstream.
    require_keys_eq!(
        *accounts.hylo_state.owner,
        exchange::ID,
        AdapterError::HyloStateWrongOwner
    );
    require_keys_eq!(
        accounts.hylo_state.key(),
        pda::HYLO,
        AdapterError::HyloStateWrongAddress
    );

    let hylo: Hylo = {
        let data = accounts.hylo_state.try_borrow_data()?;
        Hylo::try_deserialize(&mut data.as_ref())?
    };

    // Hylo names its own oracle in its own state. Checking against that rather
    // than a feed ID we chose means we cannot disagree with the protocol about
    // which price is authoritative — by construction, not by diligence.
    require_keys_eq!(
        accounts.sol_usd_pyth.key(),
        hylo.sol_usd_oracle,
        AdapterError::OracleMismatch
    );

    // Hylo says it is not operating. Publishing a NAV that no one could act on
    // would be publishing a number that is true but not usable.
    require!(
        !hylo.protocol_paused && !hylo.lst_pair_paused,
        AdapterError::HyloPaused
    );

    // ---- Load Hylo's own view of itself (spec §4) --------------------------
    // `load` performs the two checks that matter and that we must NOT
    // reimplement: TotalSolCache epoch validation (§5.1) and the Pyth query
    // against Hylo's own OracleConfig tolerance (§5.3). Either failing is a
    // refusal to publish, not a degraded publish.
    let pyth_data = accounts.sol_usd_pyth.try_borrow_data()?;
    let sol_usd =
        pyth_solana_receiver_sdk::price_update::PriceUpdateV2::try_deserialize(
            &mut pyth_data.as_ref(),
        )?;

    let ctx_hylo: LstExchangeContext<Clock> = LstExchangeContext::load(
        clock.clone(),
        &idl_bridge::total_sol_cache(hylo.total_sol_cache),
        hylo.stablecoin_mint_threshold
            .try_into()
            .map_err(|_| error!(AdapterError::ContextUnavailable))?,
        hylo_core::pyth::OracleConfig::new(
            hylo.oracle_interval_secs,
            hylo.oracle_conf_tolerance
                .try_into()
                .map_err(|_| error!(AdapterError::ContextUnavailable))?,
        ),
        idl_bridge::levercoin_fees(hylo.levercoin_fees),
        &sol_usd,
        idl_bridge::virtual_stablecoin(hylo.virtual_stablecoin),
        Some(&accounts.xsol_mint),
        idl_bridge::rebalance_curve_config(hylo.lst_sell_curve_config),
        idl_bridge::rebalance_curve_config(hylo.lst_buy_curve_config),
    )
    .map_err(|_| error!(AdapterError::ContextUnavailable))?;

    let valuation = value_xsol(&ctx_hylo, config.max_band_bps)?;

    // ---- Provenance: bind the quote to the exact inputs that produced it ---
    let source_accounts_hash = hashv(&[
        accounts.hylo_state.key().as_ref(),
        accounts.xsol_mint.key().as_ref(),
        accounts.sol_usd_pyth.key().as_ref(),
        &config.sdk_revision,
    ])
    .to_bytes();

    let update = QuoteUpdate {
        base_amount: XSOL_BASE_AMOUNT,
        quote_amount: valuation.quote_amount,
        lower_quote_amount: valuation.lower,
        upper_quote_amount: valuation.upper,
        observed_slot: clock.slot,
        observed_unix_ts: clock.unix_timestamp,
        source_accounts_hash,
        status_flags: valuation.status_flags,
    };

    publish_quote(
        &accounts.svi_core_program.to_account_info(),
        &accounts.descriptor.to_account_info(),
        &accounts.quote.to_account_info(),
        &accounts.adapter_authority.to_account_info(),
        &[crate::constants::AUTHORITY_SEED, &[config.authority_bump]],
        &update,
    )?;

    msg!(
        "xSOL NAV published: {} (band {}..{}) flags={:#b} slot={}",
        valuation.quote_amount,
        valuation.lower,
        valuation.upper,
        valuation.status_flags,
        clock.slot
    );
    Ok(())
}

/// Status flags implied by the rebalance zone alone.
///
/// Hylo names the underwater zone `Depeg`; it carries no zone bit of its own
/// because [`depeg_valuation`] sets the louder `DESTABILIZED` instead.
fn zone_flags(mode: RebalanceMode) -> u64 {
    match mode {
        RebalanceMode::SellZone1 | RebalanceMode::SellZone2 => flags::SELL_ZONE,
        RebalanceMode::BuyZone1 | RebalanceMode::BuyZone2 => flags::BUY_ZONE,
        RebalanceMode::Neutral | RebalanceMode::Depeg => 0,
    }
}

/// The published value when Hylo is underwater (spec §5.2).
///
/// Hylo's own semantics say xSOL is worth nothing here and operations halt. A
/// consuming lender MUST read this as collateral value zero, not as a dip to
/// buy — which is why the value is exactly zero rather than merely small, and
/// why both halt flags are set.
fn depeg_valuation() -> Valuation {
    Valuation {
        quote_amount: 0,
        lower: 0,
        upper: 0,
        status_flags: flags::DESTABILIZED | flags::OPERATIONS_HALTED,
    }
}

/// The published value in any healthy zone (spec §§4, 6).
///
/// Takes raw `UFix64<N9>` bits rather than the context, so the decision can be
/// tested without a Solana runtime. `redeem` is the floor-math, lower-price
/// side and becomes the headline: it is what a holder liquidating xSOL
/// actually realises, and therefore the defensible collateral value.
fn healthy_valuation(
    zone: u64,
    redeem_bits: u64,
    mint_bits: u64,
    levercoin_supply_bits: u64,
    max_band_bps: u64,
) -> Result<Valuation> {
    // `UFix64<N9>` raw bits are USD-per-whole-xSOL at 9 decimals, which is
    // exactly `QUOTE_DECIMALS`. Publishing "one whole xSOL is worth N" makes
    // the conversion the identity — no rescale, no rounding, no rounding bug.
    const _: () = assert!(QUOTE_DECIMALS == 9);

    let mut status_flags = zone;

    // §4 edge case — zero supply. `hylo-core` returns exactly 1.0 here; flag it
    // so a consumer can tell a real $1 NAV from the empty-pool default.
    if levercoin_supply_bits == 0 {
        status_flags |= flags::ZERO_SUPPLY_DEFAULT;
    }

    require!(redeem_bits <= mint_bits, AdapterError::InvalidBounds);

    // A band wider than the feed's tolerance means the inputs are too
    // uncertain to be useful. Too uncertain must fail, not publish wide.
    if redeem_bits > 0 {
        let spread = mint_bits.saturating_sub(redeem_bits);
        let bps = (spread as u128)
            .checked_mul(10_000)
            .ok_or(AdapterError::MathOverflow)?
            .div_ceil(redeem_bits as u128);
        require!(bps <= max_band_bps as u128, AdapterError::BandTooWide);
    }

    Ok(Valuation {
        quote_amount: redeem_bits,
        lower: redeem_bits,
        upper: mint_bits,
        status_flags,
    })
}

/// Decide the published value from Hylo's context.
///
/// Thin: it lifts values out of the runtime and hands them to the pure
/// functions above, which is where the actual policy lives.
fn value_xsol(ctx: &LstExchangeContext<Clock>, max_band_bps: u64) -> Result<Valuation> {
    let mode = ctx.rebalance_mode();
    if matches!(mode, RebalanceMode::Depeg) {
        return Ok(depeg_valuation());
    }

    let redeem = ctx
        .levercoin_redeem_nav()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;
    let mint = ctx
        .levercoin_mint_nav()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;
    let supply = ctx
        .levercoin_supply()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;

    healthy_valuation(
        zone_flags(mode),
        redeem.bits,
        mint.bits,
        supply.bits,
        max_band_bps,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real values observed on mainnet at slot 445910996, recorded in
    /// docs/validation/2026-09-10-xsol-nav-mainnet.md. Using the actual
    /// numbers rather than invented ones means these tests would have caught
    /// a decimals or ordering mistake in the live path.
    const REDEEM: u64 = 61_326_271;
    const MINT: u64 = 61_368_725;
    const SUPPLY: u64 = 166_267_548_251_798;
    const BAND_LIMIT_BPS: u64 = 50;

    fn err_of(r: Result<Valuation>) -> u32 {
        match r.unwrap_err() {
            anchor_lang::error::Error::AnchorError(e) => e.error_code_number,
            other => panic!("expected an AnchorError, got {other:?}"),
        }
    }

    // ---- the healthy path, against real mainnet numbers -------------------

    #[test]
    fn publishes_redeem_side_as_the_headline() {
        let v = healthy_valuation(0, REDEEM, MINT, SUPPLY, BAND_LIMIT_BPS).unwrap();
        assert_eq!(
            v,
            Valuation {
                quote_amount: REDEEM,
                lower: REDEEM,
                upper: MINT,
                status_flags: 0,
            },
            "the headline value must be the redeem side — what a liquidating \
             holder actually realises"
        );
    }

    #[test]
    fn real_mainnet_band_is_within_tolerance() {
        // 6.92 bps at the observed slot. If this ever needs raising, the feed
        // got less certain and that is a risk decision, not a test fix.
        let bps = (MINT - REDEEM) as u128 * 10_000 / REDEEM as u128;
        assert!(bps < 10, "observed band was {bps} bps, expected single digits");
        assert!(healthy_valuation(0, REDEEM, MINT, SUPPLY, 7).is_ok());
    }

    // ---- zone flags ------------------------------------------------------

    #[test]
    fn zone_flags_map_to_hylos_own_zones() {
        assert_eq!(zone_flags(RebalanceMode::SellZone1), flags::SELL_ZONE);
        assert_eq!(zone_flags(RebalanceMode::SellZone2), flags::SELL_ZONE);
        assert_eq!(zone_flags(RebalanceMode::BuyZone1), flags::BUY_ZONE);
        assert_eq!(zone_flags(RebalanceMode::BuyZone2), flags::BUY_ZONE);
        assert_eq!(zone_flags(RebalanceMode::Neutral), 0);
    }

    #[test]
    fn zone_flags_survive_into_the_published_value() {
        let v = healthy_valuation(flags::SELL_ZONE, REDEEM, MINT, SUPPLY, BAND_LIMIT_BPS).unwrap();
        assert_eq!(v.status_flags, flags::SELL_ZONE);
    }

    // ---- the case that costs a lender money if it is wrong ---------------

    /// The most consequential behaviour in the program. If a lender reads
    /// DESTABILIZED as "cheap" instead of "worthless", someone gets a loan
    /// against collateral that cannot be redeemed.
    #[test]
    fn depeg_publishes_exactly_zero_and_says_so_loudly() {
        let v = depeg_valuation();
        assert_eq!(v.quote_amount, 0, "must be exactly zero, not merely small");
        assert_eq!(v.lower, 0);
        assert_eq!(v.upper, 0);
        assert_ne!(v.status_flags & flags::DESTABILIZED, 0);
        assert_ne!(
            v.status_flags & flags::OPERATIONS_HALTED,
            0,
            "a halted protocol must be visible to a consumer without \
             interpreting the value"
        );
    }

    // ---- refusals: fail stale, never fail wrong --------------------------

    #[test]
    fn refuses_incoherent_bounds() {
        // Cannot happen while hylo-core is correct. Assert it anyway: this is
        // the last line before a lender sees an impossible band.
        let r = healthy_valuation(0, MINT, REDEEM, SUPPLY, BAND_LIMIT_BPS);
        assert_eq!(err_of(r), AdapterError::InvalidBounds as u32 + 6000);
    }

    #[test]
    fn refuses_a_band_wider_than_tolerance() {
        // Same real numbers, tolerance dropped below the observed 6.92 bps.
        let r = healthy_valuation(0, REDEEM, MINT, SUPPLY, 5);
        assert_eq!(
            err_of(r),
            AdapterError::BandTooWide as u32 + 6000,
            "too uncertain must fail, not publish wide"
        );
    }

    #[test]
    fn band_exactly_at_tolerance_is_accepted() {
        // 100 bps exactly: boundary is inclusive, so an off-by-one here would
        // reject perfectly good quotes.
        let redeem = 1_000_000_000u64;
        let mint = redeem + redeem / 100;
        assert!(healthy_valuation(0, redeem, mint, SUPPLY, 100).is_ok());
        assert_eq!(
            err_of(healthy_valuation(0, redeem, mint, SUPPLY, 99)),
            AdapterError::BandTooWide as u32 + 6000
        );
    }

    // ---- edge cases inherited from hylo-core -----------------------------

    #[test]
    fn zero_supply_is_flagged_not_hidden() {
        // hylo-core returns exactly 1.0 for an empty pool. Without the flag a
        // consumer cannot tell that from a genuine $1 valuation.
        let one = 1_000_000_000u64;
        let v = healthy_valuation(0, one, one, 0, BAND_LIMIT_BPS).unwrap();
        assert_ne!(v.status_flags & flags::ZERO_SUPPLY_DEFAULT, 0);
        assert_eq!(v.quote_amount, one);
    }

    #[test]
    fn zero_valued_quote_does_not_divide_by_zero() {
        // Guards the bps computation, which divides by the lower bound.
        let v = healthy_valuation(0, 0, 0, SUPPLY, BAND_LIMIT_BPS).unwrap();
        assert_eq!(v.quote_amount, 0);
    }

    /// The widest band expressible: 1 unit against u64::MAX. In basis points
    /// that is ~1.8e23, which does not fit in a u64 — so the comparison has to
    /// happen in u128 or it wraps to a small number and publishes an absurd
    /// quote as if it were tight.
    ///
    /// Reaching this line at all means no overflow occurred: the crate builds
    /// with `overflow-checks = true`, so a wrap would panic rather than
    /// return. The assertion is that it is refused, not accepted.
    #[test]
    fn widest_possible_band_is_refused_without_wrapping() {
        let r = healthy_valuation(0, 1, u64::MAX, SUPPLY, u64::MAX);
        assert_eq!(
            err_of(r),
            AdapterError::BandTooWide as u32 + 6000,
            "a band too wide to express in u64 bps must be refused, not wrapped"
        );
    }
}
