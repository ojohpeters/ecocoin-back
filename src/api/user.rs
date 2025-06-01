use axum::extract::Query;
use axum::http::StatusCode;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{db, error::AppError, solana};
use serde_json::json;

#[derive(Deserialize)]
struct ConnectWalletRequest {
    wallet_address: String,
    referral_code: Option<String>,
}

#[derive(Deserialize)]
struct CompleteTaskRequest {
    wallet_address: String,
    task_id: Uuid,
}

#[derive(Deserialize)]
struct ClaimRequest {
    wallet_address: String,
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/user/connect_wallet", post(connect_wallet))
        .route("/api/user/complete_task", post(complete_task))
        .route("/api/user/points", get(get_points))
        .route("/api/user/claim_airdrop", post(claim_airdrop))
        .route("/api/airdrop/stats", get(get_airdrop_stats))
        .route("/api/user/referral_code", get(get_referral_code))
}

pub async fn connect_wallet(
    Json(req): Json<ConnectWalletRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = db::create_user(&req.wallet_address)
        .await
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create user"))?;

    if let Some(ref_code) = req.referral_code {
        if let Some(referrer_id) =
            db::get_user_id_by_referral_code(&ref_code)
                .await
                .map_err(|_| {
                    AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Referral lookup failed")
                })?
        {
            db::set_referrer(&user_id, &referrer_id)
                .await
                .map_err(|_| {
                    AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to link referrer")
                })?;

            db::add_referral_points(&referrer_id).await.map_err(|_| {
                AppError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to credit referral",
                )
            })?;
        }
    }

    Ok(Json(serde_json::json!({ "status": "wallet connected" })))
}

pub async fn complete_task(
    Json(req): Json<CompleteTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::complete_task(&req.wallet_address, req.task_id)
        .await
        .map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "Task already completed or invalid"))?;

    Ok(Json(serde_json::json!({ "status": "task recorded" })))
}

pub async fn get_points(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let wallet = params
        .get("wallet")
        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, "Missing wallet param"))?;

    let user_info = db::get_user_info(wallet)
        .await
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "DB error"))?;

    Ok(Json(serde_json::json!(user_info)))
}

pub async fn get_airdrop_stats() -> Json<serde_json::Value> {
    let wallet_count = db::get_wallet_count().await.unwrap_or(0);
    let total_claims = db::get_total_airdrops().await.unwrap_or(0);

    Json(json!({
        "wallets_registered": wallet_count,
        "airdrops_sent": total_claims
    }))
}

pub async fn get_referral_code(
    Query(params): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    if let Some(wallet) = params.get("wallet") {
        match db::get_referral_code_by_wallet(wallet).await {
            Ok(code) => Json(json!({ "referral_code": code })),
            Err(_) => Json(json!({ "error": "Wallet not found" })),
        }
    } else {
        Json(json!({ "error": "Missing wallet parameter" }))
    }
}

pub async fn claim_airdrop(Json(req): Json<ClaimRequest>) -> Json<serde_json::Value> {
    let user_info = db::get_user_info(&req.wallet_address).await.unwrap();

    if user_info.total_points < 1000 {
        return Json(json!({ "error": "Not enough points (min 1000)" }));
    }

    let paid_sig = solana::check_fee_paid(&req.wallet_address).await.unwrap();
if paid_sig.is_none() {
    return Json(json!({ "error": "Fee not detected" }));
}

let fee_tx = paid_sig.unwrap();

// Check if already used
let fee_valid = db::record_fee_if_new(&req.wallet_address, &fee_tx).await.unwrap();
if !fee_valid {
    return Json(json!({ "error": "Fee already used for previous claim" }));
}

 //   if !paid {
   //     return Json(json!({ "error": "Fee not detected" }));
    //}

    // Send exactly 1000 tokens
    match solana::send_tokens(&req.wallet_address, 1000).await {
        Ok(sig) => {
            // Log airdrop + update DB
            db::log_airdrop(&req.wallet_address, 1000, &sig)
                .await
                .unwrap();

            db::deduct_user_points(&req.wallet_address, 1000)
                .await
                .unwrap();

            db::set_claimed(&req.wallet_address).await.unwrap(); // still needed
	    db::mark_fee_used(&req.wallet_address, &fee_tx).await.unwrap();

            Json(json!({
                "status": "Airdrop sent",
                "tokens": 1000,
                "tx": sig
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
