use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::dtos::{
    Approved, AssignRequest, BriefSubmitted, Cancelled, CancelRequest, CreateServiceOrderRequest,
    DeliverableSubmitted, DeliveryDownload, RequestRevisionRequest, RevisionRequested,
    ServiceOrderDetail, ServiceOrderSummary, SubmitBriefRequest, SubmitDeliverableRequest,
};
use super::service;
use crate::common::extractors::AuthUser;
use crate::error::AppError;
use crate::modules::digital::service::Page;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orders", post(create_order).get(list_orders))
        .route("/orders/{order_id}", get(get_order))
        .route("/orders/{order_id}/brief", post(submit_brief))
        .route("/orders/{order_id}/assign", patch(assign))
        .route("/orders/{order_id}/deliverable", post(submit_deliverable))
        .route("/orders/{order_id}/revision", post(request_revision))
        .route("/orders/{order_id}/approve", post(approve))
        .route("/orders/{order_id}/cancel", post(cancel))
        .route(
            "/orders/{order_id}/deliverable/{deliverable_id}/download",
            get(deliverable_download),
        )
}

#[derive(Debug, Deserialize)]
struct Pagination {
    page: Option<i64>,
    per_page: Option<i64>,
}

async fn create_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateServiceOrderRequest>,
) -> Result<(axum::http::StatusCode, Json<ServiceOrderSummary>), AppError> {
    let order = service::create_order(&state, auth.id, payload).await?;
    Ok((axum::http::StatusCode::CREATED, Json(order)))
}

async fn list_orders(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<ServiceOrderSummary>>, AppError> {
    let page = Page::new(pagination.page, pagination.per_page);
    let orders = service::list_orders(&state, &auth, &page).await?;
    Ok(Json(orders))
}

async fn get_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<ServiceOrderDetail>, AppError> {
    Ok(Json(service::get_order(&state, &auth, order_id).await?))
}

async fn submit_brief(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<SubmitBriefRequest>,
) -> Result<Json<BriefSubmitted>, AppError> {
    Ok(Json(service::submit_brief(&state, &auth, order_id, payload).await?))
}

async fn assign(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<AssignRequest>,
) -> Result<Json<ServiceOrderSummary>, AppError> {
    Ok(Json(service::assign(&state, &auth, order_id, payload).await?))
}

async fn submit_deliverable(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<SubmitDeliverableRequest>,
) -> Result<(axum::http::StatusCode, Json<DeliverableSubmitted>), AppError> {
    let result = service::submit_deliverable(&state, &auth, order_id, payload).await?;
    Ok((axum::http::StatusCode::ACCEPTED, Json(result)))
}

async fn request_revision(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<RequestRevisionRequest>,
) -> Result<Json<RevisionRequested>, AppError> {
    Ok(Json(
        service::request_revision(&state, &auth, order_id, payload).await?,
    ))
}

async fn approve(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Approved>, AppError> {
    Ok(Json(service::approve(&state, &auth, order_id).await?))
}

async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<CancelRequest>,
) -> Result<Json<Cancelled>, AppError> {
    Ok(Json(service::cancel(&state, &auth, order_id, payload).await?))
}

async fn deliverable_download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((order_id, deliverable_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<DeliveryDownload>, AppError> {
    Ok(Json(
        service::deliverable_download(&state, &auth, order_id, deliverable_id).await?,
    ))
}
