//! The wire format for calling `svi-core`, with no dependencies at all.
//!
//! # Why this crate exists
//!
//! An SVI adapter is pinned to the protocol it reads. The Hylo adapter must
//! use `anchor-lang 0.32.1` because `hylo-core` requires exactly that; a
//! future Meteora or Sanctum adapter will be pinned to whatever those
//! protocols require. Those pins will not agree with each other, and they will
//! not always agree with `svi-core`.
//!
//! If adapters had to link `svi-core` to call it, every adapter would need the
//! same `anchor-lang` version as the core, and the first protocol to disagree
//! would be unsupportable. Making the core move instead is worse: it is the
//! trust anchor, meant to be frozen and eventually immutable, and it would be
//! re-deployed every time a third party bumped a dependency.
//!
//! So adapters talk to the core over **bytes**. This crate is that contract.
//! It has no dependencies — not `anchor-lang`, not `solana-program` — so it
//! cannot conflict with anything an adapter already links. Adding a dependency
//! here would defeat its entire purpose.
//!
//! # Compatibility rule
//!
//! The discriminator is the version. **Never change the meaning or layout of
//! [`QuoteUpdate`] in place.** A layout change is a new instruction with a new
//! name, therefore a new discriminator, and an adapter built against the old
//! one then fails loudly with "instruction not found" instead of silently
//! writing misinterpreted bytes into a quote account.
//!
//! Borsh is positional: reordering two `u64` fields keeps the payload the same
//! length and deserializes without error, into the wrong values. That is the
//! failure this rule prevents.
//!
//! `svi-core`'s own test suite asserts the constants here still match what
//! Anchor generates, so drift breaks CI rather than production.

#![no_std]
#![deny(clippy::arithmetic_side_effects)]

/// Anchor instruction discriminator for `svi_core::publish_quote`.
///
/// Verified against Anchor's codegen by `svi-core/tests/abi.rs`.
pub const PUBLISH_QUOTE: [u8; 8] = [31, 48, 172, 25, 195, 45, 13, 249];

/// Borsh-encoded length of [`QuoteUpdate`]: four `u64` amounts, the observed
/// slot, the observed timestamp, a 32-byte hash, and the flags.
pub const QUOTE_UPDATE_LEN: usize = 8 * 4 + 8 + 8 + 32 + 8;

/// Total instruction data length: discriminator plus payload. Fixed, so the
/// buffer can live on the stack and an adapter never allocates to publish.
pub const PUBLISH_QUOTE_IX_LEN: usize = 8 + QUOTE_UPDATE_LEN;

/// Number of accounts `publish_quote` takes, in this order:
///
/// | # | Account | Signer | Writable |
/// |---|---|---|---|
/// | 0 | `descriptor` | no | no |
/// | 1 | `quote` | no | **yes** |
/// | 2 | `adapter_authority` | **yes** | no |
///
/// The adapter's PDA at index 2 is the whole security model: only the adapter
/// program can sign for it, so the published value's provenance is pinned to
/// specific code rather than to a key someone holds.
pub const PUBLISH_QUOTE_ACCOUNTS: usize = 3;

/// A value to publish, in declaration order — which is also wire order.
///
/// Deliberately a plain struct with no derives that would pull in a
/// serialization framework. [`encode_publish_quote`] is the only encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Encode `publish_quote` instruction data: discriminator followed by the
/// Borsh payload, little-endian, in declaration order.
///
/// Returns a fixed-size array, so this never allocates and cannot fail.
#[must_use]
pub fn encode_publish_quote(u: &QuoteUpdate) -> [u8; PUBLISH_QUOTE_IX_LEN] {
    let mut out = [0u8; PUBLISH_QUOTE_IX_LEN];
    let mut at = 0usize;

    // A tiny local writer keeps every offset derived rather than hand-counted.
    // Hand-counted offsets in a wire format are how fields end up transposed.
    macro_rules! put {
        ($bytes:expr) => {{
            let b = $bytes;
            out[at..at + b.len()].copy_from_slice(&b);
            at += b.len();
        }};
    }

    put!(PUBLISH_QUOTE);
    put!(u.base_amount.to_le_bytes());
    put!(u.quote_amount.to_le_bytes());
    put!(u.lower_quote_amount.to_le_bytes());
    put!(u.upper_quote_amount.to_le_bytes());
    put!(u.observed_slot.to_le_bytes());
    put!(u.observed_unix_ts.to_le_bytes());
    put!(u.source_accounts_hash);
    put!(u.status_flags.to_le_bytes());

    debug_assert!(at == PUBLISH_QUOTE_IX_LEN);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: QuoteUpdate = QuoteUpdate {
        base_amount: 1_000_000_000,
        quote_amount: 28_431_250,
        lower_quote_amount: 28_384_000,
        upper_quote_amount: 28_478_500,
        observed_slot: 412_345_678,
        observed_unix_ts: 1_757_000_000,
        source_accounts_hash: [7u8; 32],
        status_flags: 0b101,
    };

    #[test]
    fn length_is_fixed_and_known() {
        assert_eq!(QUOTE_UPDATE_LEN, 88);
        assert_eq!(PUBLISH_QUOTE_IX_LEN, 96);
        assert_eq!(encode_publish_quote(&SAMPLE).len(), 96);
    }

    /// Field order is the contract. Pin each offset explicitly: a transposition
    /// keeps the payload the same length and would otherwise pass silently.
    #[test]
    fn every_field_lands_at_its_documented_offset() {
        let e = encode_publish_quote(&SAMPLE);
        let at = |o: usize| u64::from_le_bytes(e[o..o + 8].try_into().unwrap());

        assert_eq!(&e[0..8], &PUBLISH_QUOTE);
        assert_eq!(at(8), SAMPLE.base_amount);
        assert_eq!(at(16), SAMPLE.quote_amount);
        assert_eq!(at(24), SAMPLE.lower_quote_amount);
        assert_eq!(at(32), SAMPLE.upper_quote_amount);
        assert_eq!(at(40), SAMPLE.observed_slot);
        assert_eq!(
            i64::from_le_bytes(e[48..56].try_into().unwrap()),
            SAMPLE.observed_unix_ts
        );
        assert_eq!(&e[56..88], &SAMPLE.source_accounts_hash);
        assert_eq!(at(88), SAMPLE.status_flags);
    }

    #[test]
    fn negative_timestamps_round_trip() {
        // i64, not u64: a pre-1970 or clock-skewed value must not be mangled.
        let mut u = SAMPLE;
        u.observed_unix_ts = -1;
        let e = encode_publish_quote(&u);
        assert_eq!(i64::from_le_bytes(e[48..56].try_into().unwrap()), -1);
    }
}
