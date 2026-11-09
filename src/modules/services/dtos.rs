use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::models::ServiceStatus;

// --- Create ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateServiceOrderRequest {
    pub service_id: Uuid,
}

// --- Brief -------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubmitBriefRequest {
    pub payload: Value,
    #[serde(default)]
    pub uploaded_asset_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BriefSubmitted {
    pub status: ServiceStatus,
    pub submitted_at: DateTime<Utc>,
}

// --- Assignment --------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AssignRequest {
    pub assigned_staff_id: Uuid,
    pub due_at: DateTime<Utc>,
}

// --- Deliverable -------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubmitDeliverableRequest {
    /// Key under `service-deliverables/{order_id}/final/`.
    pub final_asset_key: String,
    /// Key under `service-deliverables/{order_id}/raw/`; the worker watermarks it.
    pub preview_asset_key: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeliverableSubmitted {
    pub deliverable_id: Uuid,
    pub version: i32,
    pub status: ServiceStatus,
    pub preview_status: String,
}

// --- Revision ----------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RequestRevisionRequest {
    pub deliverable_id: Uuid,
    pub feedback: String,
}

#[derive(Debug, Serialize)]
pub struct RevisionRequested {
    pub status: ServiceStatus,
    pub revisions_used: i32,
    pub revision_limit: i32,
}

// --- Approve / cancel --------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Approved {
    pub status: ServiceStatus,
    pub approved_deliverable_id: Uuid,
    pub download_available: bool,
}

#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Cancelled {
    pub status: ServiceStatus,
    pub refunded: Decimal,
}

// --- Read models -------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ServiceOrderSummary {
    pub id: Uuid,
    pub order_number: String,
    pub status: ServiceStatus,
    pub price_charged: Decimal,
    pub revision_limit: i32,
    pub revisions_used: i32,
    pub assigned_staff_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// True while the order still waits for its intake brief.
    pub brief_required: bool,
}

#[derive(Debug, Serialize)]
pub struct DeliverableView {
    pub id: Uuid,
    pub version: i32,
    /// Signed URL for the watermarked preview; `None` until the worker finishes.
    pub preview_url: Option<String>,
    pub notes: Option<String>,
    pub is_approved: bool,
}

#[derive(Debug, Serialize)]
pub struct DeliveryDownload {
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
    pub filename: String,
}

#[derive(Debug, Serialize)]
pub struct ServiceOrderDetail {
    pub id: Uuid,
    pub order_number: String,
    pub status: ServiceStatus,
    /// Machine-readable contract the client uses to enable buttons.
    pub allowed_actions: Vec<&'static str>,
    pub brief: Option<Value>,
    pub deliverables: Vec<DeliverableView>,
    pub revisions_used: i32,
    pub revision_limit: i32,
    pub due_at: Option<DateTime<Utc>>,
}
