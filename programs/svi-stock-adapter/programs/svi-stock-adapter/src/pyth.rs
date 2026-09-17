//! Reading a Pyth price account, and refusing to believe it until it earns it.
//!
//! # Why this file is longer than it looks like it should be
//!
//! The Hylo adapter does almost none of this. It checks that the Pyth account
//! it was handed is the one **Hylo's own on-chain state names** as its oracle,
//! and then hands the account to `hylo-core`, which applies Hylo's configured
//! staleness and confidence rules. Hylo vouches for the feed; the adapter
//! inherits that judgement rather than forming its own.
//!
//! A tokenized stock has no protocol behind it to vouch for anything. There is
//! no on-chain account that says "this is the correct AAPL feed", and no
//! library that says how old is too old. So every check Hylo delegated has to
//! be made here, explicitly, against thresholds recorded in this adapter's own
//! config account where a third party can read them.
//!
//! The attack this defends against is mundane and complete: the keeper chooses
//! which accounts go into the transaction. If the program trusted an account
//! by position, anyone could pass a real, fully-verified Pyth account for a
//! *different, cheaper* asset and have its price published as AAPL's.

use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, VerificationLevel};
use svi_math::{deviation_bps, pyth_band, Band};

use crate::constants::QUOTE_DECIMALS;
use crate::error::StockAdapterError;

/// Clock skew we tolerate before calling a publish time "from the future".
///
/// Pyth publish times come from the wormhole message, not from this
/// validator's clock, so a second or two of disagreement is normal and not
/// evidence of anything. Beyond that, something is wrong and we stop.
const FUTURE_TOLERANCE_SECS: i64 = 30;

/// A price that has passed every check, converted to the published scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedPrice {
    /// Price and its confidence interval, at [`QUOTE_DECIMALS`].
    pub band: Band,
    /// When Pyth says this price was observed.
    pub publish_time: i64,
    /// The slot at which the update was posted on this chain.
    pub posted_slot: u64,
    /// How old the price was when we read it, in seconds. Never negative.
    pub age_secs: u64,
    /// Width of the confidence interval, in basis points of the midpoint.
    pub conf_bps: u64,
}

impl VerifiedPrice {
    /// True when the price is older than `threshold_secs`.
    #[must_use]
    pub fn older_than(&self, threshold_secs: u64) -> bool {
        self.age_secs > threshold_secs
    }
}

/// Load a Pyth price account and verify it is the one this feed is entitled to
/// use, at a quality we are willing to publish.
///
/// Checks run in this order, and each one aborts the whole refresh:
///
/// 1. the account is owned by the Pyth receiver program;
/// 2. it deserializes as a `PriceUpdateV2`;
/// 3. its verification level is `Full` — a partially verified update needs
///    fewer colluding guardians to forge, and we are not in a hurry;
/// 4. its feed id equals the one in this symbol's config;
/// 5. its publish time is positive and not from the future;
/// 6. it is not older than the hard limit for this feed;
/// 7. its price converts to a strictly positive value at our scale;
/// 8. its confidence interval is no wider than the configured tolerance.
///
/// Note what is **not** here: nothing about market hours. Age is measured and
/// returned; deciding that an old equity price means "the market is closed"
/// rather than "the oracle is broken" is a judgement the caller makes with its
/// own thresholds, because the answer differs between the two feeds.
pub fn load_verified(
    account: &AccountInfo,
    expected_feed_id: &[u8; 32],
    now_unix: i64,
    max_age_secs: u64,
    max_conf_bps: u64,
) -> Result<VerifiedPrice> {
    // 1. Owner. Without this, any account whose bytes happen to deserialize
    //    could be passed — including one an attacker wrote themselves.
    require_keys_eq!(
        *account.owner,
        pyth_solana_receiver_sdk::ID,
        StockAdapterError::PythWrongOwner
    );

    // 2. Shape.
    let data = account.try_borrow_data()?;
    let update = PriceUpdateV2::try_deserialize(&mut data.as_ref())
        .map_err(|_| error!(StockAdapterError::PythMalformed))?;

    // 3. Verification level. `gte` treats Full as greater than any Partial.
    require!(
        update.verification_level.gte(VerificationLevel::Full),
        StockAdapterError::PythNotFullyVerified
    );

    // 4. Identity. THE check: a real, fully verified price for the wrong asset
    //    is exactly what a malicious keeper would supply.
    require!(
        update.price_message.feed_id == *expected_feed_id,
        StockAdapterError::PythFeedMismatch
    );

    // 5. Time sanity.
    let publish_time = update.price_message.publish_time;
    require!(
        publish_time > 0,
        StockAdapterError::PythInvalidPublishTime
    );
    require!(
        publish_time <= now_unix.saturating_add(FUTURE_TOLERANCE_SECS),
        StockAdapterError::PythFromFuture
    );

    // Saturating: a publish time inside the tolerance window but still ahead
    // of our clock reads as age zero rather than wrapping.
    let age_secs = u64::try_from(now_unix.saturating_sub(publish_time)).unwrap_or(0);

    // 6. Hard age limit. Past this the number is not evidence of anything, and
    //    no flag can make it usable.
    require!(age_secs <= max_age_secs, StockAdapterError::PythTooOld);

    // 7. Convert to our scale. Rejects non-positive prices and anything that
    //    does not fit; the confidence interval becomes the published bounds.
    let band = pyth_band(
        update.price_message.price,
        update.price_message.conf,
        update.price_message.exponent,
        QUOTE_DECIMALS,
    )
    .ok_or(error!(StockAdapterError::PythPriceOutOfRange))?;
    require!(band.mid > 0, StockAdapterError::PythPriceOutOfRange);

    // 8. Confidence. A band wide enough to contain any answer is not a price,
    //    and a consumer reading only the midpoint would never notice.
    let conf_bps = deviation_bps(band.upper, band.mid)
        .and_then(|upper_side| deviation_bps(band.lower, band.mid).map(|lower| upper_side.max(lower)))
        .ok_or(error!(StockAdapterError::MathOverflow))?;
    require!(
        conf_bps <= max_conf_bps,
        StockAdapterError::PythConfidenceTooWide
    );

    Ok(VerifiedPrice {
        band,
        publish_time,
        posted_slot: update.posted_slot,
        age_secs,
        conf_bps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The confidence measure is the wider of the two sides, so an asymmetric
    /// band (which rounding can produce) is judged by its worst side.
    #[test]
    fn conf_bps_takes_the_wider_side() {
        let band = Band::new(90, 100, 130).unwrap();
        let upper = deviation_bps(band.upper, band.mid).unwrap();
        let lower = deviation_bps(band.lower, band.mid).unwrap();
        assert_eq!(upper, 3_000);
        assert_eq!(lower, 1_000);
        assert_eq!(upper.max(lower), 3_000);
    }

    #[test]
    fn a_point_band_has_zero_confidence_width() {
        let band = Band::new(100, 100, 100).unwrap();
        assert_eq!(deviation_bps(band.upper, band.mid).unwrap(), 0);
        assert_eq!(deviation_bps(band.lower, band.mid).unwrap(), 0);
    }

    #[test]
    fn future_tolerance_is_small_but_non_zero() {
        // A publish time a few seconds ahead of our clock is ordinary skew.
        // Minutes ahead is not, and must not read as "age zero, very fresh".
        assert!(FUTURE_TOLERANCE_SECS > 0);
        assert!(FUTURE_TOLERANCE_SECS < 60);
    }
}
