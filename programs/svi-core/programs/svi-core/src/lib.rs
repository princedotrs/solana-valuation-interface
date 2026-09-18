use anchor_lang::prelude::*;

pub mod abi;
pub mod error;
pub mod state;

use error::SviError;
use state::*;

declare_id!("5yVpoQJCtEqM1RC2Fn4D6rCSoq79e3tQ92Kx4fyePECN"); // replaced by `anchor keys sync`

#[program]
pub mod svi_core {
    use super::*;

    pub fn initialize_feed(ctx: Context<InitializeFeed>, p: InitFeedParams) -> Result<()> {
        // Keep in step with `ValueType`'s highest discriminant. A value the
        // core does not know is refused at feed creation rather than being
        // stored and later misread by a consumer that maps it to nothing.
        require!(
            p.value_type >= 1 && p.value_type <= ValueType::ReferenceFairValue as u8,
            SviError::InvalidValueType
        );

        let d = &mut ctx.accounts.descriptor;
        d.feed_id = p.feed_id;
        d.authority = ctx.accounts.authority.key();
        d.adapter_program = p.adapter_program;
        d.adapter_authority = p.adapter_authority;
        d.base_mint = p.base_mint;
        d.quote_mint = p.quote_mint;
        d.methodology_hash = p.methodology_hash;
        d.adapter_config = p.adapter_config;
        d.max_age_slots = p.max_age_slots;
        d.quote_currency_code = p.quote_currency_code;
        d.value_type = p.value_type;
        d.base_decimals = p.base_decimals;
        d.quote_decimals = p.quote_decimals;
        d.status = FeedStatus::Active as u8;
        d.version = QUOTE_LAYOUT_VERSION;
        d.bump = ctx.bumps.descriptor;

        // Stamp the quote's immutable identity now. Everything else stays zero
        // until the first publish, so `sequence == 0` means "never published".
        let mut q = ctx.accounts.quote.load_init()?;
        q.descriptor = d.key();
        q.feed_id = p.feed_id;
        q.base_mint = p.base_mint;
        q.quote_mint = p.quote_mint;
        q.adapter_program = p.adapter_program;
        q.methodology_hash = p.methodology_hash;
        q.quote_currency_code = p.quote_currency_code;
        q.value_type = p.value_type;
        q.base_decimals = p.base_decimals;
        q.quote_decimals = p.quote_decimals;
        q.version = QUOTE_LAYOUT_VERSION;
        q.bump = ctx.bumps.quote;
        Ok(())
    }

    /// Called by an adapter program via CPI. This is the ONLY way a value is written.
    pub fn publish_quote(ctx: Context<PublishQuote>, u: QuoteUpdate) -> Result<()> {
        let d = &ctx.accounts.descriptor;
        let clock = Clock::get()?;

        // ---- Rule 1: the feed is live ----
        require!(
            d.status == FeedStatus::Active as u8,
            SviError::FeedNotActive
        );

        // ---- Rule 2: only the registered adapter authority may write ----
        // The adapter's PDA can only be signed for by the adapter program itself,
        // so this single check pins the value's provenance to specific code.
        require_keys_eq!(
            ctx.accounts.adapter_authority.key(),
            d.adapter_authority,
            SviError::UnauthorizedAdapter
        );

        // ---- Rule 3: the value is internally coherent ----
        require!(u.base_amount > 0, SviError::ZeroBaseAmount);
        require!(
            u.lower_quote_amount <= u.quote_amount && u.quote_amount <= u.upper_quote_amount,
            SviError::InvalidBounds
        );

        // ---- Rule 4: the source data is fresh, and newer than what's there ----
        require!(
            u.observed_slot <= clock.slot,
            SviError::ObservationFromFuture
        );
        let age = clock.slot.saturating_sub(u.observed_slot);
        require!(age <= d.max_age_slots, SviError::ObservationTooOld);

        let mut q = ctx.accounts.quote.load_mut()?;
        require!(
            u.observed_slot > q.observed_slot,
            SviError::StaleObservation
        );

        // ---- Write ----
        q.base_amount = u.base_amount;
        q.quote_amount = u.quote_amount;
        q.lower_quote_amount = u.lower_quote_amount;
        q.upper_quote_amount = u.upper_quote_amount;
        q.source_accounts_hash = u.source_accounts_hash;
        q.status_flags = u.status_flags;
        q.observed_slot = u.observed_slot;
        q.observed_unix_ts = u.observed_unix_ts;
        q.published_slot = clock.slot;
        q.valid_until_slot = u
            .observed_slot
            .checked_add(d.max_age_slots)
            .ok_or(SviError::MathOverflow)?;
        q.sequence = q.sequence.checked_add(1).ok_or(SviError::MathOverflow)?;

        emit!(QuotePublished {
            feed_id: q.feed_id,
            quote_amount: q.quote_amount,
            observed_slot: q.observed_slot,
            sequence: q.sequence,
            status_flags: q.status_flags,
        });
        Ok(())
    }

    pub fn set_feed_status(ctx: Context<AdminFeed>, status: u8) -> Result<()> {
        ctx.accounts.descriptor.status = status;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitFeedParams {
    pub feed_id: [u8; 32],
    pub adapter_program: Pubkey,
    pub adapter_authority: Pubkey,
    pub adapter_config: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub methodology_hash: [u8; 32],
    pub max_age_slots: u64,
    pub quote_currency_code: u16,
    pub value_type: u8,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct QuoteUpdate {
    pub base_amount: u64,
    pub quote_amount: u64,
    pub lower_quote_amount: u64,
    pub upper_quote_amount: u64,
    pub observed_slot: u64,
    pub observed_unix_ts: i64,
    pub source_accounts_hash: [u8; 32],
    pub status_flags: u64,
}

#[event]
pub struct QuotePublished {
    pub feed_id: [u8; 32],
    pub quote_amount: u64,
    pub observed_slot: u64,
    pub sequence: u64,
    pub status_flags: u64,
}

#[derive(Accounts)]
#[instruction(p: InitFeedParams)]
pub struct InitializeFeed<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init, payer = authority, space = 8 + Descriptor::INIT_SPACE,
        seeds = [b"descriptor", p.feed_id.as_ref()], bump
    )]
    pub descriptor: Account<'info, Descriptor>,
    #[account(
        init, payer = authority, space = 8 + core::mem::size_of::<Quote>(),
        seeds = [b"quote", p.feed_id.as_ref()], bump
    )]
    pub quote: AccountLoader<'info, Quote>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PublishQuote<'info> {
    #[account(seeds = [b"descriptor", descriptor.feed_id.as_ref()], bump = descriptor.bump)]
    pub descriptor: Account<'info, Descriptor>,
    #[account(mut, seeds = [b"quote", descriptor.feed_id.as_ref()], bump = quote.load()?.bump)]
    pub quote: AccountLoader<'info, Quote>,
    /// The adapter's PDA. Being a Signer here is the whole security model.
    pub adapter_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdminFeed<'info> {
    #[account(has_one = authority)]
    pub descriptor: Account<'info, Descriptor>,
    pub authority: Signer<'info>,
}
