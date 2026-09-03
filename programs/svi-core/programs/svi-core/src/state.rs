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
pub mod flags {
    pub const ZERO_SUPPLY_DEFAULT: u64 = 1 << 0;
    pub const DESTABILIZED: u64 = 1 << 1;
    pub const OPERATIONS_HALTED: u64 = 1 << 2;
    pub const SELL_ZONE: u64 = 1 << 3;
    pub const BUY_ZONE: u64 = 1 << 4;
    pub const EPOCH_BOUNDARY_CPI: u64 = 1 << 5;
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
