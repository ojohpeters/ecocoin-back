use crate::db;
use crate::error::AppError;
use axum::http::StatusCode;
use axum::{routing::get, Json, Router};

pub fn routes() -> Router {
    Router::new().route("/api/tasks", get(get_tasks))
}

async fn get_tasks() -> Result<Json<serde_json::Value>, AppError> {
    let tasks = db::get_all_tasks()
        .await
        .map_err(|_| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch tasks"))?;

    Ok(Json(serde_json::json!(tasks)))
}
