use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct OrderSummary {
    pub id: Uuid,
    pub order_number: String,
    pub external_id: String,
    pub status: String,
    pub total_amount: String,
    pub cod_amount: String,
    pub currency: String,
    pub placed_at: DateTime<Utc>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
}
