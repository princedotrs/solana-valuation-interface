use anchor_lang::prelude::*;

pub const QUOTE_LAYOUT_VERSION: u8 = 1;

// ---- Value semantics. A consumer asking for NAV must never silently get a market price. ----
#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[borsh(use_discriminant = true)]
pub enum ValueType {
    ProtocolNav = 1,  // xSOL's share of Hylo collateral, per Hylo's own math
    Redemption = 2,   // what you actually receive redeeming right now, after fees
    ExchangeRate = 3, // eHYUSD -> hyUSD
    BackingNav = 4,   // hyUSD backing value
    MarketSpot = 5,   // DEX price. NEVER use for collateral.
    MarketTwap = 6,
    /// The value of the asset a token *tracks*, not of the token itself.
    ///
    /// For a tokenized stock this is the price of the underlying equity, from
    /// an equity oracle — what the token is supposed to be worth. Pair it with
    /// a `MarketSpot` feed for the same symbol to see what the token actually
    /// trades at, and the gap between them is the premium or discount.
    ///
    /// A consumer must not treat this as the redemption value of the token:
    /// the underlying equity market can be closed while the token keeps
    /// trading, which is exactly what the `MARKET_CLOSED` flag reports.
    ReferenceFairValue = 7,
}

#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[borsh(use_discriminant = true)]
pub enum FeedStatus {
    Active = 0,
    Frozen = 1,
    Deprecated = 2,
}

// ---- status_flags bitfield on the Quote ----
//
// # Bit allocation registry
//
// `status_flags` is a `u64` shared by every adapter, and the core never
// interprets it — it stores what the adapter sends. That makes collisions
// silent and dangerous: two adapters using bit 3 for different things would
// make a consumer's check mean one thing on one feed and another elsewhere.
//
// So ranges are allocated here, once, and an adapter owns its range:
//
// | bits | owner | meaning |
// |---|---|---|
// | 0-15 | protocol-NAV adapters | collateral and solvency states (below) |
// | 16-23 | `svi-stock-adapter` | reference-market and deviation states |
// | 24-63 | unallocated | claim a range here before using it |
//
// An adapter defines its own constants in its own `constants.rs` — it cannot
// import these, because adapters do not link the core (see `svi-abi`). The
// registry is the thing that keeps the hand-copied values from colliding.
pub mod flags {
    // ---- bits 0-15: protocol-NAV adapters (svi-hylo-adapter uses 0-5) ----
    pub const ZERO_SUPPLY_DEFAULT: u64 = 1 << 0;
    pub const DESTABILIZED: u64 = 1 << 1;
    pub const OPERATIONS_HALTED: u64 = 1 << 2;
    pub const SELL_ZONE: u64 = 1 << 3;
    pub const BUY_ZONE: u64 = 1 << 4;
    pub const EPOCH_BOUNDARY_CPI: u64 = 1 << 5;

    // ---- bits 16-23: svi-stock-adapter ----
    // Mirrored in that program's `constants.rs`; the values are part of the
    // published format, not an implementation detail of either program.

    /// The underlying equity market is not open, so the reference price is a
    /// last-close value rather than a live one. The token keeps trading.
    pub const MARKET_CLOSED: u64 = 1 << 16;
    /// The equity feed has not updated within the adapter's configured window.
    pub const REFERENCE_STALE: u64 = 1 << 17;
    /// The tokenized-stock feed has not updated within that window.
    pub const TOKEN_FEED_STALE: u64 = 1 << 18;
    /// Token price and reference price disagree by more than the configured
    /// tolerance. Set on BOTH quotes of the pair, so a consumer reading either
    /// one alone still sees it.
    pub const DEVIATION_HIGH: u64 = 1 << 19;

    // ---- bits 32-39: RESERVED, cross-cutting ----
    //
    // Not allocated to any adapter, because the conditions here are about the
    // relationship between an adapter and its inputs rather than about the
    // asset. Both existing adapters would set the same bit for the same
    // reason, so putting it in either private range would force the other to
    // duplicate it at a different offset and make one flag mean two things.
    //
    // Reserved rather than defined: nothing sets these yet, and a constant in
    // KNOWN_MASK that no adapter can produce would tell a consumer this core
    // understands a signal it has never seen.
    //
    //   bit 32  ORACLE_DIVERGENT -- the upstream oracle this adapter is
    //           required to use disagrees with an independent reference by
    //           more than a configured tolerance. NOT a refusal: the value
    //           stays correct with respect to the protocol that will honour
    //           it, and the flag says its input looks wrong. See
    //           docs/product/05-gtm.md.
    //   bits 33-39  unallocated.

    /// Every bit any adapter may currently set. A bit outside this mask means
    /// a newer adapter than this core knows about.
    pub const KNOWN_MASK: u64 = 0b11_1111 | (0b1111 << 16);
}

/// One per feed. Created by the admin, read on every publish. The rules live here,
/// so the Quote account can stay pure data.
#[account]
#[derive(InitSpace)]
pub struct Descriptor {
    pub feed_id: [u8; 32],         // sha256("hylo-xsol-nav-v1"), the feed's name
    pub authority: Pubkey,         // admin: may freeze / retire this feed
    pub adapter_program: Pubkey,   // informational: which program computes it
    pub adapter_authority: Pubkey, // THE security boundary: only this key may publish
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey, // Pubkey::default() when the quote unit is fiat
    pub methodology_hash: [u8; 32],
    pub adapter_config: Pubkey, // adapter's own config (pinned SDK rev, prog hash)
    pub max_age_slots: u64,     // how long a published quote stays valid
    pub quote_currency_code: u16, // ISO-4217 numeric: 840 = USD
    pub value_type: u8,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub status: u8,
    pub version: u8,
    pub bump: u8,
}

/// THE PRODUCT. 320 bytes, fixed layout, cheap for anyone to read.
#[account(zero_copy)]
#[repr(C)]
pub struct Quote {
    // --- 7 x 32 = 224 bytes ---
    pub descriptor: Pubkey,
    pub feed_id: [u8; 32],
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub adapter_program: Pubkey,
    pub methodology_hash: [u8; 32],
    pub source_accounts_hash: [u8; 32],

    // --- the value: 4 x 8 = 32 bytes ---
    pub base_amount: u64,
    pub quote_amount: u64,
    pub lower_quote_amount: u64,
    pub upper_quote_amount: u64,

    // --- time and ordering: 6 x 8 = 48 bytes ---
    pub observed_slot: u64,
    pub observed_unix_ts: i64,
    pub published_slot: u64,
    pub valid_until_slot: u64,
    pub sequence: u64,
    pub status_flags: u64,

    // --- small fields + explicit padding: 16 bytes ---
    pub quote_currency_code: u16,
    pub value_type: u8,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub version: u8,
    pub bump: u8,
    pub _padding: [u8; 9],
}

// Fails the build if the layout ever drifts. Consumers depend on these exact offsets.
const _: () = assert!(core::mem::size_of::<Quote>() == 320);
const _: () = assert!(core::mem::align_of::<Quote>() == 8);

#[cfg(test)]
mod tests {
    use super::*;

    /// The discriminants are the published format: a consumer stores "this feed
    /// is value_type 7" and maps it back. Renumbering would silently change
    /// what every existing descriptor means.
    #[test]
    fn value_type_discriminants_are_pinned() {
        assert_eq!(ValueType::ProtocolNav as u8, 1);
        assert_eq!(ValueType::Redemption as u8, 2);
        assert_eq!(ValueType::ExchangeRate as u8, 3);
        assert_eq!(ValueType::BackingNav as u8, 4);
        assert_eq!(ValueType::MarketSpot as u8, 5);
        assert_eq!(ValueType::MarketTwap as u8, 6);
        assert_eq!(ValueType::ReferenceFairValue as u8, 7);
    }

    /// `initialize_feed` validates `1..=ReferenceFairValue`. If a variant is
    /// added without touching that bound, the new type is rejected at feed
    /// creation and the failure looks like a client bug. This pins the two
    /// together.
    #[test]
    fn highest_value_type_is_the_one_initialize_feed_accepts() {
        assert_eq!(ValueType::ReferenceFairValue as u8, 7, "bump the require! in lib.rs too");
    }

    /// Ranges are allocated in the registry above. An overlap would make a
    /// consumer's flag check mean different things on different feeds.
    #[test]
    fn stock_flags_do_not_collide_with_protocol_nav_flags() {
        let protocol_nav = flags::ZERO_SUPPLY_DEFAULT
            | flags::DESTABILIZED
            | flags::OPERATIONS_HALTED
            | flags::SELL_ZONE
            | flags::BUY_ZONE
            | flags::EPOCH_BOUNDARY_CPI;
        let stock = flags::MARKET_CLOSED
            | flags::REFERENCE_STALE
            | flags::TOKEN_FEED_STALE
            | flags::DEVIATION_HIGH;

        assert_eq!(protocol_nav & stock, 0, "flag ranges overlap");
        assert_eq!(protocol_nav, 0b11_1111, "protocol-NAV adapters own bits 0-5");
        assert_eq!(stock, 0b1111 << 16, "the stock adapter owns bits 16-19");
        assert_eq!(flags::KNOWN_MASK, protocol_nav | stock);
    }

    /// Bits 32-39 are reserved for cross-cutting conditions and must stay
    /// outside KNOWN_MASK until an adapter can actually set one. Without this,
    /// a later range could be allocated over the reservation and two unrelated
    /// meanings would share a bit -- which no test would catch, because each
    /// adapter would be internally consistent.
    #[test]
    fn the_cross_cutting_range_is_still_reserved() {
        let reserved: u64 = 0xFF << 32;
        assert_eq!(flags::KNOWN_MASK & reserved, 0,
            "bits 32-39 are reserved; KNOWN_MASK must not claim them yet");
    }

    /// Bits 16-23 are the stock adapter's whole allocation; 20-23 are its room
    /// to grow without asking the core for another range.
    #[test]
    fn stock_flags_sit_inside_their_allocated_range() {
        let allocated: u64 = 0xFF << 16;
        for f in [
            flags::MARKET_CLOSED,
            flags::REFERENCE_STALE,
            flags::TOKEN_FEED_STALE,
            flags::DEVIATION_HIGH,
        ] {
            assert_eq!(f & allocated, f, "flag {f:#x} escapes bits 16-23");
        }
    }

    #[test]
    fn quote_layout_is_320_bytes() {
        assert_eq!(core::mem::size_of::<Quote>(), 320);
        assert_eq!(core::mem::align_of::<Quote>(), 8);
    }
}
