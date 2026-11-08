use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::dtos::{LoginRequest, LoginResponse, RegisterRequest, UserProfile};
use super::service;
use crate::common::extractors::AuthUser;
use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<UserProfile>), AppError> {
    let profile = service::register(&state, payload).await?;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    Ok(Json(service::login(&state, payload).await?))
}

async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UserProfile>, AppError> {
    Ok(Json(service::current_user(&state, auth.id).await?))
}
