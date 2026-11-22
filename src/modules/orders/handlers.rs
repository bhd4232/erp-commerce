use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::dtos::OrderSummary;
use super::service;
use crate::common::pagination::Page;
use crate::common::response::Envelope;
use crate::common::tenant::TenantContext;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/orders", get(list_orders))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
}

async fn list_orders(
    State(state): State<AppState>,
    ctx: TenantContext,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<OrderSummary>>>, AppError> {
    let page = Page::new(query.page, query.per_page);
    let (orders, total) = service::list_orders(
        &state,
        &ctx,
        &page,
        query.status.as_deref().filter(|value| !value.is_empty()),
    )
    .await?;
    Ok(Json(Envelope::with_meta(orders, page.meta(total))))
}
