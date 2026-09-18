use {
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    svi_core::state::Quote,
};

const PROGRAM_ID: Pubkey = svi_core::ID;
const DISC: usize = 8;
pub const FEED_ID: [u8; 32] = [
    0x6a, 0xb8, 0x26, 0xe3, 0xa5, 0xd8, 0x0b, 0xc1, 0xca, 0xb3, 0x80, 0x47, 0x48, 0xea, 0x4f, 0x4d,
    0x27, 0xbb, 0x6d, 0xd4, 0x33, 0xe2, 0xd3, 0xba, 0xe1, 0xbc, 0x5b, 0x3f, 0x4d, 0x89, 0x5c, 0x86,
];

fn read_quote(svm: &LiteSVM, quote: Pubkey) -> Quote {
    let raw = svm.get_account(&quote).unwrap().data;
    assert_eq!(raw.len(), DISC + std::mem::size_of::<Quote>());
    // pod_read_unaligned copies; from_bytes would panic on an unaligned Vec buffer.
    bytemuck::pod_read_unaligned::<Quote>(&raw[DISC..DISC + std::mem::size_of::<Quote>()])
}

/// Consumers parse this account by byte offset. If this test fails, you have
/// silently broken every downstream reader — treat it as a breaking change.
#[test]
fn layout_is_frozen() {
    use std::mem::offset_of;
    assert_eq!(std::mem::size_of::<Quote>(), 320);
    assert_eq!(std::mem::align_of::<Quote>(), 8);
    assert_eq!(offset_of!(Quote, base_amount), 224);
    assert_eq!(offset_of!(Quote, observed_slot), 256);
    assert_eq!(offset_of!(Quote, published_slot), 272);
    assert_eq!(offset_of!(Quote, valid_until_slot), 280);
    assert_eq!(offset_of!(Quote, sequence), 288);
    assert_eq!(offset_of!(Quote, status_flags), 296);
    assert_eq!(offset_of!(Quote, quote_currency_code), 304);
}

struct Ctx {
    svm: LiteSVM,
    admin: Keypair,
    adapter: Keypair,
    descriptor: Pubkey,
    quote: Pubkey,
}

fn setup() -> Ctx {
    let admin = Keypair::new();
    let adapter = Keypair::new(); // stands in for the adapter PDA until Step 3
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/svi_core.so"
    ));
    svm.add_program(PROGRAM_ID, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    let (descriptor, _) = Pubkey::find_program_address(&[b"descriptor", &FEED_ID], &PROGRAM_ID);
    let (quote, _) = Pubkey::find_program_address(&[b"quote", &FEED_ID], &PROGRAM_ID);

    let params = svi_core::InitFeedParams {
        feed_id: FEED_ID,
        adapter_program: PROGRAM_ID,
        adapter_authority: adapter.pubkey(),
        adapter_config: Pubkey::default(),
        base_mint: Pubkey::new_unique(),
        quote_mint: Pubkey::default(),
        methodology_hash: [7u8; 32],
        max_age_slots: 200,       // ~60s at 300ms slots
        quote_currency_code: 840, // USD
        value_type: 1,            // ProtocolNav
        base_decimals: 9,
        quote_decimals: 6,
    };
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: svi_core::accounts::InitializeFeed {
            authority: admin.pubkey(),
            descriptor,
            quote,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: svi_core::instruction::InitializeFeed { p: params }.data(),
    };
    send(&mut svm, &[ix], &admin, &[&admin]).expect("initialize_feed should succeed");
    Ctx {
        svm,
        admin,
        adapter,
        descriptor,
        quote,
    }
}

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e.err))
}

fn publish(c: &mut Ctx, signer: &Keypair, u: svi_core::QuoteUpdate) -> Result<(), String> {
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: svi_core::accounts::PublishQuote {
            descriptor: c.descriptor,
            quote: c.quote,
            adapter_authority: signer.pubkey(),
        }
        .to_account_metas(None),
        data: svi_core::instruction::PublishQuote { u }.data(),
    };
    let admin = c.admin.insecure_clone();
    send(&mut c.svm, &[ix], &admin, &[&admin, signer])
}

fn good_update(slot: u64) -> svi_core::QuoteUpdate {
    svi_core::QuoteUpdate {
        base_amount: 1_000_000_000, // 1 xSOL
        quote_amount: 64_640,       // $0.064640
        lower_quote_amount: 64_600,
        upper_quote_amount: 64_685,
        observed_slot: slot,
        observed_unix_ts: 1_788_254_656,
        source_accounts_hash: [1u8; 32],
        status_flags: 0,
    }
}

#[test]
fn publish_happy_path() {
    let mut c = setup();

    let q = read_quote(&c.svm, c.quote);
    assert_eq!(q.sequence, 0, "never published yet");
    assert_eq!(q.methodology_hash, [7u8; 32], "identity stamped at init");

    c.svm.warp_to_slot(1000);
    let adapter = c.adapter.insecure_clone();
    publish(&mut c, &adapter, good_update(1000)).unwrap();

    let q = read_quote(&c.svm, c.quote);
    assert_eq!(q.sequence, 1);
    assert_eq!(q.quote_amount, 64_640);
    assert_eq!(q.observed_slot, 1000);
    assert_eq!(q.published_slot, 1000);
    assert_eq!(q.valid_until_slot, 1200, "observed_slot + max_age_slots");
    assert_eq!(q.value_type, 1);
    assert_eq!(q.quote_currency_code, 840);
}

#[test]
fn rejects_replay_of_same_slot() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);
    let adapter = c.adapter.insecure_clone();
    publish(&mut c, &adapter, good_update(1000)).unwrap();
    // Without this the second tx is byte-identical to the first, so the runtime
    // rejects it as AlreadyProcessed before our program ever runs.
    c.svm.expire_blockhash();
    let err = publish(&mut c, &adapter, good_update(1000)).unwrap_err();
    assert!(err.contains("6002"), "expected StaleObservation, got {err}");
}

#[test]
fn rejects_inverted_bounds() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);
    let adapter = c.adapter.insecure_clone();
    let mut bad = good_update(1000);
    bad.lower_quote_amount = 99_999_999;
    let err = publish(&mut c, &adapter, bad).unwrap_err();
    assert!(err.contains("6005"), "expected InvalidBounds, got {err}");
}

#[test]
fn rejects_wrong_signer() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);
    let impostor = Keypair::new();
    c.svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();
    let err = publish(&mut c, &impostor, good_update(1000)).unwrap_err();
    assert!(
        err.contains("6001"),
        "expected UnauthorizedAdapter, got {err}"
    );
}

#[test]
fn rejects_observation_from_future() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);
    let adapter = c.adapter.insecure_clone();
    let err = publish(&mut c, &adapter, good_update(1500)).unwrap_err();
    assert!(
        err.contains("6003"),
        "expected ObservationFromFuture, got {err}"
    );
}

#[test]
fn rejects_observation_too_old() {
    let mut c = setup();
    c.svm.warp_to_slot(2000);
    let adapter = c.adapter.insecure_clone();
    let err = publish(&mut c, &adapter, good_update(1000)).unwrap_err();
    assert!(
        err.contains("6004"),
        "expected ObservationTooOld, got {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// value_type admission
//
// `initialize_feed` is the only gate on `value_type`. A type the core does not
// know must be refused here, because nothing downstream re-checks it: the
// Quote stores a bare `u8` and a consumer maps it back by number.
// ─────────────────────────────────────────────────────────────────────────────

/// Try to create a feed with an arbitrary `value_type`, in a fresh VM.
fn try_init_feed(value_type: u8, feed_id: [u8; 32]) -> Result<(), String> {
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/svi_core.so"
    ));
    svm.add_program(PROGRAM_ID, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    let (descriptor, _) = Pubkey::find_program_address(&[b"descriptor", &feed_id], &PROGRAM_ID);
    let (quote, _) = Pubkey::find_program_address(&[b"quote", &feed_id], &PROGRAM_ID);

    let params = svi_core::InitFeedParams {
        feed_id,
        adapter_program: PROGRAM_ID,
        adapter_authority: Pubkey::new_unique(),
        adapter_config: Pubkey::default(),
        base_mint: Pubkey::new_unique(),
        quote_mint: Pubkey::default(),
        methodology_hash: [0u8; 32],
        max_age_slots: 200,
        quote_currency_code: 840,
        value_type,
        base_decimals: 6,
        quote_decimals: 9,
    };
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: svi_core::accounts::InitializeFeed {
            authority: admin.pubkey(),
            descriptor,
            quote,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: svi_core::instruction::InitializeFeed { p: params }.data(),
    };
    send(&mut svm, &[ix], &admin, &[&admin])
}

fn feed_id_for(tag: u8) -> [u8; 32] {
    let mut id = FEED_ID;
    id[0] = tag;
    id
}

/// Every discriminant the enum defines must be creatable. If this fails after
/// adding a variant, the `require!` bound in `initialize_feed` was not bumped.
#[test]
fn accepts_every_known_value_type() {
    for vt in 1..=(svi_core::state::ValueType::ReferenceFairValue as u8) {
        assert!(
            try_init_feed(vt, feed_id_for(vt)).is_ok(),
            "value_type {vt} should be accepted"
        );
    }
}

/// The tokenized-stock adapter publishes a reference fair value alongside a
/// market price, so this variant specifically has to be admissible.
#[test]
fn accepts_reference_fair_value() {
    assert_eq!(svi_core::state::ValueType::ReferenceFairValue as u8, 7);
    assert!(try_init_feed(7, feed_id_for(7)).is_ok());
}

#[test]
fn rejects_unknown_value_type() {
    let err = try_init_feed(8, feed_id_for(8)).unwrap_err();
    assert!(err.contains("6008"), "expected InvalidValueType, got {err}");
}

#[test]
fn rejects_zero_value_type() {
    let err = try_init_feed(0, feed_id_for(200)).unwrap_err();
    assert!(err.contains("6008"), "expected InvalidValueType, got {err}");
}
