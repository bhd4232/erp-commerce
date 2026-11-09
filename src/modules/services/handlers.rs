use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
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
use super::{service, transitions};
use crate::common::extractors::AuthUser;
use crate::common::pagination::Page;
use crate::common::response::Envelope;
use crate::error::AppError;
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
) -> Result<(StatusCode, Json<Envelope<ServiceOrderSummary>>), AppError> {
    let order = service::create_order(&state, auth.id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(order))))
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
) -> Result<Json<Envelope<BriefSubmitted>>, AppError> {
    let result = transitions::submit_brief(&state, &auth, order_id, payload).await?;
    Ok(Json(Envelope::new(result)))
}

async fn assign(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<AssignRequest>,
) -> Result<Json<Envelope<ServiceOrderSummary>>, AppError> {
    let order = transitions::assign(&state, &auth, order_id, payload).await?;
    Ok(Json(Envelope::new(order)))
}

async fn submit_deliverable(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<SubmitDeliverableRequest>,
) -> Result<(StatusCode, Json<Envelope<DeliverableSubmitted>>), AppError> {
    let result = transitions::submit_deliverable(&state, &auth, order_id, payload).await?;
    Ok((StatusCode::ACCEPTED, Json(Envelope::new(result))))
}

async fn request_revision(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<RequestRevisionRequest>,
) -> Result<Json<Envelope<RevisionRequested>>, AppError> {
    let result = transitions::request_revision(&state, &auth, order_id, payload).await?;
    Ok(Json(Envelope::new(result)))
}

async fn approve(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Envelope<Approved>>, AppError> {
    let result = transitions::approve(&state, &auth, order_id).await?;
    Ok(Json(Envelope::new(result)))
}

async fn cancel(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<CancelRequest>,
) -> Result<Json<Envelope<Cancelled>>, AppError> {
    let result = transitions::cancel(&state, &auth, order_id, payload).await?;
    Ok(Json(Envelope::new(result)))
}

async fn deliverable_download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((order_id, deliverable_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<DeliveryDownload>>, AppError> {
    let result = service::deliverable_download(&state, &auth, order_id, deliverable_id).await?;
    Ok(Json(Envelope::new(result)))
}
