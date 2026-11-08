//! Password hashing and JWT issuance/verification.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Claims carried by every access token. Deliberately minimal: no PII.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User id.
    pub sub: Uuid,
    pub role: String,
    /// Issued-at, unix seconds.
    pub iat: i64,
    /// Expiry, unix seconds.
    pub exp: i64,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))
}

pub fn verify_password(password: &str, stored_hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| AppError::Internal(format!("invalid stored password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn issue_token(
    user_id: Uuid,
    role: &str,
    secret: &str,
    ttl_seconds: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        role: role.to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::seconds(ttl_seconds)).timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("token signing failed: {e}")))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    // Any decoding failure is an auth failure, with no detail leaked.
    .map_err(|_| AppError::Unauthorized)?;

    Ok(data.claims)
}
