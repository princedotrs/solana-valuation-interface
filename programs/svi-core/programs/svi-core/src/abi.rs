//! The stable wire format other programs use to call this one.
//!
//! An adapter does NOT need to depend on this crate. It needs three things:
//! the instruction discriminator, the Borsh layout of [`crate::QuoteUpdate`],
//! and the account order of [`crate::PublishQuote`]. All three are frozen here
//! and asserted against Anchor's own codegen in `tests/abi.rs`.
//!
//! This decoupling is deliberate, and it is what makes SVI a standard rather
//! than a library. Adapters are pinned to the protocols they read — the Hylo
//! adapter must use the exact `anchor-lang` version `hylo-core` requires — and
//! that version will not always match this program's. Talking over bytes
//! instead of Rust types means an adapter can be written in any framework, any
//! Anchor version, or no framework at all.

/// Anchor instruction discriminator for `publish_quote`.
///
/// Do not edit by hand. `tests/abi.rs::discriminator_matches_anchor` asserts
/// this equals what the `#[program]` macro generated; if Anchor ever changes
/// its derivation, that test fails loudly rather than adapters silently
/// calling into the wrong instruction.
pub const PUBLISH_QUOTE: [u8; 8] = [31, 48, 172, 25, 195, 45, 13, 249];

/// Account order for `publish_quote`, as an adapter must build it.
///
/// | # | Account            | Signer | Writable |
/// |---|--------------------|--------|----------|
/// | 0 | descriptor         | no     | no       |
/// | 1 | quote              | no     | **yes**  |
/// | 2 | adapter_authority  | **yes**| no       |
///
/// The adapter's PDA at index 2 is the entire security model: only the adapter
/// program can sign for it, so the value's provenance is pinned to specific
/// code rather than to a key a person holds.
pub const PUBLISH_QUOTE_ACCOUNTS: usize = 3;

/// Borsh-serialized size of [`crate::QuoteUpdate`]: four u64 amounts, the
/// observed slot, the observed timestamp, a 32-byte hash, and the flags.
pub const QUOTE_UPDATE_LEN: usize = 8 * 4 + 8 + 8 + 32 + 8;
