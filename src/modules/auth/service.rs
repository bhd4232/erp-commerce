use uuid::Uuid;

use super::dtos::{LoginRequest, LoginResponse, RegisterRequest, UserProfile};
use super::models::User;
use crate::common::auth::{hash_password, issue_token, verify_password};
use crate::error::AppError;
use crate::state::AppState;

const MIN_PASSWORD_LEN: usize = 12;

pub async fn register(state: &AppState, payload: RegisterRequest) -> Result<UserProfile, AppError> {
    let email = payload.email.trim().to_lowercase();
    let full_name = payload.full_name.trim().to_string();

    if !email.contains('@') || email.len() > 255 {
        return Err(AppError::Validation("a valid email is required".into()));
    }
    if full_name.is_empty() || full_name.chars().count() > 100 {
        return Err(AppError::Validation(
            "full_name must be 1-100 characters".into(),
        ));
    }
    if payload.password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let password_hash = hash_password(&payload.password)?;

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, password_hash, full_name)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&full_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("users_email_key") => {
            AppError::EmailTaken
        }
        _ => AppError::Db(e),
    })?;

    Ok(user.into())
}

pub async fn login(state: &AppState, payload: LoginRequest) -> Result<LoginResponse, AppError> {
    let email = payload.email.trim().to_lowercase();

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?
        // Same error for unknown email and wrong password: no user enumeration.
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(&payload.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    if !user.is_active {
        return Err(AppError::Forbidden);
    }

    let token = issue_token(
        user.id,
        user.role.as_str(),
        &state.cfg.jwt_secret,
        state.cfg.jwt_ttl_seconds,
    )?;

    Ok(LoginResponse {
        token,
        expires_in: state.cfg.jwt_ttl_seconds,
        user: user.into(),
    })
}

pub async fn current_user(state: &AppState, user_id: Uuid) -> Result<UserProfile, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Forbidden);
    }

    Ok(user.into())
}
