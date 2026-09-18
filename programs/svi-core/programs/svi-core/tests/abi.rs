//! Guards the wire format the dependency-free `svi-abi` crate promises.
//!
//! Adapters encode calls with `svi-abi` and never link this program, because
//! they are pinned to whatever `anchor-lang` version the protocol they read
//! requires. That only stays safe while `svi-abi`'s encoding matches the one
//! Anchor generates here. This is the test that keeps them in step.

use anchor_lang::{Discriminator, InstructionData};
use svi_core::QuoteUpdate;

/// If this fails, every adapter is calling the wrong instruction.
#[test]
fn discriminator_matches_anchor() {
    assert_eq!(
        svi_abi::PUBLISH_QUOTE.as_slice(),
        svi_core::instruction::PublishQuote::DISCRIMINATOR,
        "svi_abi::PUBLISH_QUOTE has drifted from Anchor's generated discriminator"
    );
}

/// The strong one: `svi-abi`'s hand-rolled encoder must produce the exact
/// bytes Anchor's derived serializer does. Not the same length — the same
/// bytes. Borsh is positional, so a transposed pair of `u64` fields keeps the
/// length identical and would deserialize into the wrong values.
#[test]
fn svi_abi_encoding_is_byte_identical_to_anchor() {
    let update = QuoteUpdate {
        base_amount: 1_000_000,
        quote_amount: 61_469_035,
        lower_quote_amount: 61_469_035,
        upper_quote_amount: 61_495_215,
        observed_slot: 445_914_806,
        observed_unix_ts: 1_788_487_200,
        source_accounts_hash: [0xAB; 32],
        status_flags: 0b1_0001,
    };

    let anchor_bytes = svi_core::instruction::PublishQuote { u: update.clone() }.data();
    let abi_bytes = svi_abi::encode_publish_quote(&svi_abi::QuoteUpdate {
        base_amount: update.base_amount,
        quote_amount: update.quote_amount,
        lower_quote_amount: update.lower_quote_amount,
        upper_quote_amount: update.upper_quote_amount,
        observed_slot: update.observed_slot,
        observed_unix_ts: update.observed_unix_ts,
        source_accounts_hash: update.source_accounts_hash,
        status_flags: update.status_flags,
    });

    assert_eq!(
        anchor_bytes.len(),
        svi_abi::PUBLISH_QUOTE_IX_LEN,
        "PUBLISH_QUOTE_IX_LEN no longer describes the encoded instruction"
    );
    assert_eq!(
        anchor_bytes,
        abi_bytes.to_vec(),
        "svi-abi's encoding has diverged from Anchor's. Adapters in the wild \
         encode with svi-abi; do NOT change QuoteUpdate in place. Add a new \
         instruction with a new name so the discriminator changes and old \
         adapters fail loudly."
    );
}

/// Anchor must be able to read back what an adapter wrote.
#[test]
fn anchor_can_decode_what_svi_abi_encodes() {
    use anchor_lang::AnchorDeserialize;

    let u = svi_abi::QuoteUpdate {
        base_amount: 1_000_000,
        quote_amount: 61_469_035,
        lower_quote_amount: 61_326_271,
        upper_quote_amount: 61_495_215,
        observed_slot: 445_914_806,
        observed_unix_ts: -1,
        source_accounts_hash: [9u8; 32],
        status_flags: u64::MAX,
    };
    let bytes = svi_abi::encode_publish_quote(&u);

    let decoded = QuoteUpdate::try_from_slice(&bytes[8..]).expect("anchor must decode it");
    assert_eq!(decoded.base_amount, u.base_amount);
    assert_eq!(decoded.quote_amount, u.quote_amount);
    assert_eq!(decoded.lower_quote_amount, u.lower_quote_amount);
    assert_eq!(decoded.upper_quote_amount, u.upper_quote_amount);
    assert_eq!(decoded.observed_slot, u.observed_slot);
    assert_eq!(decoded.observed_unix_ts, u.observed_unix_ts);
    assert_eq!(decoded.source_accounts_hash, u.source_accounts_hash);
    assert_eq!(decoded.status_flags, u.status_flags);
}
