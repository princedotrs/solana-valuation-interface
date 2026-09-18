use anchor_lang::prelude::*;

use crate::constants::{AUTHORITY_SEED, CONFIG_SEED};
use crate::state::AdapterConfig;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitConfigParams {
    pub svi_core_program: Pubkey,
    pub descriptor: Pubkey,
    pub quote: Pubkey,
    pub methodology_hash: [u8; 32],
    pub sdk_revision: [u8; 20],
    pub max_band_bps: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + AdapterConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, AdapterConfig>,

    /// CHECK: The signing authority PDA. Never read or written — it exists
    /// only to be signed for. Validated by its seeds.
    #[account(seeds = [AUTHORITY_SEED], bump)]
    pub adapter_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(ctx: Context<Initialize>, p: InitConfigParams) -> Result<()> {
    let c = &mut ctx.accounts.config;
    c.authority = ctx.accounts.authority.key();
    c.svi_core_program = p.svi_core_program;
    c.descriptor = p.descriptor;
    c.quote = p.quote;
    c.methodology_hash = p.methodology_hash;
    c.sdk_revision = p.sdk_revision;
    c.max_band_bps = p.max_band_bps;
    c.authority_bump = ctx.bumps.adapter_authority;
    c.bump = ctx.bumps.config;

    msg!(
        "svi-hylo-adapter initialized: authority={}, band<={}bps",
        ctx.accounts.adapter_authority.key(),
        p.max_band_bps
    );
    Ok(())
}
