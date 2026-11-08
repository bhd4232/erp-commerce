use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::dtos::{
    CheckoutRequest, CheckoutResponse, DownloadResponse, ProductCard, ProductDetail, VaultItem,
};
use super::service::{self, Page};
use crate::common::extractors::AuthUser;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", get(list_products))
        .route("/products/{slug}", get(get_product))
        .route("/my-vault", get(my_vault))
        .route("/checkout", post(checkout))
        .route("/download/{entitlement_id}", post(download))
}

#[derive(Debug, Deserialize)]
struct Pagination {
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn list_products(
    State(state): State<AppState>,
    caller: Option<AuthUser>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<ProductCard>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let products = service::list_products(&state, caller.map(|c| c.id), &page).await?;
    Ok(Json(products))
}

async fn get_product(
    State(state): State<AppState>,
    caller: Option<AuthUser>,
    Path(slug): Path<String>,
) -> Result<Json<ProductDetail>, AppError> {
    let product = service::get_product(&state, &slug, caller.map(|c| c.id)).await?;
    Ok(Json(product))
}

async fn my_vault(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<VaultItem>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let items = service::my_vault(&state, auth.id, &page).await?;
    Ok(Json(items))
}

async fn download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entitlement_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DownloadResponse>, AppError> {
    let ip = client_ip(&headers);
    let ua = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok());

    let response = service::download(&state, auth.id, entitlement_id, &ip, ua).await?;
    Ok(Json(response))
}

async fn checkout(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>, AppError> {
    let response = service::checkout(&state, auth.id, payload).await?;
    Ok(Json(response))
}

/// Best-effort client IP. The proxy in front of the API is trusted to set
/// `X-Forwarded-For`; never trust it when the API is directly exposed.
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| "0.0.0.0".to_string())
}
