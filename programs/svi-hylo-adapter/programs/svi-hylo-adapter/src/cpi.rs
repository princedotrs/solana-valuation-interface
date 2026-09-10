//! CPI into `svi-core`, over its frozen byte ABI.
//!
//! ## Why bytes and not the generated Anchor CPI helper
//!
//! This program cannot link `svi-core`. `hylo-core` requires
//! `anchor-lang = "=0.32.1"` — a hard pin — while `svi-core` is built on
//! `1.1.2`. Asking for both puts two incompatible `anchor-lang` versions in
//! one dependency graph: `Pubkey`, `Clock` and `AccountDeserialize` all become
//! two unrelated types, `hylo-core`'s trait impls stop applying, and the build
//! fails with a dozen mismatched-type errors.
//!
//! That is not a quirk of Hylo. Every adapter is pinned to the protocol it
//! reads, those pins will disagree with each other, and forcing the core to
//! match whichever protocol came first would mean re-deploying the trust
//! anchor whenever a third party bumps a dependency.
//!
//! So the encoding lives in [`svi_abi`], which depends on nothing at all and
//! therefore conflicts with nothing. `svi-core/tests/abi.rs` asserts its
//! output is byte-identical to Anchor's own serializer, so drift fails CI
//! rather than silently writing misread values into a quote account.

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
