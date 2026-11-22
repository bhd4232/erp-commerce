use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::dtos::{
    ConnectorHealth, ConnectorSummary, CreateConnectorRequest, CreatedConnector, ImportResult,
    SyncResult, TestResult, WebhookResult,
};
use super::service;
use crate::common::response::Envelope;
use crate::common::tenant::TenantContext;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/connectors",
            get(list_connectors).post(create_connector),
        )
        .route("/api/v1/connectors/{connector_id}/import", post(import_csv))
        .route(
            "/api/v1/connectors/{connector_id}/test",
            post(test_connector),
        )
        .route(
            "/api/v1/connectors/{connector_id}/sync",
            post(sync_connector),
        )
        .route("/api/v1/connectors/{connector_id}/health", get(health))
        .route(
            "/api/v1/webhooks/woocommerce/{connector_id}",
            post(woocommerce_webhook),
        )
}

async fn list_connectors(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> Result<Json<Envelope<Vec<ConnectorSummary>>>, AppError> {
    Ok(Json(Envelope::new(
        service::list_connectors(&state, &ctx).await?,
    )))
}

async fn create_connector(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(payload): Json<CreateConnectorRequest>,
) -> Result<(StatusCode, Json<Envelope<CreatedConnector>>), AppError> {
    let connector = service::create_connector(&state, &ctx, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(connector))))
}

async fn import_csv(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(connector_id): Path<Uuid>,
    body: Bytes,
) -> Result<Json<Envelope<ImportResult>>, AppError> {
    let result = service::import_csv(&state, &ctx, connector_id, &body).await?;
    Ok(Json(Envelope::new(result)))
}

#[derive(Debug, Deserialize)]
struct SyncQuery {
    pages: Option<u32>,
}

async fn test_connector(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(connector_id): Path<Uuid>,
) -> Result<Json<Envelope<TestResult>>, AppError> {
    Ok(Json(Envelope::new(
        service::test_connector(&state, &ctx, connector_id).await?,
    )))
}

async fn sync_connector(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(connector_id): Path<Uuid>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Envelope<SyncResult>>, AppError> {
    Ok(Json(Envelope::new(
        service::sync_connector(&state, &ctx, connector_id, query.pages).await?,
    )))
}

async fn health(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(connector_id): Path<Uuid>,
) -> Result<Json<Envelope<ConnectorHealth>>, AppError> {
    Ok(Json(Envelope::new(
        service::connector_health(&state, &ctx, connector_id).await?,
    )))
}

async fn woocommerce_webhook(
    State(state): State<AppState>,
    Path(connector_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Envelope<WebhookResult>>, AppError> {
    let result = service::ingest_woocommerce(&state, connector_id, &headers, &body).await?;
    Ok(Json(Envelope::new(result)))
}
