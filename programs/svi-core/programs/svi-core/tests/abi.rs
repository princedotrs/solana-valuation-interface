//! Guards the wire format `svi-core/src/abi.rs` promises to adapters.
//!
//! Adapters cannot depend on this crate (they are pinned to whatever
//! `anchor-lang` version the protocol they read requires), so they hardcode
//! these bytes. That is only safe if something asserts the hardcoded values
//! still match Anchor's codegen. This is that something.

use anchor_lang::{Discriminator, InstructionData};
use svi_core::{abi, QuoteUpdate};

/// If this fails, every adapter in the wild is calling the wrong instruction.
/// Update `abi::PUBLISH_QUOTE` and treat it as a breaking change to the
/// standard, not a patch.
#[test]
fn discriminator_matches_anchor() {
    assert_eq!(
        abi::PUBLISH_QUOTE.as_slice(),
        svi_core::instruction::PublishQuote::DISCRIMINATOR,
        "abi::PUBLISH_QUOTE has drifted from Anchor's generated discriminator"
    );
}

/// An adapter serializes `QuoteUpdate` by hand. Prove the layout Anchor emits
/// is the one `abi::QUOTE_UPDATE_LEN` describes, and that a hand-rolled
/// encoding round-trips through Anchor's own decoder.
#[test]
fn quote_update_wire_layout_is_stable() {
    let update = QuoteUpdate {
        base_amount: 1_000_000_000,
        quote_amount: 28_431_250,
        lower_quote_amount: 28_384_000,
        upper_quote_amount: 28_478_500,
        observed_slot: 412_345_678,
        observed_unix_ts: 1_757_000_000,
        source_accounts_hash: [7u8; 32],
        status_flags: 0b101,
    };

    let anchor_encoded = svi_core::instruction::PublishQuote { u: update.clone() }.data();
    assert_eq!(
        anchor_encoded.len(),
        8 + abi::QUOTE_UPDATE_LEN,
        "QUOTE_UPDATE_LEN no longer describes the encoded instruction"
    );
    assert_eq!(&anchor_encoded[..8], abi::PUBLISH_QUOTE.as_slice());

    // Hand-rolled encoding, exactly as an adapter with no dependency on this
    // crate would build it. Little-endian scalars in declaration order.
    let mut manual = Vec::with_capacity(8 + abi::QUOTE_UPDATE_LEN);
    manual.extend_from_slice(&abi::PUBLISH_QUOTE);
    manual.extend_from_slice(&update.base_amount.to_le_bytes());
    manual.extend_from_slice(&update.quote_amount.to_le_bytes());
    manual.extend_from_slice(&update.lower_quote_amount.to_le_bytes());
    manual.extend_from_slice(&update.upper_quote_amount.to_le_bytes());
    manual.extend_from_slice(&update.observed_slot.to_le_bytes());
    manual.extend_from_slice(&update.observed_unix_ts.to_le_bytes());
    manual.extend_from_slice(&update.source_accounts_hash);
    manual.extend_from_slice(&update.status_flags.to_le_bytes());

    assert_eq!(
        manual, anchor_encoded,
        "an adapter's hand-rolled encoding no longer matches Anchor's"
    );
}
