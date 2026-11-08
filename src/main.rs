mod common;
mod config;
mod error;
mod modules;
mod state;

use std::sync::Arc;

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::common::storage::Storage;
use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    let bind = format!("{}:{}", cfg.host, cfg.port);

    let db = PgPoolOptions::new()
        .max_connections(cfg.database_max_connections)
        .connect(&cfg.database_url)
        .await?;

    // Migrations are embedded at compile time; safe to run on every boot
    // because sqlx records applied versions. In production, run them as a
    // separate one-shot job under an advisory lock instead.
    if std::env::var("RUN_MIGRATIONS").ok().as_deref() == Some("1") {
        sqlx::migrate!("./migrations").run(&db).await?;
        tracing::info!("database migrations applied");
    }

    let storage = Storage::new(&cfg).await;

    let state = AppState {
        db,
        cfg: Arc::new(cfg),
        storage,
    };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("listening on {bind}");
    axum::serve(listener, app).await?;

    Ok(())
}

/// Single place where the HTTP surface is composed.
///
/// Route paths are the contract in docs/API_SPEC.md; keep them in sync.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1/auth", modules::auth::handlers::router())
        .nest("/api/v1/digital", modules::digital::handlers::router())
        .nest("/api/v1/services", modules::services::handlers::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
