//! Cranking and watching the tokenized-stock feeds.
//!
//! Kept in its own module so the Hylo path above is untouched. The two share
//! the quote decoder and nothing else: Hylo's feed derives every address from
//! one hardcoded program id, while a stock feed has a pair of quotes, a pair
//! of Pyth accounts and a per-symbol config, all of which are chosen at deploy
//! time and therefore read from `deployments.json` rather than derived here.
//!
//! # Where the Pyth price accounts come from
//!
//! This keeper does **not** post price updates. It reads price accounts that
//! already exist and lets the adapter verify them, which is the same shape as
//! the Hylo path (which reads a SOL/USD account it never wrote).
//!
//! That works when the symbol's feeds are *sponsored* — Pyth keeps a price
//! account updated at a known address, fully verified. If a feed is not
//! sponsored on your cluster, someone has to post updates for it, and that is
//! a separate job from cranking: posting a fully-verified update is a
//! multi-transaction Wormhole VAA flow, and the single-transaction shortcut
//! (`post_update_atomic`) yields only a *partial* verification, which the
//! adapter refuses unless that symbol's `min_verification_level` is 0.
//!
//! So: put the price account addresses in `deployments.json`, whatever their
//! provenance, and the adapter re-checks the feed id inside each one on every
//! refresh. Substituting a different account cannot go unnoticed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use anyhow::{anyhow, bail, Context, Result};

/// `sha256("global:refresh_stock")[..8]`.
pub const IX_REFRESH_STOCK: [u8; 8] = [53, 10, 191, 136, 12, 213, 123, 198];

/// Status flag bits 16-19, as allocated to the stock adapter in
/// `svi-core::state::flags`. Mirrored here for display only.
pub const FLAG_NAMES: [(u64, &str); 4] = [
    (1 << 16, "MARKET_CLOSED"),
    (1 << 17, "REFERENCE_STALE"),
    (1 << 18, "TOKEN_FEED_STALE"),
    (1 << 19, "DEVIATION_HIGH"),
];

/// Render a `status_flags` word using this adapter's bit allocation.
#[must_use]
pub fn flag_names(bits: u64) -> String {
    let set: Vec<&str> = FLAG_NAMES
        .iter()
        .filter(|(b, _)| bits & b != 0)
        .map(|(_, n)| *n)
        .collect();
    if set.is_empty() {
        "none".into()
    } else {
        set.join(" | ")
    }
}

/// One symbol's on-chain wiring, as recorded at deploy time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockFeed {
    pub symbol: String,
    pub adapter: Pubkey,
    pub svi_core: Pubkey,
    pub config: Pubkey,
    pub adapter_authority: Pubkey,
    pub fair_descriptor: Pubkey,
    pub fair_quote: Pubkey,
    pub market_descriptor: Pubkey,
    pub market_quote: Pubkey,
    pub equity_price: Pubkey,
    pub token_price: Pubkey,
    /// The Pyth feed ids this symbol's config will accept, as recorded at
    /// deploy time. Only the `--post-updates` path needs them: it asks Hermes
    /// for these exact feeds. The adapter checks them again on-chain.
    pub equity_feed_id: [u8; 32],
    pub token_feed_id: [u8; 32],
}

impl StockFeed {
    /// Accounts in the order `RefreshStock` declares them. Getting this order
    /// wrong is the classic silent failure, so it is written once, here.
    #[must_use]
    pub fn refresh_ix(&self, payer: &Pubkey) -> Instruction {
        self.refresh_ix_with(payer, self.equity_price, self.token_price)
    }

    /// The same instruction against price accounts other than the ones in
    /// `deployments.json`.
    ///
    /// Used by the `--post-updates` path, where the keeper creates the price
    /// accounts itself rather than reading Pyth's sponsored ones. The
    /// substitution is safe to allow here because the adapter does not trust
    /// these addresses: it re-reads the feed id inside each account and aborts
    /// on a mismatch, so passing the wrong one fails the transaction rather
    /// than publishing another asset's price.
    pub fn refresh_ix_with(
        &self,
        payer: &Pubkey,
        equity_price: Pubkey,
        token_price: Pubkey,
    ) -> Instruction {
        Instruction {
            program_id: self.adapter,
            accounts: vec![
                AccountMeta::new(*payer, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new_readonly(equity_price, false),
                AccountMeta::new_readonly(token_price, false),
                AccountMeta::new_readonly(self.fair_descriptor, false),
                AccountMeta::new(self.fair_quote, false),
                AccountMeta::new_readonly(self.market_descriptor, false),
                AccountMeta::new(self.market_quote, false),
                AccountMeta::new_readonly(self.adapter_authority, false),
                AccountMeta::new_readonly(self.svi_core, false),
            ],
            data: IX_REFRESH_STOCK.to_vec(),
        }
    }
}

/// Everything `deployments.json` records, keyed by symbol.
#[derive(Debug, Clone, Default)]
pub struct Deployments {
    pub cluster: String,
    pub feeds: BTreeMap<String, StockFeed>,
}

impl Deployments {
    pub fn get(&self, symbol: &str) -> Result<&StockFeed> {
        self.feeds.get(symbol).ok_or_else(|| {
            let known: Vec<&str> = self.feeds.keys().map(String::as_str).collect();
            anyhow!(
                "no symbol {symbol:?} in deployments.json. Known: {}",
                if known.is_empty() { "(none)".into() } else { known.join(", ") }
            )
        })
    }
}

/// Default location: `deployments.json` at the repository root.
#[must_use]
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("deployments.json")
}

/// Read one of the `feed_ids` entries: 32 bytes as 64 lowercase hex chars.
///
/// Strict about the length rather than tolerant: a short id that got padded
/// somewhere would name a different feed, and the adapter would reject it
/// on-chain with `PythFeedMismatch` — correct, but after a round trip and
/// without saying which of the two ids was malformed.
fn feed_id(v: &serde_json::Value, leg: &str) -> Result<[u8; 32]> {
    let s = v
        .get("feed_ids")
        .and_then(|f| f.get(leg))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing feed_ids.{leg}"))?;
    let raw = hex::decode(s.strip_prefix("0x").unwrap_or(s))
        .with_context(|| format!("feed_ids.{leg} is not hex: {s}"))?;
    raw.try_into()
        .map_err(|v: Vec<u8>| anyhow!("feed_ids.{leg} is {} bytes, expected 32", v.len()))
}

fn key(v: &serde_json::Value, field: &str) -> Result<Pubkey> {
    let s = v
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing string field {field:?}"))?;
    s.parse::<Pubkey>()
        .with_context(|| format!("field {field:?} is not a pubkey: {s}"))
}

/// Parse a `deployments.json` body.
///
/// Separated from file IO so it is testable without a fixture on disk — the
/// failure this guards against is a hand-edited file with one address wrong,
/// which would otherwise surface as an opaque transaction error.
pub fn parse(body: &str) -> Result<Deployments> {
    let root: serde_json::Value =
        serde_json::from_str(body).context("deployments.json is not valid JSON")?;

    let cluster = root
        .get("cluster")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let programs = root
        .get("programs")
        .ok_or_else(|| anyhow!("deployments.json has no \"programs\" object"))?;
    let svi_core = key(programs, "svi_core")?;
    let adapter = key(programs, "svi_stock_adapter")?;

    let stocks = root
        .get("stocks")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("deployments.json has no \"stocks\" object"))?;

    let mut feeds = BTreeMap::new();
    for (symbol, v) in stocks {
        let feed = StockFeed {
            symbol: symbol.clone(),
            adapter,
            svi_core,
            config: key(v, "config").with_context(|| format!("symbol {symbol}"))?,
            adapter_authority: key(v, "adapter_authority")
                .with_context(|| format!("symbol {symbol}"))?,
            fair_descriptor: key(v, "fair_descriptor")
                .with_context(|| format!("symbol {symbol}"))?,
            fair_quote: key(v, "fair_quote").with_context(|| format!("symbol {symbol}"))?,
            market_descriptor: key(v, "market_descriptor")
                .with_context(|| format!("symbol {symbol}"))?,
            market_quote: key(v, "market_quote").with_context(|| format!("symbol {symbol}"))?,
            equity_price: key(v, "equity_price").with_context(|| format!("symbol {symbol}"))?,
            token_price: key(v, "token_price").with_context(|| format!("symbol {symbol}"))?,
            equity_feed_id: feed_id(v, "equity").with_context(|| format!("symbol {symbol}"))?,
            token_feed_id: feed_id(v, "token").with_context(|| format!("symbol {symbol}"))?,
        };
        if feed.equity_feed_id == feed.token_feed_id {
            bail!("symbol {symbol}: both Pyth feed ids are the same, so drift is always zero");
        }
        if feed.fair_quote == feed.market_quote {
            bail!("symbol {symbol}: fair_quote and market_quote are the same account");
        }
        if feed.equity_price == feed.token_price {
            bail!("symbol {symbol}: equity_price and token_price are the same account");
        }
        feeds.insert(symbol.clone(), feed);
    }

    if feeds.is_empty() {
        bail!("deployments.json lists no symbols");
    }
    Ok(Deployments { cluster, feeds })
}

pub fn load(path: &Path) -> Result<Deployments> {
    let body = std::fs::read_to_string(path).with_context(|| {
        format!(
            "cannot read {}. Run the deploy runbook first, or pass --deployments <path>.",
            path.display()
        )
    })?;
    parse(&body)
}

/// Parse a `--feed` value. `hylo-xsol` keeps the original behaviour;
/// `stock:AAPL` selects a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSelector {
    HyloXsol,
    Stock(String),
}

impl FeedSelector {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "hylo-xsol" => Ok(Self::HyloXsol),
            other => match other.strip_prefix("stock:") {
                Some(sym) if !sym.is_empty() => Ok(Self::Stock(sym.to_ascii_uppercase())),
                _ => bail!(
                    "unknown --feed {other:?}. Use `hylo-xsol` or `stock:<SYM>`, e.g. stock:AAPL"
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "cluster": "devnet",
      "programs": {
        "svi_core": "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH",
        "svi_stock_adapter": "3RmdZomoBXkWwGwvdjWK8ELYB4XeqHxTmrzedcmucHxq"
      },
      "stocks": {
        "AAPL": {
          "feed_ids": {
            "equity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "token": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
          },
          "config": "11111111111111111111111111111112",
          "adapter_authority": "11111111111111111111111111111113",
          "fair_descriptor": "11111111111111111111111111111114",
          "fair_quote": "11111111111111111111111111111115",
          "market_descriptor": "11111111111111111111111111111116",
          "market_quote": "11111111111111111111111111111117",
          "equity_price": "11111111111111111111111111111118",
          "token_price": "11111111111111111111111111111119"
        }
      }
    }"#;

    #[test]
    fn parses_a_well_formed_file() {
        let d = parse(SAMPLE).unwrap();
        assert_eq!(d.cluster, "devnet");
        let f = d.get("AAPL").unwrap();
        assert_eq!(f.symbol, "AAPL");
        assert_ne!(f.fair_quote, f.market_quote);
    }

    /// The account list is the contract with the program. Order and
    /// writability are both part of it: a read-only quote account would fail
    /// at runtime with a permissions error rather than anything descriptive.
    #[test]
    fn refresh_instruction_matches_the_programs_account_order() {
        let d = parse(SAMPLE).unwrap();
        let f = d.get("AAPL").unwrap();
        let payer = Pubkey::new_unique();
        let ix = f.refresh_ix(&payer);

        assert_eq!(ix.program_id, f.adapter);
        assert_eq!(ix.data, IX_REFRESH_STOCK.to_vec());
        assert_eq!(ix.accounts.len(), 10);

        let keys: Vec<Pubkey> = ix.accounts.iter().map(|a| a.pubkey).collect();
        assert_eq!(
            keys,
            vec![
                payer,
                f.config,
                f.equity_price,
                f.token_price,
                f.fair_descriptor,
                f.fair_quote,
                f.market_descriptor,
                f.market_quote,
                f.adapter_authority,
                f.svi_core,
            ]
        );
        // Only the payer signs; only the two quotes are written.
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts.iter().skip(1).all(|a| !a.is_signer));
        let writable: Vec<usize> = ix
            .accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_writable)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(writable, vec![0, 5, 7], "payer, fair_quote, market_quote");
    }

    #[test]
    fn rejects_a_file_whose_pair_is_the_same_account() {
        let bad = SAMPLE.replace(
            "\"market_quote\": \"11111111111111111111111111111117\"",
            "\"market_quote\": \"11111111111111111111111111111115\"",
        );
        let err = parse(&bad).unwrap_err().to_string();
        assert!(err.contains("same account"), "got {err}");
    }

    #[test]
    fn a_bad_address_names_the_symbol_and_the_field() {
        let bad = SAMPLE.replace("11111111111111111111111111111114", "not-a-pubkey");
        let err = format!("{:#}", parse(&bad).unwrap_err());
        assert!(err.contains("AAPL"), "got {err}");
        assert!(err.contains("fair_descriptor"), "got {err}");
    }

    #[test]
    fn unknown_symbol_lists_what_is_available() {
        let d = parse(SAMPLE).unwrap();
        let err = d.get("TSLA").unwrap_err().to_string();
        assert!(err.contains("AAPL"), "got {err}");
    }

    #[test]
    fn missing_sections_are_named() {
        assert!(parse("{}").unwrap_err().to_string().contains("programs"));
        let no_stocks = r#"{"programs":{"svi_core":"11111111111111111111111111111112",
            "svi_stock_adapter":"11111111111111111111111111111113"}}"#;
        assert!(parse(no_stocks).unwrap_err().to_string().contains("stocks"));
    }

    #[test]
    fn feed_selector_parsing() {
        assert_eq!(FeedSelector::parse("hylo-xsol").unwrap(), FeedSelector::HyloXsol);
        assert_eq!(
            FeedSelector::parse("stock:aapl").unwrap(),
            FeedSelector::Stock("AAPL".into())
        );
        assert!(FeedSelector::parse("stock:").is_err());
        assert!(FeedSelector::parse("nonsense").is_err());
    }

    /// The override must replace exactly the two price slots and nothing
    /// else. Getting this wrong would send the refresh at the right accounts
    /// in the wrong order, which fails as a deserialisation error naming no
    /// account.
    #[test]
    fn overriding_the_price_accounts_touches_only_those_two_slots() {
        let f = parse(SAMPLE).unwrap().get("AAPL").unwrap().clone();
        let payer = Pubkey::new_unique();
        let (eq, tok) = (Pubkey::new_unique(), Pubkey::new_unique());

        let base = f.refresh_ix(&payer);
        let over = f.refresh_ix_with(&payer, eq, tok);

        assert_eq!(base.data, over.data);
        assert_eq!(base.accounts.len(), over.accounts.len());
        assert_eq!(over.accounts[2].pubkey, eq);
        assert_eq!(over.accounts[3].pubkey, tok);
        for i in (0..base.accounts.len()).filter(|i| *i != 2 && *i != 3) {
            assert_eq!(base.accounts[i].pubkey, over.accounts[i].pubkey, "slot {i}");
            assert_eq!(base.accounts[i].is_writable, over.accounts[i].is_writable);
        }
    }

    #[test]
    fn feed_ids_are_parsed_as_32_bytes_and_a_wrong_length_is_named() {
        let f = parse(SAMPLE).unwrap().get("AAPL").unwrap().clone();
        assert_eq!(f.equity_feed_id, [0xaa; 32]);
        assert_eq!(f.token_feed_id, [0xbb; 32]);

        // 31 bytes: valid hex, wrong length. Must be refused, not zero-extended
        // into a different feed.
        let short = SAMPLE.replace("\"equity\": \"aa", "\"equity\": \"");
        let err = format!("{:#}", parse(&short).unwrap_err());
        assert!(err.contains("31 bytes") && err.contains("32"), "{err}");

        // Not hex at all is caught earlier, and says so.
        let junk = SAMPLE.replace("\"equity\": \"aa", "\"equity\": \"zz");
        assert!(format!("{:#}", parse(&junk).unwrap_err()).contains("not hex"));

        // Two identical legs can only ever report zero drift.
        let same = SAMPLE.replace("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                                  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(parse(&same).is_err());
    }

    #[test]
    fn flag_rendering() {
        assert_eq!(flag_names(0), "none");
        assert_eq!(flag_names(1 << 16), "MARKET_CLOSED");
        assert_eq!(
            flag_names((1 << 16) | (1 << 19)),
            "MARKET_CLOSED | DEVIATION_HIGH"
        );
        // Hylo's bits 0-5 are not ours and must not render as our names.
        assert_eq!(flag_names(0b11_1111), "none");
    }
}

/// Decode a `StockAdapterError` code into the thing an operator should do.
///
/// Anchor numbers these from 6000 in declaration order; the list is
/// append-only in the program for exactly this reason.
#[must_use]
pub fn explain_stock(err: &str) -> String {
    let code = err
        .find("custom program error: 0x")
        .and_then(|i| {
            err.get(i + 24..)?
                .split(|c: char| !c.is_ascii_hexdigit())
                .next()
        })
        .and_then(|h| u32::from_str_radix(h, 16).ok());
    match code {
        Some(6000) => "PythWrongOwner (6000): that account is not owned by the Pyth receiver — wrong address in deployments.json".into(),
        Some(6001) => "PythMalformed (6001): account does not deserialize as PriceUpdateV2".into(),
        Some(6002) => "PythNotFullyVerified (6002): update is only partially verified. Either use a sponsored feed, or initialize this symbol with min_verification_level = 0 and accept the weaker guarantee".into(),
        Some(6003) => "PythFeedMismatch (6003): the account carries a different feed id than the config — the price account and the feed id disagree".into(),
        Some(6004) => "PythInvalidPublishTime (6004): non-positive publish time".into(),
        Some(6005) => "PythFromFuture (6005): publish time is ahead of the chain clock".into(),
        Some(6006) => "PythTooOld (6006): past the hard age limit. For the equity leg this means longer than reference_max_age_secs — raise it, or the feed really has stopped".into(),
        Some(6007) => "PythPriceOutOfRange (6007): price is not strictly positive, or does not fit 9 decimals".into(),
        Some(6008) => "PythConfidenceTooWide (6008): Pyth's confidence interval exceeds max_conf_bps".into(),
        Some(6009) => "WrongCoreProgram (6009): svi_core_program account does not match the config".into(),
        Some(6010) => "WrongFeedAccount (6010): a descriptor or quote account does not match the config".into(),
        Some(6011) => "FeedPairNotDistinct (6011): the two feeds of the pair are the same".into(),
        Some(6012) => "InvalidSymbol (6012): symbol must be non-empty alphanumeric ASCII".into(),
        Some(6013) => "InvalidThresholds (6013): staleness windows must widen, closed <= stale <= max".into(),
        Some(6014) => "InvalidTolerance (6014): a bps tolerance or decimals value is out of range".into(),
        Some(6015) => "MathOverflow (6015)".into(),
        Some(c) => format!("program error {c}"),
        None => err.lines().next().unwrap_or(err).to_string(),
    }
}

#[cfg(test)]
mod explain_tests {
    use super::*;

    #[test]
    fn decodes_the_codes_an_operator_will_actually_hit() {
        // 0x1772 == 6002
        let s = explain_stock("Error processing Instruction 0: custom program error: 0x1772");
        assert!(s.contains("PythNotFullyVerified"), "{s}");
        assert!(s.contains("min_verification_level"), "should say what to do: {s}");

        // 0x1773 == 6003
        assert!(explain_stock("custom program error: 0x1773").contains("PythFeedMismatch"));
        // 0x1776 == 6006
        assert!(explain_stock("custom program error: 0x1776").contains("PythTooOld"));
    }

    #[test]
    fn passes_through_anything_it_cannot_decode() {
        assert_eq!(explain_stock("blockhash not found"), "blockhash not found");
    }
}
