use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Maps to the PostgreSQL `user_role` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Staff,
    Client,
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Staff => "staff",
            UserRole::Client => "client",
        }
    }
}

/// Row of `users`. `password_hash` must never be serialized outward.
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub role: UserRole,
    pub wallet_balance: Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}
