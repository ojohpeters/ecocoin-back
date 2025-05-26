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
        .route("/api/user/stats", get(get_user_stats))
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

pub async fn get_user_stats() -> Result<Json<serde_json::Value>, AppError> {
    let count = db::get_wallet_count().await.map_err(|_| {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get wallet count",
        )
    })?;

    Ok(Json(serde_json::json!({ "total_wallets": count })))
}

pub async fn claim_airdrop(
    Json(req): Json<ClaimRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_info = db::get_user_info(&req.wallet_address)
        .await
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "DB error"))?;

    if user_info.total_points < 1000 {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "Not enough points (min 1000)",
        ));
    }

    let paid = solana::check_fee_paid(&req.wallet_address)
        .await
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Fee check failed"))?;

    if !paid {
        return Err(AppError::new(StatusCode::BAD_REQUEST, "Fee not detected"));
    }

    solana::send_tokens(&req.wallet_address, user_info.total_points)
        .await
        .map_err(|e| AppError::new(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "Airdrop sent",
        "tokens": user_info.total_points
    })))
}
