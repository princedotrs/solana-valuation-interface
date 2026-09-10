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
        &hylo.total_sol_cache.into(),
        hylo.stablecoin_mint_threshold
            .try_into()
            .map_err(|_| error!(AdapterError::ContextUnavailable))?,
        hylo_core::pyth::OracleConfig::new(
            hylo.oracle_interval_secs,
            hylo.oracle_conf_tolerance
                .try_into()
                .map_err(|_| error!(AdapterError::ContextUnavailable))?,
        ),
        hylo.levercoin_fees.into(),
        &sol_usd,
        hylo.virtual_stablecoin.into(),
        Some(&accounts.xsol_mint),
        hylo.lst_sell_curve_config.into(),
        hylo.lst_buy_curve_config.into(),
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

/// Decide the published value from Hylo's context. Pure — no accounts, no
/// clock — so it can be tested directly.
fn value_xsol(
    ctx: &LstExchangeContext<Clock>,
    max_band_bps: u64,
) -> Result<Valuation> {
    let mode = ctx.rebalance_mode();

    let mut status_flags = match mode {
        RebalanceMode::SellZone1 | RebalanceMode::SellZone2 => flags::SELL_ZONE,
        RebalanceMode::BuyZone1 | RebalanceMode::BuyZone2 => flags::BUY_ZONE,
        _ => 0,
    };

    // §5.2 — Destabilized. Hylo's own semantics say xSOL is worth nothing and
    // operations halt. A consuming lender MUST read this as collateral value
    // zero, not as a dip to buy, which is why the flags are as loud as they
    // are and the value is exactly zero rather than merely small.
    // Hylo calls this zone `Depeg`: CR has fallen below the point where the
    // stablecoin is fully backed, so xSOL's claim on collateral is nil.
    if matches!(mode, RebalanceMode::Depeg) {
        return Ok(Valuation {
            quote_amount: 0,
            lower: 0,
            upper: 0,
            status_flags: status_flags | flags::DESTABILIZED | flags::OPERATIONS_HALTED,
        });
    }

    // §4 — the two sides of Hylo's own range. Redeem uses floor math against
    // Pyth's lower bound; mint uses ceil against the upper. We pick neither:
    // we publish both, and take redeem as the headline because that is what a
    // holder liquidating xSOL actually realises.
    let redeem = ctx
        .levercoin_redeem_nav()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;
    let mint = ctx
        .levercoin_mint_nav()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;

    // `UFix64<N9>` raw bits are USD-per-whole-xSOL at 9 decimals, which is
    // exactly `quote_decimals`. Publishing "one whole xSOL is worth N" makes
    // the conversion the identity — no rescale, no rounding, no rounding bug.
    let (lower, upper) = (redeem.bits, mint.bits);
    const _: () = assert!(QUOTE_DECIMALS == 9);

    // §4 edge case — zero supply. `hylo-core` returns exactly 1.0 here; flag it
    // so a consumer can tell a real $1 NAV from the empty-pool default.
    let supply = ctx
        .levercoin_supply()
        .map_err(|_| error!(AdapterError::NavUnavailable))?;
    if supply.bits == 0 {
        status_flags |= flags::ZERO_SUPPLY_DEFAULT;
    }

    require!(lower <= upper, AdapterError::InvalidBounds);

    // A band wider than the feed's tolerance means the inputs are too
    // uncertain to be useful. Too uncertain must fail, not publish wide.
    if lower > 0 {
        let spread = upper.saturating_sub(lower);
        let bps = (spread as u128)
            .checked_mul(10_000)
            .ok_or(AdapterError::MathOverflow)?
            .div_ceil(lower as u128);
        require!(bps <= max_band_bps as u128, AdapterError::BandTooWide);
    }

    Ok(Valuation {
        quote_amount: lower,
        lower,
        upper,
        status_flags,
    })
}
