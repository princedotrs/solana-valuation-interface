use anchor_lang::prelude::*;

/// One per adapter deployment. Records exactly which code produced a value.
///
/// `sdk_revision` is the point of the account: it is the `hylo-core` git commit
/// this binary was built against, recorded on-chain so a third party can fetch
/// the same revision and reproduce any published quote byte-for-byte. A quote
/// whose methodology you cannot pin to a specific revision of specific math is
/// not reproducible, and reproducibility is the entire product.
#[account]
#[derive(InitSpace)]
pub struct AdapterConfig {
    /// Admin: may retire this adapter. Cannot influence a published value.
    pub authority: Pubkey,
    /// The `svi-core` program this adapter publishes into.
    pub svi_core_program: Pubkey,
    /// Descriptor account of the feed this adapter writes.
    pub descriptor: Pubkey,
    /// Quote account of the feed this adapter writes.
    pub quote: Pubkey,
    /// `sha256` of the frozen methodology document (`hylo-xsol-nav-v1`).
    pub methodology_hash: [u8; 32],
    /// The pinned `hylo-core` git revision, as 20 raw bytes of the commit SHA.
    pub sdk_revision: [u8; 20],
    /// Maximum acceptable width of the mint/redeem band, in basis points of
    /// the midpoint. A quote too uncertain to be useful must fail, not publish.
    pub max_band_bps: u64,
    /// Bump for the signing authority PDA.
    pub authority_bump: u8,
    /// Bump for this config PDA.
    pub bump: u8,
}
