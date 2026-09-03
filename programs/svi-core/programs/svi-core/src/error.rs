use anchor_lang::prelude::*;

// Anchor 1.x allows exactly ONE #[error_code] enum per program. Plan accordingly.
#[error_code]
pub enum SviError {
    #[msg("Feed is frozen or deprecated")]
    FeedNotActive,
    #[msg("Signer is not the registered adapter authority for this feed")]
    UnauthorizedAdapter,
    #[msg("Source data is not newer than the currently published quote")]
    StaleObservation,
    #[msg("observed_slot is in the future")]
    ObservationFromFuture,
    #[msg("Source data was already too old to publish")]
    ObservationTooOld,
    #[msg("Bounds violated: require lower <= value <= upper")]
    InvalidBounds,
    #[msg("base_amount must be non-zero")]
    ZeroBaseAmount,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Invalid value type discriminant")]
    InvalidValueType,
}
