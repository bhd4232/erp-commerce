use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use super::dtos::{PresignRequest, PresignResponse};
use super::service;
use crate::common::extractors::AuthUser;
use crate::common::response::Envelope;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/presign", post(presign))
}

async fn presign(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<PresignRequest>,
) -> Result<Json<Envelope<PresignResponse>>, AppError> {
    let response = service::presign(&state, auth.id, payload).await?;
    Ok(Json(Envelope::new(response)))
}
