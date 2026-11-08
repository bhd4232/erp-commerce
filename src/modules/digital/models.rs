use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Maps to the PostgreSQL `digital_product_type` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "digital_product_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DigitalProductType {
    PremadeFile,
    LicenseKey,
    DynamicTool,
}

#[derive(Debug, Clone, FromRow)]
pub struct DigitalProduct {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub product_type: DigitalProductType,
    pub price: Decimal,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProductFile {
    pub id: Uuid,
    pub product_id: Uuid,
    pub version: String,
    /// S3 key. Never serialized to a client.
    pub storage_path: String,
    pub file_size_bytes: i64,
    pub mime_type: String,
    pub checksum_sha256: Option<String>,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserEntitlement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub order_ref: String,
    pub download_count: i32,
    /// -1 means unlimited.
    pub max_downloads: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
