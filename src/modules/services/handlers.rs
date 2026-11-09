use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use super::dtos::{
    Approved, AssignRequest, BriefSubmitted, CancelRequest, Cancelled, CreateServiceOrderRequest,
    DeliverableSubmitted, DeliveryDownload, RequestRevisionRequest, RevisionRequested,
    ServiceOrderDetail, ServiceOrderSummary, SubmitBriefRequest, SubmitDeliverableRequest,
};
use super::models::ServiceStatus;
use super::service;
use crate::common::extractors::AuthUser;
use crate::common::response::Envelope;
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
struct ListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
    /// `me` limits the board to the caller's assigned orders.
    assignee: Option<String>,
}

async fn create_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateServiceOrderRequest>,
) -> Result<(axum::http::StatusCode, Json<Envelope<ServiceOrderSummary>>), AppError> {
    let order = service::create_order(&state, auth.id, payload).await?;
    Ok((axum::http::StatusCode::CREATED, Json(Envelope::new(order))))
}

async fn list_orders(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<ServiceOrderSummary>>>, AppError> {
    let status = match query.status.as_deref() {
        None => None,
        Some(raw) => Some(
            ServiceStatus::parse(raw)
                .ok_or_else(|| AppError::Validation(format!("unknown status '{raw}'")))?,
        ),
    };
    let assignee_me = query.assignee.as_deref() == Some("me");

    let page = Page::new(query.page, query.per_page);
    let (orders, total) = service::list_orders(&state, &auth, &page, status, assignee_me).await?;
    Ok(Json(Envelope::with_meta(orders, page.meta(total))))
}

async fn get_order(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Envelope<ServiceOrderDetail>>, AppError> {
    let order = service::get_order(&state, &auth, order_id).await?;
    Ok(Json(Envelope::new(order)))
}

async fn submit_brief(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<SubmitBriefRequest>,
) -> Result<Json<BriefSubmitted>, AppError> {
    Ok(Json(
        service::submit_brief(&state, &auth, order_id, payload).await?,
    ))
}

async fn assign(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<AssignRequest>,
) -> Result<Json<ServiceOrderSummary>, AppError> {
    Ok(Json(
        service::assign(&state, &auth, order_id, payload).await?,
    ))
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
    Ok(Json(
        service::cancel(&state, &auth, order_id, payload).await?,
    ))
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
