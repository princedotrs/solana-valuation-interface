use anchor_lang::prelude::*;

/// Anchor allows exactly one `#[error_code]` enum per program.
///
/// Every variant here is a refusal to publish. There is deliberately no error
/// that means "published something approximate": the design rule is *fail
/// stale, never fail wrong*, so an adapter that cannot verify its inputs
/// aborts and lets the existing quote age past its `valid_until_slot`.
#[error_code]
pub enum AdapterError {
    #[msg("Hylo state account is not owned by the Hylo Exchange program")]
    HyloStateWrongOwner,
    #[msg("Hylo state account is not the canonical protocol state PDA")]
    HyloStateWrongAddress,
    #[msg("Levercoin mint is not xSOL")]
    WrongLevercoinMint,
    #[msg("Pyth account does not match the feed Hylo itself is configured to use")]
    OracleMismatch,
    #[msg("Hylo protocol or the LST pair is paused")]
    HyloPaused,
    #[msg("Hylo state could not be loaded: stale LST cache, or an oracle outside Hylo's own tolerance")]
    ContextUnavailable,
    #[msg("hylo-core declined to produce a NAV for this state")]
    NavUnavailable,
    #[msg("Computed bounds are incoherent: require lower <= value <= upper")]
    InvalidBounds,
    #[msg("Uncertainty band is wider than this feed's configured tolerance")]
    BandTooWide,
    #[msg("Configured svi-core program does not match the account passed")]
    WrongCoreProgram,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
