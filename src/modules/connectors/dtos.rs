use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateConnectorRequest {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectorSummary {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreatedConnector {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub webhook_path: Option<String>,
    /// Returned exactly once at creation; never stored by the client.
    pub webhook_secret: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResult {
    pub status: String,
    pub order_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub processed: usize,
}

#[derive(Debug, Deserialize)]
pub struct CsvOrder {
    pub order_id: String,
    #[serde(default)]
    pub order_number: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub total: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub placed_at: Option<String>,
    #[serde(default)]
    pub customer_name: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address_1: Option<String>,
    #[serde(default)]
    pub address_2: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}
