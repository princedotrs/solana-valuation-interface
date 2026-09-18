use anchor_lang::prelude::*;

/// Anchor allows exactly one `#[error_code]` enum per program.
///
/// **Append only.** Anchor numbers these positionally from 6000, and the
/// numbers are what a keeper, a dashboard and a log reader match on. Inserting
/// a variant in the middle silently renumbers every code after it.
///
/// Every variant here is a refusal to publish. There is deliberately no error
/// meaning "published something approximate": the design rule is *fail stale,
/// never fail wrong*, so an adapter that cannot verify its inputs aborts and
/// lets the existing quotes age past their `valid_until_slot`.
///
/// Conditions that are *reportable but not disqualifying* — a closed equity
/// market, a wide premium — are status flags on the published quote, not
/// errors. Refusing to publish overnight would take the feed dark exactly when
/// a consumer most needs to be told the market is shut.
#[error_code]
pub enum StockAdapterError {
    // ---- Pyth account identity ----
    #[msg("Price account is not owned by the Pyth receiver program")]
    PythWrongOwner,
    #[msg("Price account could not be deserialized as a PriceUpdateV2")]
    PythMalformed,
    #[msg("Price update is only partially verified; full guardian verification is required")]
    PythNotFullyVerified,
    #[msg("Price account carries a different feed id than this symbol's config")]
    PythFeedMismatch,

    // ---- Pyth data quality ----
    #[msg("Price update has a non-positive publish time")]
    PythInvalidPublishTime,
    #[msg("Price update is dated in the future")]
    PythFromFuture,
    #[msg("Price update is older than this feed's hard limit; refusing to publish")]
    PythTooOld,
    #[msg("Price is not strictly positive, or does not fit the published scale")]
    PythPriceOutOfRange,
    #[msg("Pyth confidence interval is wider than this symbol's tolerance")]
    PythConfidenceTooWide,

    // ---- account wiring ----
    #[msg("Configured svi-core program does not match the account passed")]
    WrongCoreProgram,
    #[msg("Descriptor or quote account does not match the one recorded in config")]
    WrongFeedAccount,
    #[msg("The two feeds of a pair must be distinct accounts")]
    FeedPairNotDistinct,

    // ---- config validity ----
    #[msg("Symbol must be non-empty, ASCII, and shorter than the fixed width")]
    InvalidSymbol,
    #[msg("Staleness thresholds must increase: closed <= stale <= max age")]
    InvalidThresholds,
    #[msg("Deviation or confidence tolerance is outside the permitted range")]
    InvalidTolerance,

    // ---- arithmetic ----
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
