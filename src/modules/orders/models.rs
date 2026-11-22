use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct OrderRow {
    pub id: Uuid,
    pub order_number: String,
    pub external_id: String,
    pub status: String,
    pub total_amount: Decimal,
    pub cod_amount: Decimal,
    pub currency: String,
    pub placed_at: DateTime<Utc>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
}
