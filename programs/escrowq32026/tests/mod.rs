use {
    anchor_lang::{
        prelude::msg, solana_program::instruction::Instruction, solana_program::program_pack::Pack,
        system_program::ID as SYSTEM_PROGRAM_ID, AccountDeserialize, InstructionData,
        ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::Message,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (LiteSVM, Keypair) {
    let program_id = escrowq32026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer)
}

#[test]
fn test_make_and_refund() {
    let (mut program, payer) = setup();
    let maker = payer.pubkey();

    let mint_a = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let mint_b = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();

    let seed = 123u64;
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;

    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed,
            receive: 10_000_000,
            expiration: 17780206209,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("Make transaction successful, CU: {}", tx.compute_units_consumed);

    let vault_account = program.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.seed, seed);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.receive, 10_000_000);

    let refund_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Refund {
            maker,
            mint_a,
            maker_ata_a,
            escrow,
            vault,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Refund {}.data(),
    };

    let message = Message::new(&[refund_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("Refund transaction successful, CU: {}", tx.compute_units_consumed);
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
}

#[test]
fn test_make_and_take() {
    let (mut program, maker_payer) = setup();
    let maker = maker_payer.pubkey();

    let taker_payer = Keypair::new();
    program.airdrop(&taker_payer.pubkey(), 1_000_000_000).unwrap();
    let taker = taker_payer.pubkey();

    let mint_a = CreateMint::new(&mut program, &maker_payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let mint_b = CreateMint::new(&mut program, &maker_payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &maker_payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();

    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut program, &taker_payer, &mint_b)
        .owner(&taker)
        .send()
        .unwrap();

    let seed = 456u64;
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;

    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    let taker_ata_a = associated_token::get_associated_token_address(&taker, &mint_a);
    let maker_ata_b = associated_token::get_associated_token_address(&maker, &mint_b);

    let deposit_amount = 50_000_000u64;
    let receive_amount = 25_000_000u64;

    MintTo::new(&mut program, &maker_payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    MintTo::new(&mut program, &maker_payer, &mint_b, &taker_ata_b, 1000_000_000)
        .send()
        .unwrap();

    // 1. Make
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: deposit_amount,
            seed,
            receive: receive_amount,
            expiration: 17780206209,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&maker_payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&maker_payer], message, recent_blockhash);
    program.send_transaction(transaction).unwrap();

    // 2. Take
    let take_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Take {
            taker,
            maker,
            mint_a,
            mint_b,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Take {}.data(),
    };

    let message = Message::new(&[take_ix], Some(&taker_payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&taker_payer], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("Take transaction successful, CU: {}", tx.compute_units_consumed);

    // Verify balances
    let maker_ata_b_acc = program.get_account(&maker_ata_b).unwrap();
    let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_acc.data).unwrap();
    assert_eq!(maker_ata_b_data.amount, receive_amount);

    let taker_ata_a_acc = program.get_account(&taker_ata_a).unwrap();
    let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_acc.data).unwrap();
    assert_eq!(taker_ata_a_data.amount, deposit_amount);

    // Verify escrow and vault closed
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
}

#[test]
fn test_make_and_update() {
    let (mut program, payer) = setup();
    let maker = payer.pubkey();

    let mint_a = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let mint_b = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();

    let seed = 789u64;
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;

    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    // 1. Make
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed,
            receive: 10_000_000,
            expiration: 17780206209,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    program.send_transaction(transaction).unwrap();

    // 2. Update
    let new_receive = 35_000_000u64;
    let new_expiration = 20000000000i64;

    let update_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Update {
            maker,
            escrow,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Update {
            receive: new_receive,
            expiration: new_expiration,
        }
        .data(),
    };

    let message = Message::new(&[update_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("Update transaction successful, CU: {}", tx.compute_units_consumed);

    // Verify updated state
    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.receive, new_receive);
    assert_eq!(escrow_data.expiration, new_expiration);
}
