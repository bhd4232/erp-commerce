use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::commerce;
use super::dtos::{
    CheckoutRequest, CheckoutResponse, DownloadResponse, ProductCard, ProductDetail,
    RefundResponse, VaultItem,
};
use super::service::{self, Page};
use crate::common::extractors::{AuthUser, Role};
use crate::common::response::Envelope;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/products", get(list_products))
        .route("/products/{slug}", get(get_product))
        .route("/my-vault", get(my_vault))
        .route("/checkout", post(checkout))
        .route("/download/{entitlement_id}", post(download))
        .route("/orders/{order_id}/refund", post(refund_order))
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
) -> Result<Json<Envelope<Vec<ProductCard>>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let (items, total) = service::list_products(&state, caller.map(|c| c.id), &page).await?;
    Ok(Json(Envelope::with_meta(items, page.meta(total))))
}

async fn get_product(
    State(state): State<AppState>,
    caller: Option<AuthUser>,
    Path(slug): Path<String>,
) -> Result<Json<Envelope<ProductDetail>>, AppError> {
    let product = service::get_product(&state, &slug, caller.map(|c| c.id)).await?;
    Ok(Json(Envelope::new(product)))
}

async fn my_vault(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Envelope<Vec<VaultItem>>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let (items, total) = service::my_vault(&state, auth.id, &page).await?;
    Ok(Json(Envelope::with_meta(items, page.meta(total))))
}

async fn download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entitlement_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Envelope<DownloadResponse>>, AppError> {
    let ip = client_ip(&headers);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok());

    let response = service::download(&state, auth.id, entitlement_id, &ip, user_agent).await?;
    Ok(Json(Envelope::new(response)))
}

async fn checkout(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CheckoutRequest>,
) -> Result<(StatusCode, Json<Envelope<CheckoutResponse>>), AppError> {
    let response = commerce::checkout(&state, auth.id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(response))))
}

async fn refund_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Envelope<RefundResponse>>, AppError> {
    auth.require_role(&[Role::Admin])?;
    let response = commerce::refund_order(&state, order_id).await?;
    Ok(Json(Envelope::new(response)))
}

/// Best-effort client IP. The proxy in front of the API is trusted to set
/// `X-Forwarded-For`; never trust it when the API is directly exposed.
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().chars().take(45).collect())
        .unwrap_or_else(|| "0.0.0.0".to_string())
}
