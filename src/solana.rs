use crate::error::AppError;
use axum::http::StatusCode;
use solana_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::{
    EncodedTransaction, UiMessage, UiParsedMessage, UiTransaction, UiTransactionEncoding,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction::transfer_checked;
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::{env, str::FromStr};

const REQUIRED_LAMPORTS: u64 = 6_000; // 0.006 SOL
const TOKEN_DECIMALS: u8 = 6;

pub async fn check_fee_paid(user_wallet: &str) -> Result<bool, AppError> {
    let rpc_url = env::var("SOLANA_RPC_URL")
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Missing SOLANA_RPC_URL"))?;

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let user_pubkey = Pubkey::from_str(user_wallet)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "Invalid user wallet"))?;

    let airdrop_wallet = Pubkey::from_str("DkrCNNn27B1Loz6eGpMYKAL7b5J4GY6wwQs8wqY9ERBT")
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Invalid airdrop wallet"))?;

    let sigs = rpc
        .get_signatures_for_address_with_config(
            &airdrop_wallet,
            GetConfirmedSignaturesForAddress2Config {
                limit: Some(50),
                ..Default::default()
            },
        )
        .map_err(|_| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch transactions",
            )
        })?;

    for sig_info in sigs {
        let sig = Signature::from_str(&sig_info.signature)
            .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Invalid signature"))?;

        let tx = rpc
            .get_transaction(&sig, UiTransactionEncoding::JsonParsed)
            .map_err(|_| {
                AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch transaction",
                )
            })?;

        if let Some(meta) = tx.transaction.meta {
            if let EncodedTransaction::Json(json_tx) = tx.transaction.transaction {
                let pubkeys: Vec<String> = match &json_tx.message {
                    UiMessage::Parsed(parsed_msg) => parsed_msg
                        .account_keys
                        .iter()
                        .map(|acc| acc.pubkey.clone())
                        .collect(),
                    UiMessage::Raw(raw_msg) => raw_msg.account_keys.clone(),
                };

                let s_airdrop_wallet = airdrop_wallet.to_string();
                if let Ok(airdrop_pubkey) = Pubkey::from_str(&s_airdrop_wallet) {
                    if let Some(idx) = pubkeys
                        .iter()
                        .position(|k| Pubkey::from_str(k).unwrap() == airdrop_pubkey)
                    {
                        let delta = meta.post_balances[idx] as i64 - meta.pre_balances[idx] as i64;

                        if delta >= REQUIRED_LAMPORTS as i64
                            && pubkeys
                                .iter()
                                .any(|k| Pubkey::from_str(k).unwrap() == user_pubkey)
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

pub async fn send_tokens(to_wallet: &str, token_amount: i32) -> Result<(), AppError> {
    let rpc_url = env::var("SOLANA_RPC_URL")
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Missing SOLANA_RPC_URL"))?;

    let rpc = RpcClient::new(rpc_url);

    let payer = read_keypair_file(env::var("AIR_DROP_WALLET_PATH").map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing AIR_DROP_WALLET_PATH",
        )
    })?)
    .map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to load wallet keypair",
        )
    })?;
    println!("🔑 Airdrop wallet address: {}", payer.pubkey());

    let payer_pubkey = payer.pubkey();

    let mint = Pubkey::from_str(
        &env::var("TOKEN_MINT")
            .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Missing TOKEN_MINT"))?,
    )
    .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Invalid mint address"))?;

    let to_pubkey = Pubkey::from_str(to_wallet)
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "Invalid recipient wallet"))?;

    let payer_token_account = get_associated_token_address(&payer_pubkey, &mint);
    let recipient_token_account = get_associated_token_address(&to_pubkey, &mint);

    // Create ATA if it doesn't exist
    if rpc.get_account(&recipient_token_account).is_err() {
        println!("📦 ATA not found for {} — creating it...", to_wallet);
        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &payer_pubkey,     // Fee payer
                &to_pubkey,        // User receiving token
                &mint,             // Your SPL token mint
                &TOKEN_PROGRAM_ID, // ✅ SPL Token Program ID
            );

        let blockhash = rpc.get_latest_blockhash().map_err(|_| {
            AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch blockhash for ATA creation",
            )
        })?;

        let create_ata_tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&payer_pubkey),
            &[&payer],
            blockhash,
        );

        rpc.send_and_confirm_transaction(&create_ata_tx)
            .map_err(|e| {
                AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create ATA: {}", e),
                )
            })?;

        println!("✅ ATA created for {}", to_wallet);
    }

    // Token transfer
    let amount = token_amount as u64;

    let transfer_ix: Instruction = transfer_checked(
        &spl_token::ID,
        &payer_token_account,
        &mint,
        &recipient_token_account,
        &payer_pubkey,
        &[],
        amount,
        TOKEN_DECIMALS,
    )
    .map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build transfer instruction",
        )
    })?;

    let recent_blockhash = rpc.get_latest_blockhash().map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch blockhash",
        )
    })?;

    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&payer_pubkey),
        &[&payer],
        recent_blockhash,
    );

    let sig = rpc.send_and_confirm_transaction(&tx).map_err(|e| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Transfer failed: {}", e),
        )
    })?;

    println!("✅ Tokens sent: {} to {}", amount, to_wallet);
    println!("🔗 Tx: https://solscan.io/tx/{}", sig);

    Ok(())
}
