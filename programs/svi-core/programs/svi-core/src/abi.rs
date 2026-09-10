//! The stable wire format other programs use to call this one.
//!
//! The contract itself lives in the dependency-free [`svi_abi`] crate, so an
//! adapter can encode a call without linking this program — and therefore
//! without agreeing with it about `anchor-lang`, `solana-program`, or anything
//! else. See that crate's docs for why.
//!
//! This module re-exports it so `svi-core` has one visible surface, and
//! `tests/abi.rs` asserts those constants still match what Anchor generates
//! here. That test is the only thing keeping the two in step; if it fails,
//! every adapter in the wild is affected.

pub use svi_abi::{
    encode_publish_quote, PUBLISH_QUOTE, PUBLISH_QUOTE_ACCOUNTS, PUBLISH_QUOTE_IX_LEN,
    QUOTE_UPDATE_LEN,
};
