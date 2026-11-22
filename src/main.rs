use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;

use erp_commerce::app::build_router;
use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::state::AppState;

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
