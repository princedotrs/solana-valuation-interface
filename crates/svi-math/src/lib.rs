//! Exact integer math with an explicit, mandatory rounding direction.
//!
//! Every function here is *total*: it returns `Option` instead of panicking,
//! and the intermediate product of two `u64`s is computed in `u128`, which
//! cannot overflow. There is no floating point anywhere, by design.
#![cfg_attr(not(test), no_std)]
#![deny(clippy::arithmetic_side_effects)]

/// Which way to fall when a division has a remainder.
///
/// This is a security property, not formatting. The rule: round *against*
/// whoever benefits from the number. Collateral rounds `Down`, debt rounds `Up`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    Down,
    Up,
}

/// Largest `n` such that `10u64.pow(n)` fits in a `u64`.
pub const MAX_POW10: u32 = 19;

/// `10^exp` as a `u64`, or `None` if it would overflow.
#[must_use]
pub fn pow10(exp: u32) -> Option<u64> {
    if exp > MAX_POW10 {
        return None;
    }
    10u64.checked_pow(exp)
}

/// `a * b / denom`, rounding down. Exact: the product is computed in `u128`.
///
/// Returns `None` if `denom == 0` or the result does not fit in a `u64`.
#[must_use]
pub fn mul_div_floor(a: u64, b: u64, denom: u64) -> Option<u64> {
    let n = (a as u128).checked_mul(b as u128)?;
    // checked_div returns None for denom == 0 - no separate guard needed.
    u64::try_from(n.checked_div(u128::from(denom))?).ok()
}

/// `a * b / denom`, rounding up.
#[must_use]
pub fn mul_div_ceil(a: u64, b: u64, denom: u64) -> Option<u64> {
    let n = (a as u128).checked_mul(b as u128)?;
    let d = u128::from(denom);
    let q = n.checked_div(d)?;
    let q = if n.checked_rem(d)? == 0 { q } else { q.checked_add(1)? };
    u64::try_from(q).ok()
}

/// `a * b / denom` with the direction chosen at the call site.
#[must_use]
pub fn mul_div(a: u64, b: u64, denom: u64, rounding: Rounding) -> Option<u64> {
    match rounding {
        Rounding::Down => mul_div_floor(a, b, denom),
        Rounding::Up => mul_div_ceil(a, b, denom),
    }
}

/// Move a fixed-point amount between decimal scales.
///
/// `rescale(64_640_132, 9, 6, Rounding::Down) == Some(64_640)`
#[must_use]
pub fn rescale(amount: u64, from_decimals: u8, to_decimals: u8, rounding: Rounding) -> Option<u64> {
    if to_decimals >= from_decimals {
        let k = u32::from(to_decimals.checked_sub(from_decimals)?);
        mul_div(amount, pow10(k)?, 1, rounding)
    } else {
        let k = u32::from(from_decimals.checked_sub(to_decimals)?);
        mul_div(amount, 1, pow10(k)?, rounding)
    }
}

/// Value of `base_amount` base units, given the value of ONE WHOLE base token.
///
/// This is the SVI quote conversion: an adapter produces "one xSOL is worth
/// `unit_value`" and this turns it into the `base_amount -> quote_amount` pair
/// the quote account stores.
///
/// Both internal steps round in the same direction, so the total error is at
/// most 1 unit in `quote_decimals` and always on the conservative side.
#[must_use]
pub fn quote_amount(
    unit_value: u64,
    unit_value_decimals: u8,
    base_amount: u64,
    base_decimals: u8,
    quote_decimals: u8,
    rounding: Rounding,
) -> Option<u64> {
    let whole = pow10(u32::from(base_decimals))?;
    let scaled = mul_div(unit_value, base_amount, whole, rounding)?;
    rescale(scaled, unit_value_decimals, quote_decimals, rounding)
}

/// A published value with its uncertainty band. Construction enforces the
/// invariant the SVI core also checks: `lower <= mid <= upper`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    pub lower: u64,
    pub mid: u64,
    pub upper: u64,
}

impl Band {
    #[must_use]
    pub fn new(lower: u64, mid: u64, upper: u64) -> Option<Self> {
        if lower <= mid && mid <= upper {
            Some(Self { lower, mid, upper })
        } else {
            None
        }
    }

    /// Width of the band, saturating (never panics, never wraps).
    #[must_use]
    pub fn spread(&self) -> u64 {
        self.upper.saturating_sub(self.lower)
    }

    /// Reject a band whose width exceeds `max_bps` of the midpoint.
    /// A quote too uncertain to be useful must fail, not be published.
    #[must_use]
    pub fn within_tolerance(&self, max_bps: u64) -> Option<bool> {
        if self.mid == 0 {
            return Some(self.spread() == 0);
        }
        let bps = mul_div_ceil(self.spread(), 10_000, self.mid)?;
        Some(bps <= max_bps)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Oracle-price helpers
//
// Added for the tokenized-stock adapter, which reads two Pyth feeds and has to
// turn each one into a quote with bounds, then compare them. Deliberately
// expressed over plain integers — no Pyth types, no Solana types — so this
// crate stays dependency-free and testable on the host.
// ─────────────────────────────────────────────────────────────────────────────

/// Multiply by `10^shift`, or divide by `10^-shift` when `shift` is negative.
///
/// Pyth reports a price as a mantissa plus a base-10 `exponent` (usually
/// negative), so moving it to a fixed number of decimals is a signed shift
/// rather than the unsigned `from -> to` that [`rescale`] expresses.
///
/// `scale_by_pow10(19_234_567_800, 1, Rounding::Down) == Some(192_345_678_000)`
#[must_use]
pub fn scale_by_pow10(amount: u64, shift: i32, rounding: Rounding) -> Option<u64> {
    if shift >= 0 {
        let k = u32::try_from(shift).ok()?;
        mul_div(amount, pow10(k)?, 1, rounding)
    } else {
        // `-shift` cannot overflow for any i32 except i32::MIN, which
        // checked_neg rejects.
        let k = u32::try_from(shift.checked_neg()?).ok()?;
        mul_div(amount, 1, pow10(k)?, rounding)
    }
}

/// Turn a Pyth price and its confidence interval into a [`Band`] at
/// `out_decimals`.
///
/// Pyth publishes `price ± conf` with a shared base-10 `expo`. The confidence
/// interval is the oracle's own statement of how sure it is; SVI publishes it
/// as the quote's bounds rather than discarding it, so a consumer can see the
/// uncertainty instead of inferring it.
///
/// Rounding follows the house rule — round *against* whoever benefits:
/// the lower bound floors, the upper bound ceils, and the midpoint floors.
/// The band therefore never understates the oracle's uncertainty.
///
/// Returns `None` when the price is negative or zero (an equity or token price
/// that is not strictly positive is not a price we will publish), when the
/// exponent is out of range, or when any scaled value does not fit a `u64`.
///
/// `price - conf` **saturates at zero** rather than failing: a confidence
/// interval wider than the price itself is a real state of the world, and the
/// caller's band-width tolerance is the right place to reject it. Saturating
/// keeps `lower <= mid` true so the `Band` still constructs and the caller can
/// see how bad it is.
#[must_use]
pub fn pyth_band(price: i64, conf: u64, expo: i32, out_decimals: u8) -> Option<Band> {
    if price <= 0 {
        return None;
    }
    let p = u64::try_from(price).ok()?;

    // Shift from the oracle's own scale to ours.
    let shift = i32::from(out_decimals).checked_add(expo)?;

    let mid = scale_by_pow10(p, shift, Rounding::Down)?;
    let lower = scale_by_pow10(p.saturating_sub(conf), shift, Rounding::Down)?;
    let upper = scale_by_pow10(p.checked_add(conf)?, shift, Rounding::Up)?;

    Band::new(lower, mid, upper)
}

/// How far `market` sits from `reference`, in basis points of `reference`.
///
/// Used to decide whether a tokenized stock has drifted from the equity it
/// tracks. Absolute, so a discount and a premium of the same size report the
/// same number — the flag says "these disagree", and the sign is visible from
/// the two published values themselves.
///
/// Rounds **up**, so a deviation exactly on a threshold trips the flag rather
/// than slipping under it. Returns `None` if `reference` is zero.
///
/// `deviation_bps(102, 100) == Some(200)`
#[must_use]
pub fn deviation_bps(market: u64, reference: u64) -> Option<u64> {
    if reference == 0 {
        return None;
    }
    let diff = market.abs_diff(reference);
    mul_div_ceil(diff, 10_000, reference)
}
