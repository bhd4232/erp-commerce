use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{User, UserRole};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Public projection of a user. Never carries `password_hash`.
#[derive(Debug, Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: UserRole,
    pub wallet_balance: Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserProfile {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            full_name: u.full_name,
            role: u.role,
            wallet_balance: u.wallet_balance,
            is_active: u.is_active,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: i64,
    pub user: UserProfile,
}
