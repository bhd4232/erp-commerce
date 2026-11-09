//! Axum extractors: authenticated caller and role guards.

use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::common::auth::decode_token;
use crate::error::AppError;
use crate::state::AppState;

/// The authenticated caller, derived from the `Authorization: Bearer` header.
///
/// Handlers that take `AuthUser` are authenticated by construction; handlers
/// that do not are public. There is no other auth path.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Staff,
    Client,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Staff => "staff",
            Role::Client => "client",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "admin" => Some(Role::Admin),
            "staff" => Some(Role::Staff),
            "client" => Some(Role::Client),
            _ => None,
        }
    }
}

impl AuthUser {
    /// Guard for role-restricted routes. Returns 403, never 404: the resource
    /// is known to exist, the caller simply may not act on it.
    pub fn require_role(&self, allowed: &[Role]) -> Result<(), AppError> {
        if allowed.contains(&self.role) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let token = raw
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(AppError::Unauthorized)?;

        let claims = decode_token(token, &state.cfg.jwt_secret)?;
        let role = Role::parse(&claims.role).ok_or(AppError::Unauthorized)?;

        // NOTE: `users.is_active` must be re-checked by the service layer on
        // privileged actions, or by a short-lived cache in front of the DB.
        Ok(AuthUser {
            id: claims.sub,
            role,
        })
    }
}

/// Allows `Option<AuthUser>` in handlers (public endpoints that enrich the
/// response when the caller happens to be logged in).
///
/// A missing header yields `None`. A header that is present but invalid is
/// still an error — an expired token must not silently downgrade a user.
impl OptionalFromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        let Some(raw) = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        else {
            return Ok(None);
        };

        let token = raw
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(AppError::Unauthorized)?;

        let claims = decode_token(token, &state.cfg.jwt_secret)?;
        let role = Role::parse(&claims.role).ok_or(AppError::Unauthorized)?;

        Ok(Some(AuthUser {
            id: claims.sub,
            role,
        }))
    }
}
