use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct WalletView {
    pub currency: String,
    pub wallet_balance: Decimal,
    pub ai_credits: i32,
    pub pending_topups: i64,
}

#[derive(Debug, Serialize)]
pub struct TransactionView {
    pub id: Uuid,
    pub txn_type: String,
    pub amount: Decimal,
    pub balance_after: Decimal,
    pub reference_type: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TopupRequest {
    pub amount: Decimal,
    #[serde(default)]
    pub currency: Option<String>,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct TopupView {
    pub topup_id: Uuid,
    pub status: String,
    pub amount: Decimal,
    pub currency: String,
    /// `None` when no gateway redirect base is configured (local development).
    pub redirect_url: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceSummary {
    pub id: Uuid,
    pub invoice_number: String,
    pub currency: String,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub status: String,
    pub issued_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceDetail {
    #[serde(flatten)]
    pub summary: InvoiceSummary,
    pub order_id: Option<Uuid>,
    /// Signed URL for the rendered PDF; `None` until the PDF worker exists.
    pub pdf_url: Option<String>,
}
