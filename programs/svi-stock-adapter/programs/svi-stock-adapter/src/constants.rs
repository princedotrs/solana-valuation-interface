use anchor_lang::prelude::*;

/// Seed for a symbol's config account: `[CONFIG_SEED, symbol]`.
///
/// One config per symbol, so AAPL and TSLA are independent: retiring one, or
/// getting one's thresholds wrong, cannot affect the other.
#[constant]
pub const CONFIG_SEED: &[u8] = b"stock-config";

/// Seed for the adapter's single signing authority.
///
/// This PDA is the entire security model. Only this program can sign for it,
/// so `svi-core` accepting it as the writer pins a published value's
/// provenance to *this code*, not to any key a human holds.
///
/// Deliberately **not** per-symbol: `svi-core` records one `adapter_authority`
/// per feed, and every feed this program owns is written by the same program.
/// A per-symbol authority would add accounts and PDAs without adding a
/// boundary, since one program can sign for all of them either way.
#[constant]
pub const AUTHORITY_SEED: &[u8] = b"authority";

/// Decimals of every published quote. USD at 9 decimal places.
///
/// Matches the Hylo adapter, so a consumer reading any SVI quote scales it the
/// same way regardless of which adapter wrote it.
pub const QUOTE_DECIMALS: u8 = 9;

/// ISO-4217 numeric code for USD.
pub const USD_CURRENCY_CODE: u16 = 840;

/// Longest symbol this adapter supports, in bytes. Right-padded with zeros.
///
/// Fixed-size so `AdapterConfig` stays `InitSpace`-able and the PDA seed is a
/// predictable length. Eight covers every US ticker plus an `X` suffix.
pub const SYMBOL_LEN: usize = 8;

/// Status flags, bits 16-23, allocated to this adapter in the registry in
/// `svi-core::state::flags`.
///
/// Duplicated rather than imported because this program cannot depend on
/// `svi-core` (see `cpi.rs`). The values are part of the published format, not
/// an implementation detail — the registry in the core is what keeps these
/// hand-copied constants from colliding with another adapter's.
pub mod flags {
    /// The underlying equity market is not open. The reference price is the
    /// last one the equity oracle published, which is a real and useful number
    /// — it is simply not a *live* one, and the token keeps trading against it.
    ///
    /// This is the flag the whole product exists to raise. A tokenized stock
    /// trades 24/7 against an equity that trades roughly 6.5 hours a weekday;
    /// for most of the week a consumer comparing the two is comparing a live
    /// price to a stale one, and until now nothing on-chain said so.
    pub const MARKET_CLOSED: u64 = 1 << 16;

    /// The equity feed has not updated for so long that even "the market is
    /// closed" no longer explains it — a holiday weekend has passed, or the
    /// publisher has stopped. Distinct from `MARKET_CLOSED`, which is the
    /// ordinary nightly state.
    pub const REFERENCE_STALE: u64 = 1 << 17;

    /// The tokenized-stock feed has not updated within its window. This one is
    /// always abnormal: the token trades continuously, so its feed should not
    /// age the way the equity feed does.
    pub const TOKEN_FEED_STALE: u64 = 1 << 18;

    /// The token price and the reference price disagree by more than the
    /// symbol's configured tolerance.
    ///
    /// Set on **both** quotes of the pair, so a consumer that reads only the
    /// fair-value feed still learns that the market disagrees with it.
    pub const DEVIATION_HIGH: u64 = 1 << 19;

    /// Every bit this adapter may set. Bits 20-23 remain for future use
    /// without needing another range from the core's registry.
    pub const ALL: u64 = MARKET_CLOSED | REFERENCE_STALE | TOKEN_FEED_STALE | DEVIATION_HIGH;

    /// The range the core's registry allocated to this adapter.
    pub const ALLOCATED_RANGE: u64 = 0xFF << 16;
}
