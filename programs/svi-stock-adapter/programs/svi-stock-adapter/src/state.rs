use anchor_lang::prelude::*;

use crate::constants::SYMBOL_LEN;

/// One per symbol. Every threshold this adapter applies lives here, on-chain,
/// so a third party can read the rules that produced a quote rather than
/// having to trust a description of them.
///
/// Nothing here can change a price. The config says which feeds are
/// authoritative and when to raise a flag; the values themselves come from
/// Pyth and are converted by `svi-math`.
#[account]
#[derive(InitSpace)]
pub struct AdapterConfig {
    /// Admin: may retire this symbol. Cannot influence a published value.
    pub authority: Pubkey,

    /// The `svi-core` program this adapter publishes into.
    pub svi_core_program: Pubkey,

    // ---- the feed pair, both written by one refresh ----
    /// Descriptor of the `ReferenceFairValue` feed (the equity's price).
    pub fair_descriptor: Pubkey,
    /// Quote account of the `ReferenceFairValue` feed.
    pub fair_quote: Pubkey,
    /// Descriptor of the `MarketSpot` feed (the token's own price).
    pub market_descriptor: Pubkey,
    /// Quote account of the `MarketSpot` feed.
    pub market_quote: Pubkey,

    // ---- which Pyth feeds are authoritative for this symbol ----
    /// Feed id of the underlying equity, e.g. `Equity.US.AAPL/USD`.
    ///
    /// Checked against the feed id inside the price account on every refresh.
    /// Passing a different Pyth account — even a real, fully verified one —
    /// aborts, so a keeper cannot substitute a more flattering price.
    pub equity_feed_id: [u8; 32],
    /// Feed id of the tokenized stock, e.g. `Crypto.AAPLX/USD`.
    pub token_feed_id: [u8; 32],

    // ---- staleness policy ----
    //
    // Two windows per feed, because "too old to use" and "old enough to
    // mention" are different questions and collapsing them would make the
    // product impossible: an equity feed is *expected* to be hours old
    // overnight, and refusing to publish then would mean the feed goes dark
    // exactly when a consumer most needs to be told the market is shut.
    /// Equity feed: past this age, publish but set `MARKET_CLOSED`.
    /// Sized to the gap between updates while the market is open.
    pub market_closed_secs: u64,
    /// Equity feed: past this age, also set `REFERENCE_STALE`.
    /// Sized to span a long weekend, so ordinary closures do not trip it.
    pub reference_stale_secs: u64,
    /// Equity feed: past this age, refuse to publish at all. A reference price
    /// this old is not evidence of anything.
    pub reference_max_age_secs: u64,
    /// Token feed: past this age, publish but set `TOKEN_FEED_STALE`.
    pub token_stale_secs: u64,
    /// Token feed: past this age, refuse to publish. The token trades
    /// continuously, so a stale token feed means the oracle is broken.
    pub token_max_age_secs: u64,

    // ---- value policy ----
    /// Set `DEVIATION_HIGH` on both quotes when
    /// `|market - fair| / fair` exceeds this, in basis points.
    pub max_deviation_bps: u64,
    /// Refuse to publish a quote whose Pyth confidence interval is wider than
    /// this, in basis points of the price. An oracle this unsure is not
    /// publishing a price, and a wide band is how a bad value sneaks past a
    /// consumer that only reads the midpoint.
    pub max_conf_bps: u64,

    /// `sha256` of the frozen methodology document for this feed pair.
    pub methodology_hash: [u8; 32],

    /// Minimum Pyth verification level this symbol will accept.
    ///
    /// `1` (the default, and what the runbooks set) means **Full**: two thirds
    /// of the current guardian set have been verified. `0` means a partially
    /// verified update is acceptable.
    ///
    /// This is a field rather than a constant because the two ways to get a
    /// price on-chain give different answers. A sponsored feed that Pyth keeps
    /// updated is fully verified, and needs no posting at all. Posting an
    /// update yourself in a single transaction — `post_update_atomic` — checks
    /// only a subset of signatures and produces a *partial* update, so a
    /// deployment that has to post its own prices cannot also demand Full.
    ///
    /// Recorded on-chain so the trade-off is visible to a consumer rather than
    /// buried in a keeper's configuration: a feed running at level 0 is one
    /// where fewer colluding guardians could forge a price.
    pub min_verification_level: u8,

    /// Ticker, right-padded with zero bytes, e.g. `b"AAPL\0\0\0\0"`.
    pub symbol: [u8; SYMBOL_LEN],
    /// Decimals of the tokenized stock's mint, used to size one whole token.
    pub base_decimals: u8,
    /// Bump for the shared signing authority PDA.
    pub authority_bump: u8,
    /// Bump for this config PDA.
    pub bump: u8,
}

impl AdapterConfig {
    /// The symbol as text, for log messages. Trailing zero padding removed.
    ///
    /// Returns `None` if the stored bytes are not valid UTF-8, which cannot
    /// happen for a config created through `initialize` but is checked rather
    /// than assumed.
    #[must_use]
    pub fn symbol_str(&self) -> Option<&str> {
        let end = self
            .symbol
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(SYMBOL_LEN);
        core::str::from_utf8(self.symbol.get(..end)?).ok()
    }
}
