use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::tenant::TenantRole;

#[derive(Debug, Serialize)]
pub struct TenantSummary {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: TenantRole,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct TeamMember {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: TenantRole,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PendingInvitation {
    pub membership_id: Uuid,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub tenant_slug: String,
    pub role: TenantRole,
    pub invited_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct InviteRequest {
    pub email: String,
    pub role: TenantRole,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: Option<TenantRole>,
    pub status: Option<String>,
}
