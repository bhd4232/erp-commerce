use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Row for the team list. `role`/`status` are text so the service can parse
/// them into the public DTO without exposing SQLx enum details.
#[derive(Debug, sqlx::FromRow)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct InvitationRow {
    pub membership_id: Uuid,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub tenant_slug: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}
