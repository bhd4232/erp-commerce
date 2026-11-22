//! Tenant context extracted from `X-Tenant-Id` plus an active membership.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::extractors::AuthUser;
use crate::error::AppError;
use crate::state::AppState;

pub const TENANT_HEADER: &str = "x-tenant-id";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TenantRole {
    Owner,
    Admin,
    Staff,
}

impl TenantRole {
    pub fn as_str(self) -> &'static str {
        match self {
            TenantRole::Owner => "owner",
            TenantRole::Admin => "admin",
            TenantRole::Staff => "staff",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "owner" => Some(TenantRole::Owner),
            "admin" => Some(TenantRole::Admin),
            "staff" => Some(TenantRole::Staff),
            _ => None,
        }
    }

    pub fn can_manage(self) -> bool {
        matches!(self, TenantRole::Owner | TenantRole::Admin)
    }
}

/// Active tenant and the caller's role inside it.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub role: TenantRole,
    pub tenant_name: String,
    pub tenant_slug: String,
}

impl TenantContext {
    pub fn require_role(&self, allowed: &[TenantRole]) -> Result<(), AppError> {
        if allowed.contains(&self.role) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    pub fn require_manager(&self) -> Result<(), AppError> {
        if self.role.can_manage() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

impl FromRequestParts<AppState> for TenantContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthUser::from_request_parts(parts, state).await?;

        let raw = parts
            .headers
            .get(TENANT_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Validation("X-Tenant-Id header is required".into()))?;

        let tenant_id = Uuid::parse_str(raw.trim())
            .map_err(|_| AppError::Validation("X-Tenant-Id must be a UUID".into()))?;

        let row: Option<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT m.role::text, t.name, t.slug
              FROM memberships m
              JOIN tenants t ON t.id = m.tenant_id
              JOIN users u ON u.id = m.user_id
             WHERE m.tenant_id = $1
               AND m.user_id = $2
               AND m.status = 'active'
               AND t.status = 'active'
               AND u.is_active = true
            "#,
        )
        .bind(tenant_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?;

        let (role, tenant_name, tenant_slug) = row.ok_or(AppError::NotFound)?;
        let role = TenantRole::parse(&role)
            .ok_or_else(|| AppError::Internal(format!("unknown tenant role '{role}'")))?;

        Ok(TenantContext {
            tenant_id,
            user_id: auth.id,
            role,
            tenant_name,
            tenant_slug,
        })
    }
}
