use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PresignRequest {
    pub purpose: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct PresignResponse {
    pub upload_url: String,
    pub key: String,
    pub expires_at: DateTime<Utc>,
    pub max_size_bytes: i64,
}
