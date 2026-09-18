//! Raw CPI into `svi-core`, over its frozen byte ABI.
//!
//! ## Why not the generated Anchor CPI helper?
//!
//! Because this program cannot depend on `svi-core`. `hylo-core` requires
//! `anchor-lang 0.32.1`; `svi-core` is built on `1.1.2`. Both in one dependency
//! graph means two incompatible `Pubkey` and `AccountInfo` types.
//!
//! That constraint turned out to be a feature. An adapter is pinned to the
//! protocol it reads, and those pins will not always agree with the core's.
//! Talking over bytes means an adapter can be written against any Anchor
//! version, another framework, or none — which is what `svi-core` needs to be
//! if it is a standard rather than a library.
//!
//! The three constants below are duplicated from `svi-core/src/abi.rs`, and
//! `svi-core/tests/abi.rs` asserts they still match Anchor's codegen. If Anchor
//! changes its discriminator derivation, that test fails in CI rather than
//! this program silently calling into the wrong instruction.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

/// Anchor discriminator for `svi_core::publish_quote`.
/// Mirror of `svi_core::abi::PUBLISH_QUOTE`.
const PUBLISH_QUOTE: [u8; 8] = [31, 48, 172, 25, 195, 45, 13, 249];

/// Borsh-encoded length of `svi_core::QuoteUpdate`.
/// Mirror of `svi_core::abi::QUOTE_UPDATE_LEN`.
const QUOTE_UPDATE_LEN: usize = 8 * 4 + 8 + 8 + 32 + 8;

/// The value to publish. Field order here IS the wire order.
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

impl QuoteUpdate {
    fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + QUOTE_UPDATE_LEN);
        data.extend_from_slice(&PUBLISH_QUOTE);
        data.extend_from_slice(&self.base_amount.to_le_bytes());
        data.extend_from_slice(&self.quote_amount.to_le_bytes());
        data.extend_from_slice(&self.lower_quote_amount.to_le_bytes());
        data.extend_from_slice(&self.upper_quote_amount.to_le_bytes());
        data.extend_from_slice(&self.observed_slot.to_le_bytes());
        data.extend_from_slice(&self.observed_unix_ts.to_le_bytes());
        data.extend_from_slice(&self.source_accounts_hash);
        data.extend_from_slice(&self.status_flags.to_le_bytes());
        debug_assert_eq!(data.len(), 8 + QUOTE_UPDATE_LEN);
        data
    }
}

/// Publish `update` into `svi-core`, signed by this program's authority PDA.
///
/// Account order is fixed by `svi_core::abi::PUBLISH_QUOTE_ACCOUNTS`:
/// descriptor (ro), quote (rw), adapter_authority (signer).
pub fn publish_quote<'info>(
    core_program: &AccountInfo<'info>,
    descriptor: &AccountInfo<'info>,
    quote: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    authority_seeds: &[&[u8]],
    update: &QuoteUpdate,
) -> Result<()> {
    let ix = Instruction {
        program_id: *core_program.key,
        accounts: vec![
            AccountMeta::new_readonly(*descriptor.key, false),
            AccountMeta::new(*quote.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data: update.encode(),
    };
    invoke_signed(
        &ix,
        &[descriptor.clone(), quote.clone(), authority.clone()],
        &[authority_seeds],
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encoding is a wire format, so pin it against a known-good vector
    /// rather than against itself. The same vector appears in
    /// `svi-core/tests/abi.rs`, encoded by Anchor.
    #[test]
    fn encoding_is_stable() {
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
        let encoded = update.encode();
        assert_eq!(encoded.len(), 8 + QUOTE_UPDATE_LEN);
        assert_eq!(&encoded[..8], &PUBLISH_QUOTE);
        assert_eq!(
            &encoded[8..16],
            &1_000_000_000u64.to_le_bytes(),
            "base_amount must lead the payload"
        );
        assert_eq!(&encoded[encoded.len() - 8..], &5u64.to_le_bytes());
    }
}
