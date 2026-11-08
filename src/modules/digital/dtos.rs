use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::DigitalProductType;

// --- Catalogue ---------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProductCard {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub product_type: DigitalProductType,
    pub price: Decimal,
    pub thumbnail_url: Option<String>,
    pub current_version: Option<String>,
    /// True when the authenticated caller already owns this product.
    pub owned: bool,
}

#[derive(Debug, Serialize)]
pub struct ProductDetail {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub product_type: DigitalProductType,
    pub price: Decimal,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    /// Metadata only — never a storage key or a signed URL.
    pub file: Option<PublicFileInfo>,
    pub owned: bool,
}

#[derive(Debug, Serialize)]
pub struct PublicFileInfo {
    pub version: String,
    pub file_size_bytes: i64,
    pub mime_type: String,
}

// --- Vault -------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct VaultProductRef {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
}

#[derive(Debug, Serialize)]
pub struct VaultItem {
    pub entitlement_id: Uuid,
    pub product: VaultProductRef,
    /// The version that will actually be served on the next download.
    pub version: String,
    pub download_count: i32,
    pub max_downloads: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// --- Download ----------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
    pub filename: String,
}

// --- Checkout ----------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CheckoutItem {
    pub product_id: Uuid,
    #[serde(default = "one")]
    pub quantity: i32,
}

fn one() -> i32 {
    1
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub items: Vec<CheckoutItem>,
    /// "wallet" is the only provider implemented in v1.
    pub payment_provider: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub order_id: Uuid,
    pub order_number: String,
    pub status: String,
    pub total_amount: Decimal,
    pub entitlements_created: usize,
}
