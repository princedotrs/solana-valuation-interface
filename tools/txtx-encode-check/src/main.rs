//! Runs txtx 0.3.8's real borsh encoder against the real IDLs with the exact
//! Value shapes the runbook produces, so the encoding question is settled
//! here instead of on the user's machine.
use std::panic::{catch_unwind, AssertUnwindSafe};
use txtx_addon_kit::indexmap::IndexMap;
use txtx_addon_kit::types::types::Value;
use txtx_addon_network_svm::codec::idl::IdlRef;
use txtx_addon_network_svm::typing::SvmValue;

const CORE: &str = "H6495aW1Cxyoz2R7MuYVHF3Pu3FumFE9cZwKSJCR8oHH";
const ADAPTER: &str = "FfK5xyE3v3GT2F9wzqhpLMHx7zCJsGPQrHAvxtgL3Mpr";
const AUTHORITY_PDA: &str = "288zBibxcA9UY7cb2vywykWTnKcR1s8j1boeswP1Hzzu";
const CONFIG_PDA: &str = "G2yAbxeiLZYo1akLrqRndRtfTBux1rYWgjvoBFTRoJyx";
const DESCRIPTOR_PDA: &str = "F4shxf8FN6jnp2cu7Rcv5MjZGzvpeGpQzsRbxbJFJjsa";
const QUOTE_PDA: &str = "G28Ba5F9X71Pih2SNobXyU98tjQdga1Ztbzqg3PU4rWj";
const XSOL: &str = "4sWNB8zGWHkh6UnmwiEtzNxL4XrN7uK9tosbESbJFfVs";
const FEED_ID_HEX: &str = "6ab826e3a5d80bc1cab3804748ea4f4d27bb6dd433e2d3bae1bc5b3f4d895c86";

fn s(v: &str) -> Value { Value::string(v.to_string()) }
fn n(v: i128) -> Value { Value::integer(v) }
fn bytes_array(b: &[u8]) -> Value { Value::array(b.iter().map(|x| n(*x as i128)).collect()) }
/// What deploy_program actually stores in `program_id` (deploy_program.rs:394).
fn program_id_addon(b58: &str) -> Value { SvmValue::pubkey(bs58::decode(b58).into_vec().unwrap()) }
/// What std::encode_base58 does to that Addon (base58.rs get_bytes_for_encoding).
fn encode_base58(v: &Value) -> Value {
    let bytes = match v { Value::Addon(a) => a.bytes.clone(), _ => panic!("harness only models the Addon case") };
    Value::string(bs58::encode(bytes).into_string())
}
fn obj(fields: Vec<(&str, Value)>) -> Value {
    let mut m = IndexMap::new();
    for (k, v) in fields { m.insert(k.to_string(), v); }
    Value::object(m)
}
fn pk(b58: &str) -> Vec<u8> { bs58::decode(b58).into_vec().unwrap() }

fn init_feed_args(adapter_program: Value, quote_mint: Value) -> Vec<Value> {
    vec![obj(vec![
        ("feed_id", bytes_array(&hex::decode(FEED_ID_HEX).unwrap())),
        ("adapter_program", adapter_program),
        ("adapter_authority", s(AUTHORITY_PDA)),
        ("adapter_config", s(CONFIG_PDA)),
        ("base_mint", s(XSOL)),
        ("quote_mint", quote_mint),
        ("methodology_hash", bytes_array(&[0u8; 32])),
        ("max_age_slots", n(750)),
        ("quote_currency_code", n(840)),
        ("value_type", n(1)),
        ("base_decimals", n(6)),
        ("quote_decimals", n(9)),
    ])]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let core = IdlRef::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let adapter = IdlRef::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
    let mut failures = 0;
    // The negative cases below panic on purpose; keep their backtraces out of
    // the report.
    std::panic::set_hook(Box::new(|_| {}));

    // ---- the runbook as it will be committed ------------------------------
    let good = init_feed_args(
        encode_base58(&program_id_addon(ADAPTER)),
        s("0x0000000000000000000000000000000000000000000000000000000000000000"),
    );
    let mut expect = Vec::new();
    expect.extend(hex::decode(FEED_ID_HEX).unwrap());
    expect.extend(pk(ADAPTER)); expect.extend(pk(AUTHORITY_PDA)); expect.extend(pk(CONFIG_PDA));
    expect.extend(pk(XSOL)); expect.extend([0u8; 32]); expect.extend([0u8; 32]);
    expect.extend(750u64.to_le_bytes()); expect.extend(840u16.to_le_bytes());
    expect.extend([1u8, 6, 9]);
    match core.get_encoded_args("initialize_feed", good) {
        Ok(enc) if enc == expect => println!("PASS initialize_feed: {} bytes, byte-identical to hand-built borsh", enc.len()),
        Ok(enc) => { failures += 1; println!("FAIL initialize_feed: encoded {} bytes, expected {}\n  got {:?}", enc.len(), expect.len(), enc); }
        Err(e) => { failures += 1; println!("FAIL initialize_feed: {e}"); }
    }

    let good2 = vec![obj(vec![
        ("svi_core_program", encode_base58(&program_id_addon(CORE))),
        ("descriptor", s(DESCRIPTOR_PDA)),
        ("quote", s(QUOTE_PDA)),
        ("methodology_hash", bytes_array(&[0u8; 32])),
        ("sdk_revision", bytes_array(&hex::decode("ee8d1cba023ae26d5e81c77ccab52a4f71c44a3f").unwrap())),
        ("max_band_bps", n(500)),
    ])];
    let mut expect2 = Vec::new();
    expect2.extend(pk(CORE)); expect2.extend(pk(DESCRIPTOR_PDA)); expect2.extend(pk(QUOTE_PDA));
    expect2.extend([0u8; 32]); expect2.extend(hex::decode("ee8d1cba023ae26d5e81c77ccab52a4f71c44a3f").unwrap());
    expect2.extend(500u64.to_le_bytes());
    match adapter.get_encoded_args("initialize", good2) {
        Ok(enc) if enc == expect2 => println!("PASS initialize: {} bytes, byte-identical to hand-built borsh", enc.len()),
        Ok(enc) => { failures += 1; println!("FAIL initialize: encoded {} bytes, expected {}", enc.len(), expect2.len()); }
        Err(e) => { failures += 1; println!("FAIL initialize: {e}"); }
    }
    match adapter.get_encoded_args("refresh_xsol_nav", vec![]) {
        Ok(enc) if enc.is_empty() => println!("PASS refresh_xsol_nav: no args"),
        other => { failures += 1; println!("FAIL refresh_xsol_nav: {other:?}"); }
    }

    // ---- the two forms that were committed before, to prove the diagnosis --
    let r = catch_unwind(AssertUnwindSafe(|| {
        core.get_encoded_args("initialize_feed", init_feed_args(program_id_addon(ADAPTER), s("0x0000000000000000000000000000000000000000000000000000000000000000")))
    }));
    match r {
        Err(_) => println!("PASS (negative) raw Addon program_id panics -> 'not yet implemented' at idl/mod.rs:539, as observed"),
        Ok(x) => { failures += 1; println!("FAIL (negative) raw Addon program_id did not panic: {x:?}"); }
    }
    let r = catch_unwind(AssertUnwindSafe(|| {
        core.get_encoded_args("initialize_feed", init_feed_args(encode_base58(&program_id_addon(ADAPTER)), s("11111111111111111111111111111111")))
    }));
    match r {
        Err(_) => println!("PASS (negative) quote_mint \"111...1\" panics: 32 hex-looking chars -> 16 bytes -> hex[0..32] out of range"),
        Ok(Err(e)) => println!("NOTE (negative) quote_mint \"111...1\" errors instead of panicking: {e}"),
        Ok(Ok(_)) => { failures += 1; println!("FAIL (negative) quote_mint \"111...1\" was accepted; the hex-collision theory is wrong"); }
    }

    if failures > 0 { std::process::exit(1); }
}
