//! CPI into `svi-core`, over its frozen byte ABI.
//!
//! ## Why bytes rather than the generated Anchor CPI helper
//!
//! This adapter is pinned to `anchor-lang = "=0.32.1"`, because
//! `pyth-solana-receiver-sdk 1.2.0` requires it. `svi-core` is built on
//! `1.1.2`. Asking for both puts two incompatible `anchor-lang` versions in
//! one dependency graph, where `Pubkey`, `Clock` and `AccountDeserialize`
//! become two unrelated types and nothing lines up.
//!
//! That is not special to Pyth. Every adapter is pinned to whatever the thing
//! it reads requires, those pins disagree with each other, and making the core
//! move to match whichever protocol came first would mean re-deploying the
//! trust anchor whenever a third party bumps a dependency.
//!
//! So the encoding lives in [`svi_abi`], which depends on nothing at all.
//! `svi-core/tests/abi.rs` asserts its output is byte-identical to Anchor's
//! own serializer, so drift fails CI rather than silently writing misread
//! values into a quote account.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

pub use svi_abi::QuoteUpdate;

/// Publish `update` into `svi-core`, signed by this program's authority PDA.
///
/// Account order is fixed by [`svi_abi::PUBLISH_QUOTE_ACCOUNTS`]:
/// descriptor (ro), quote (rw), adapter_authority (signer).
pub fn publish_quote<'info>(
    core_program: &AccountInfo<'info>,
    descriptor: &AccountInfo<'info>,
    quote: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    authority_seeds: &[&[u8]],
    update: &QuoteUpdate,
) -> Result<()> {
    let accounts = vec![
        AccountMeta::new_readonly(*descriptor.key, false),
        AccountMeta::new(*quote.key, false),
        AccountMeta::new_readonly(*authority.key, true),
    ];
    debug_assert_eq!(accounts.len(), svi_abi::PUBLISH_QUOTE_ACCOUNTS);

    let ix = Instruction {
        program_id: *core_program.key,
        accounts,
        data: svi_abi::encode_publish_quote(update).to_vec(),
    };

    invoke_signed(
        &ix,
        &[descriptor.clone(), quote.clone(), authority.clone()],
        &[authority_seeds],
    )
    .map_err(Into::into)
}
