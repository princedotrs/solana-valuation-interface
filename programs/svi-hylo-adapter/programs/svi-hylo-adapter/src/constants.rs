use anchor_lang::prelude::*;

/// Seed for the adapter's config account.
#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

/// Seed for the adapter's signing authority.
///
/// This PDA is the entire security model. Only this program can sign for it,
/// so `svi-core` accepting it as the writer pins the published value's
/// provenance to *this code*, not to any key a human holds.
#[constant]
pub const AUTHORITY_SEED: &[u8] = b"authority";

/// xSOL has 6 decimals, so one whole xSOL is 10^6 base units.
///
/// The quote is published as "1 whole xSOL is worth N", which makes the
/// conversion exact: `hylo-core` returns NAV as USD-per-whole-xSOL in
/// `UFix64<N9>`, and its raw bits ARE the quote amount at 9 decimals. No
/// rescaling, so no rounding, so no rounding bug.
pub const XSOL_BASE_AMOUNT: u64 = 1_000_000;

/// Decimals of the published quote. Matches `UFix64<N9>` exactly.
pub const QUOTE_DECIMALS: u8 = 9;

/// ISO-4217 numeric code for USD.
pub const USD_CURRENCY_CODE: u16 = 840;

/// Status flags mirrored from `svi-core::state::flags`.
///
/// Duplicated rather than imported because this program cannot depend on
/// `svi-core` (different `anchor-lang` major version — see `cpi.rs`). The
/// values are part of the frozen wire format, not an implementation detail.
pub mod flags {
    pub const ZERO_SUPPLY_DEFAULT: u64 = 1 << 0;
    pub const DESTABILIZED: u64 = 1 << 1;
    pub const OPERATIONS_HALTED: u64 = 1 << 2;
    pub const SELL_ZONE: u64 = 1 << 3;
    pub const BUY_ZONE: u64 = 1 << 4;
    pub const EPOCH_BOUNDARY_CPI: u64 = 1 << 5;
}
