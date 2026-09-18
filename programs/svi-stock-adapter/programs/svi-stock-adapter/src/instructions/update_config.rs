//! Change a live symbol's thresholds. Admin-only.
//!
//! What is mutable here is deliberately narrow. The thresholds and tolerances
//! decide *when* a quote is flagged or refused, and getting one wrong is an
//! ordinary operational mistake: a tolerance too tight flags every quote, one
//! too loose flags none. Those deserve a fix that costs a transaction.
//!
//! What is **not** mutable is everything that says which asset this symbol is:
//! the two Pyth feed ids, the two quote accounts, the two descriptors and the
//! core program. A symbol whose feed id could be changed by its authority is a
//! symbol whose published value can be repointed at a different asset without
//! any consumer seeing a change -- which is precisely the substitution
//! `load_verified` refuses to allow a keeper to perform. An admin key is a
//! smaller set of people than "anyone", not a different kind of trust, so the
//! identity of a feed is fixed at creation and a genuinely different asset
//! needs a genuinely different symbol.
//!
//! `min_verification_level` is mutable, and is the reason this instruction
//! exists. Whether Pyth sponsors a price account for a feed differs per cluster
//! and changes over time; a deployment that has to post its own prices needs to
//! accept partially verified updates, and that fact is not always known before
//! the symbol is created. Without this, discovering it afterwards means
//! redeploying the whole adapter under a new program id, because the config PDA
//! is seeded by the ticker and cannot be re-initialised.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::state::AdapterConfig;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateSymbolParams {
    /// Ticker, zero-padded. Also the config's PDA seed, so it is supplied here
    /// as well as in the accounts.
    pub symbol: [u8; SYMBOL_LEN],

    pub market_closed_secs: u64,
    pub reference_stale_secs: u64,
    pub reference_max_age_secs: u64,
    pub token_stale_secs: u64,
    pub token_max_age_secs: u64,

    pub max_deviation_bps: u64,
    pub max_conf_bps: u64,

    /// Rewritten alongside the thresholds it describes. Leaving it stale would
    /// leave consumers checking a hash that no longer matches the rules in
    /// force, which is worse than letting it change.
    pub methodology_hash: [u8; 32],

    /// 1 = require Pyth's Full verification. 0 = also accept a partially
    /// verified update, which posting prices atomically produces.
    pub min_verification_level: u8,
}

#[derive(Accounts)]
#[instruction(p: UpdateSymbolParams)]
pub struct UpdateSymbolConfig<'info> {
    /// Must be the authority recorded when the symbol was created. Anchor's
    /// `has_one` enforces it; there is no transfer path here on purpose.
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED, p.symbol.as_ref()],
        bump,
        has_one = authority
    )]
    pub config: Account<'info, AdapterConfig>,
}

pub fn handle_update_symbol_config(
    ctx: Context<UpdateSymbolConfig>,
    p: UpdateSymbolParams,
) -> Result<()> {
    // The same gate `initialize_symbol` applies. Shared rather than restated:
    // if the two ever disagreed, this instruction could write a configuration
    // that `initialize_symbol` would have rejected, and the invariant that
    // every live config passed validation would quietly stop holding.
    crate::instructions::initialize::validate_thresholds(
        p.market_closed_secs,
        p.reference_stale_secs,
        p.reference_max_age_secs,
        p.token_stale_secs,
        p.token_max_age_secs,
        p.max_deviation_bps,
        p.max_conf_bps,
        p.min_verification_level,
    )?;

    let c = &mut ctx.accounts.config;

    msg!(
        "config updated: verification {} -> {}, deviation {} -> {}bps",
        c.min_verification_level,
        p.min_verification_level,
        c.max_deviation_bps,
        p.max_deviation_bps
    );

    c.market_closed_secs = p.market_closed_secs;
    c.reference_stale_secs = p.reference_stale_secs;
    c.reference_max_age_secs = p.reference_max_age_secs;
    c.token_stale_secs = p.token_stale_secs;
    c.token_max_age_secs = p.token_max_age_secs;
    c.max_deviation_bps = p.max_deviation_bps;
    c.max_conf_bps = p.max_conf_bps;
    c.methodology_hash = p.methodology_hash;
    c.min_verification_level = p.min_verification_level;

    // Feed ids, quote accounts, descriptors, core program and authority are
    // untouched by construction: this function never names them.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::initialize::MAX_BPS;
    use crate::instructions::initialize::validate_thresholds;

    /// Thresholds a real deployment uses, as a starting point to perturb.
    fn ok_args() -> (u64, u64, u64, u64, u64, u64, u64, u8) {
        (900, 4 * 24 * 3600, 8 * 24 * 3600, 120, 30 * 60, 200, 500, 1)
    }

    fn call(a: (u64, u64, u64, u64, u64, u64, u64, u8)) -> Result<()> {
        validate_thresholds(a.0, a.1, a.2, a.3, a.4, a.5, a.6, a.7)
    }

    #[test]
    fn accepts_the_shipping_configuration() {
        assert!(call(ok_args()).is_ok());
    }

    #[test]
    fn both_verification_levels_are_settable() {
        // The whole point of the instruction: 0 must be reachable, because a
        // deployment posting its own prices cannot produce anything else.
        let mut a = ok_args();
        a.7 = 0;
        assert!(call(a).is_ok(), "level 0 must be settable");
        a.7 = 1;
        assert!(call(a).is_ok(), "level 1 must stay settable");
        a.7 = 2;
        assert!(call(a).is_err(), "there is no level above Full");
    }

    #[test]
    fn thresholds_must_widen() {
        // stale before closed: the flag would fire before the milder one.
        let mut a = ok_args();
        a.1 = a.0 - 1;
        assert!(call(a).is_err(), "reference_stale below market_closed");

        // max_age below stale: refused before ever being doubted.
        let mut a = ok_args();
        a.2 = a.1 - 1;
        assert!(call(a).is_err(), "reference_max_age below reference_stale");

        let mut a = ok_args();
        a.4 = a.3 - 1;
        assert!(call(a).is_err(), "token_max_age below token_stale");
    }

    #[test]
    fn zero_max_age_is_rejected() {
        // Would refuse every price including one published this instant.
        let mut a = ok_args();
        a.2 = 0;
        assert!(call(a).is_err());
        let mut a = ok_args();
        a.4 = 0;
        assert!(call(a).is_err());
    }

    #[test]
    fn tolerances_must_be_usable() {
        // Zero deviation flags every quote; above 100% flags none ever.
        let mut a = ok_args();
        a.5 = 0;
        assert!(call(a).is_err(), "zero deviation tolerance");
        let mut a = ok_args();
        a.5 = MAX_BPS + 1;
        assert!(call(a).is_err(), "deviation above 100%");

        let mut a = ok_args();
        a.6 = 0;
        assert!(call(a).is_err(), "zero confidence tolerance");
        let mut a = ok_args();
        a.6 = MAX_BPS + 1;
        assert!(call(a).is_err(), "confidence above 100%");
    }

    #[test]
    fn boundaries_are_inclusive() {
        // Equal thresholds are degenerate but ordered, so they are allowed;
        // rejecting them would be an off-by-one that only shows up in
        // production on a deliberately tight configuration.
        let mut a = ok_args();
        a.0 = 900;
        a.1 = 900;
        a.2 = 900;
        assert!(call(a).is_ok(), "equal reference thresholds are ordered");

        let mut a = ok_args();
        a.5 = MAX_BPS;
        a.6 = MAX_BPS;
        assert!(call(a).is_ok(), "exactly 100% is within range");
    }

    /// The reason the validator is shared rather than copied.
    ///
    /// Whatever `update_symbol_config` accepts, `initialize_symbol` must accept
    /// too. This drives the real `InitSymbolParams::validate` rather than
    /// calling the shared helper twice -- comparing a function to itself would
    /// pass even if `initialize_symbol` stopped delegating to it, which is the
    /// only way the two could actually diverge.
    #[test]
    fn update_cannot_write_what_initialize_would_reject() {
        use crate::instructions::initialize::InitSymbolParams;

        let cases: [(u64, u64, u64, u64, u64, u64, u64, u8); 9] = [
            ok_args(),
            (0, 0, 1, 0, 1, 1, 1, 0),
            (900, 900, 900, 120, 120, MAX_BPS, MAX_BPS, 1),
            (1, 2, 3, 1, 2, 10, 10, 0),
            (5000, 100, 8 * 24 * 3600, 0, 1800, 200, 500, 1),
            (900, 4 * 24 * 3600, 8 * 24 * 3600, 120, 1800, 0, 1, 2),
            // Each of the next three isolates one rule: everything else is
            // valid, so the case can only fail on the field being probed. A
            // case that trips an earlier rule proves nothing about a later
            // one, which is how the first draft of this list missed a
            // divergence in the verification-level check entirely.
            (900, 4 * 24 * 3600, 8 * 24 * 3600, 120, 1800, 200, 500, 2),
            (900, 4 * 24 * 3600, 8 * 24 * 3600, 120, 1800, MAX_BPS + 1, 500, 1),
            (900, 4 * 24 * 3600, 0, 120, 1800, 200, 500, 1),
        ];

        for (i, a) in cases.iter().enumerate() {
            let init = InitSymbolParams {
                symbol: *b"AAPL\0\0\0\0",
                svi_core_program: Pubkey::new_unique(),
                fair_descriptor: Pubkey::new_unique(),
                fair_quote: Pubkey::new_unique(),
                market_descriptor: Pubkey::new_unique(),
                market_quote: Pubkey::new_unique(),
                equity_feed_id: [1u8; 32],
                token_feed_id: [2u8; 32],
                market_closed_secs: a.0,
                reference_stale_secs: a.1,
                reference_max_age_secs: a.2,
                token_stale_secs: a.3,
                token_max_age_secs: a.4,
                max_deviation_bps: a.5,
                max_conf_bps: a.6,
                methodology_hash: [0u8; 32],
                base_decimals: 0,
                min_verification_level: a.7,
            };
            assert_eq!(
                call(*a).is_ok(),
                init.validate().is_ok(),
                "case {i}: update and initialize disagree about the same thresholds"
            );
        }
    }
}
