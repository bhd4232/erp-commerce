use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use uuid::Uuid;

use super::dtos::{
    InviteRequest, PendingInvitation, TeamMember, TenantSummary, UpdateMemberRequest,
};
use super::service;
use crate::common::extractors::AuthUser;
use crate::common::response::Envelope;
use crate::common::tenant::TenantContext;
use crate::error::AppError;
use crate::state::AppState;

/// Full paths so `main` can `merge` this router without another `/api/v1` nest.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tenants", get(list_tenants))
        .route("/api/v1/tenants/current", get(current_tenant))
        .route("/api/v1/team", get(list_members))
        .route("/api/v1/team/invite", post(invite))
        .route("/api/v1/team/invitations", get(pending_invitations))
        .route(
            "/api/v1/team/invitations/{membership_id}/accept",
            post(accept_invitation),
        )
        .route("/api/v1/team/{user_id}", patch(update_member))
}

async fn list_tenants(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Envelope<Vec<TenantSummary>>>, AppError> {
    Ok(Json(Envelope::new(
        service::list_tenants(&state, auth.id).await?,
    )))
}

async fn current_tenant(ctx: TenantContext) -> Result<Json<Envelope<TenantSummary>>, AppError> {
    Ok(Json(Envelope::new(service::current_tenant(&ctx).await)))
}

async fn list_members(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> Result<Json<Envelope<Vec<TeamMember>>>, AppError> {
    Ok(Json(Envelope::new(
        service::list_members(&state, &ctx).await?,
    )))
}

async fn invite(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(payload): Json<InviteRequest>,
) -> Result<(StatusCode, Json<Envelope<TeamMember>>), AppError> {
    let member = service::invite(&state, &ctx, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(member))))
}

async fn pending_invitations(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Envelope<Vec<PendingInvitation>>>, AppError> {
    Ok(Json(Envelope::new(
        service::pending_invitations(&state, auth.id).await?,
    )))
}

async fn accept_invitation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(membership_id): Path<Uuid>,
) -> Result<Json<Envelope<TenantSummary>>, AppError> {
    Ok(Json(Envelope::new(
        service::accept_invitation(&state, auth.id, membership_id).await?,
    )))
}

async fn update_member(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateMemberRequest>,
) -> Result<Json<Envelope<TeamMember>>, AppError> {
    Ok(Json(Envelope::new(
        service::update_member(&state, &ctx, user_id, payload).await?,
    )))
}
