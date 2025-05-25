mod api;
mod config;
mod db;
mod models;
mod solana;
mod error;

use axum::Router;
use dotenvy::dotenv;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    db::init_db().await.expect("Database failed");

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any) // You can replace Any with a specific origin
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(api::user::routes())
        .merge(api::tasks::routes())
        .layer(cors); // Add the CORS layer here

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🚀 Server running at http://localhost:8080");

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
