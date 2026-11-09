use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::dtos::{
    InvoiceDetail, InvoiceSummary, TopupRequest, TopupView, TransactionView, WalletView,
};
use super::service;
use crate::common::extractors::{AuthUser, Role};
use crate::common::pagination::Page;
use crate::common::response::Envelope;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/wallet", get(wallet))
        .route("/transactions", get(transactions))
        .route("/topup", post(topup))
        .route("/invoices", get(list_invoices))
        .route("/invoices/{invoice_id}", get(get_invoice))
}

#[derive(Debug, Deserialize)]
struct Pagination {
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn wallet(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Envelope<WalletView>>, AppError> {
    Ok(Json(Envelope::new(service::wallet(&state, auth.id).await?)))
}

async fn transactions(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Envelope<Vec<TransactionView>>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let (items, total) = service::transactions(&state, auth.id, &page).await?;
    Ok(Json(Envelope::with_meta(items, page.meta(total))))
}

async fn topup(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<TopupRequest>,
) -> Result<(StatusCode, Json<Envelope<TopupView>>), AppError> {
    let topup = service::topup(&state, auth.id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(topup))))
}

async fn list_invoices(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Envelope<Vec<InvoiceSummary>>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let (items, total) = service::invoices(&state, auth.id, &page).await?;
    Ok(Json(Envelope::with_meta(items, page.meta(total))))
}

async fn get_invoice(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(invoice_id): Path<Uuid>,
) -> Result<Json<Envelope<InvoiceDetail>>, AppError> {
    let is_admin = auth.role == Role::Admin;
    let invoice = service::invoice(&state, auth.id, is_admin, invoice_id).await?;
    Ok(Json(Envelope::new(invoice)))
}
