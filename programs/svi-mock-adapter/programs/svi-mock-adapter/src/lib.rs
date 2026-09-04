use anchor_lang::prelude::*;
use svi_core::{cpi::accounts::PublishQuote, program::SviCore, QuoteUpdate};

declare_id!("3kK7593G5wN5R9S9Sce5zVAy2BuijtMEFwzUCjh3oW8M");

pub const AUTHORITY_SEED: &[u8] = b"svi-authority";

#[program]
pub mod svi_mock_adapter {
    use super::*;

    /// A real adapter reads verified accounts and computes this value.
    /// The mock takes it as an argument. That is the ONLY difference.
    pub fn publish_mock(
        ctx: Context<PublishMock>,
        quote_amount: u64,
        spread_bps: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;

        let spread = quote_amount
            .checked_mul(spread_bps)
            .ok_or(MockError::MathOverflow)?
            / 10_000;

        let update = QuoteUpdate {
            base_amount: 1_000_000_000,
            quote_amount,
            lower_quote_amount: quote_amount.saturating_sub(spread),
            upper_quote_amount: quote_amount
                .checked_add(spread)
                .ok_or(MockError::MathOverflow)?,
            observed_slot: clock.slot,
            observed_unix_ts: clock.unix_timestamp,
            source_accounts_hash: [0u8; 32],
            status_flags: 0,
        };

        // The PDA has no private key. The runtime grants a signature for it
        // ONLY to this program, and only when these exact seeds are supplied.
        let bump = ctx.bumps.adapter_authority;
        let seeds: &[&[u8]] = &[AUTHORITY_SEED, &[bump]];
        let signer: &[&[&[u8]]] = &[seeds];

        svi_core::cpi::publish_quote(
            CpiContext::new_with_signer(
                ctx.accounts.svi_core_program.key(), // <-- Pubkey in Anchor 1.x
                PublishQuote {
                    descriptor: ctx.accounts.descriptor.to_account_info(),
                    quote: ctx.accounts.quote.to_account_info(),
                    adapter_authority: ctx.accounts.adapter_authority.to_account_info(),
                },
                signer,
            ),
            update,
        )
    }
}

#[derive(Accounts)]
pub struct PublishMock<'info> {
    /// CHECK: svi-core validates this against its own PDA seeds.
    pub descriptor: UncheckedAccount<'info>,
    /// CHECK: svi-core validates this against its own PDA seeds.
    #[account(mut)]
    pub quote: UncheckedAccount<'info>,
    /// CHECK: PDA signer. Only this program can produce a signature for it.
    #[account(seeds = [AUTHORITY_SEED], bump)]
    pub adapter_authority: UncheckedAccount<'info>,
    pub svi_core_program: Program<'info, SviCore>,
}

#[error_code]
pub enum MockError {
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
