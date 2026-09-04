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

const CORE_ID: Pubkey = svi_core::ID;
const ADAPTER_ID: Pubkey = svi_mock_adapter::ID;
const DISC: usize = 8;

pub const FEED_ID: [u8; 32] = [
    0x6a, 0xb8, 0x26, 0xe3, 0xa5, 0xd8, 0x0b, 0xc1, 0xca, 0xb3, 0x80, 0x47, 0x48, 0xea, 0x4f, 0x4d,
    0x27, 0xbb, 0x6d, 0xd4, 0x33, 0xe2, 0xd3, 0xba, 0xe1, 0xbc, 0x5b, 0x3f, 0x4d, 0x89, 0x5c, 0x86,
];

fn read_quote(svm: &LiteSVM, quote: Pubkey) -> Quote {
    let raw = svm.get_account(&quote).unwrap().data;
    bytemuck::pod_read_unaligned::<Quote>(&raw[DISC..DISC + std::mem::size_of::<Quote>()])
}

fn send(
    svm: &mut LiteSVM, ixs: &[Instruction], payer: &Keypair, signers: &[&Keypair],
) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e.err))
}

struct Ctx {
    svm: LiteSVM,
    admin: Keypair,
    descriptor: Pubkey,
    quote: Pubkey,
    adapter_authority: Pubkey,
}

fn setup() -> Ctx {
    let mut svm = LiteSVM::new();
    let admin = Keypair::new();
    svm.add_program(
        CORE_ID,
        // svi-core is a separate Anchor workspace, so its artifact lands in its
        // own target/deploy. Build it before running these tests.
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../../../svi-core/target/deploy/svi_core.so"
        )),
    )
    .unwrap();
    svm.add_program(
        ADAPTER_ID,
        include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/svi_mock_adapter.so")),
    )
    .unwrap();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    let (descriptor, _) = Pubkey::find_program_address(&[b"descriptor", &FEED_ID], &CORE_ID);
    let (quote, _) = Pubkey::find_program_address(&[b"quote", &FEED_ID], &CORE_ID);
    // The adapter's PDA. Nobody holds its private key - it does not have one.
    let (adapter_authority, _) =
        Pubkey::find_program_address(&[svi_mock_adapter::AUTHORITY_SEED], &ADAPTER_ID);

    let params = svi_core::InitFeedParams {
        feed_id: FEED_ID,
        adapter_program: ADAPTER_ID,
        adapter_authority, // <-- the PDA, not a wallet
        adapter_config: Pubkey::default(),
        base_mint: Pubkey::new_unique(),
        quote_mint: Pubkey::default(),
        methodology_hash: [7u8; 32],
        max_age_slots: 200,
        quote_currency_code: 840,
        value_type: 1,
        base_decimals: 9,
        quote_decimals: 6,
    };
    let ix = Instruction {
        program_id: CORE_ID,
        accounts: svi_core::accounts::InitializeFeed {
            authority: admin.pubkey(),
            descriptor,
            quote,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: svi_core::instruction::InitializeFeed { p: params }.data(),
    };
    send(&mut svm, &[ix], &admin, &[&admin]).expect("initialize_feed");
    Ctx { svm, admin, descriptor, quote, adapter_authority }
}

fn publish_mock(
    c: &mut Ctx, payer: &Keypair, quote_amount: u64, spread_bps: u64,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: ADAPTER_ID,
        accounts: svi_mock_adapter::accounts::PublishMock {
            descriptor: c.descriptor,
            quote: c.quote,
            adapter_authority: c.adapter_authority,
            svi_core_program: CORE_ID,
        }
        .to_account_metas(None),
        data: svi_mock_adapter::instruction::PublishMock { quote_amount, spread_bps }.data(),
    };
    send(&mut c.svm, &[ix], payer, &[payer])
}

#[test]
fn adapter_pda_can_publish_through_core() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);

    let admin = c.admin.insecure_clone();
    publish_mock(&mut c, &admin, 64_640, 50).unwrap(); // 0.50% spread

    let q = read_quote(&c.svm, c.quote);
    assert_eq!(q.sequence, 1);
    assert_eq!(q.quote_amount, 64_640);
    assert_eq!(q.lower_quote_amount, 64_640 - 323);
    assert_eq!(q.upper_quote_amount, 64_640 + 323);
    assert_eq!(q.observed_slot, 1000);
    assert_eq!(q.valid_until_slot, 1200);
}

/// The keeper is just paying for gas. ANYONE can crank it, and the value
/// written is identical because the adapter computes it, not the caller.
#[test]
fn any_wallet_can_crank_it() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);

    let stranger = Keypair::new();
    c.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    publish_mock(&mut c, &stranger, 64_640, 50).unwrap();

    assert_eq!(read_quote(&c.svm, c.quote).sequence, 1);
}

/// A wallet cannot bypass the adapter and write to the core directly.
#[test]
fn wallet_cannot_write_to_core_directly() {
    let mut c = setup();
    c.svm.warp_to_slot(1000);

    let attacker = Keypair::new();
    c.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction {
        program_id: CORE_ID,
        accounts: svi_core::accounts::PublishQuote {
            descriptor: c.descriptor,
            quote: c.quote,
            adapter_authority: attacker.pubkey(),
        }
        .to_account_metas(None),
        data: svi_core::instruction::PublishQuote {
            u: svi_core::QuoteUpdate {
                base_amount: 1_000_000_000,
                quote_amount: 999_999_999, // a lie
                lower_quote_amount: 999_999_999,
                upper_quote_amount: 999_999_999,
                observed_slot: 1000,
                observed_unix_ts: 0,
                source_accounts_hash: [0u8; 32],
                status_flags: 0,
            },
        }
        .data(),
    };
    let err = send(&mut c.svm, &[ix], &attacker, &[&attacker]).unwrap_err();
    assert!(err.contains("6001"), "expected UnauthorizedAdapter, got {err}");
    assert_eq!(read_quote(&c.svm, c.quote).sequence, 0, "nothing was written");
}
