use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::modules;
use crate::state::AppState;

/// Single place where the HTTP surface is composed. Integration tests build the
/// same router so the handler/extractor path is covered, not just services.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", axum::routing::get(healthz))
        .nest("/api/v1/auth", modules::auth::handlers::router())
        .nest("/api/v1/digital", modules::digital::handlers::router())
        .nest("/api/v1/services", modules::services::handlers::router())
        .nest("/api/v1/uploads", modules::uploads::handlers::router())
        .nest("/api/v1/finance", modules::finance::handlers::router())
        .nest("/api/v1/webhooks", modules::finance::webhooks::router())
        .merge(modules::tenancy::handlers::router())
        .merge(modules::connectors::handlers::router())
        .merge(modules::orders::handlers::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
