use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// Maps to the PostgreSQL `service_status` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "service_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    PendingBrief,
    InQueue,
    InProgress,
    UnderReview,
    RevisionRequested,
    Approved,
    Completed,
    Cancelled,
}

impl ServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ServiceStatus::PendingBrief => "pending_brief",
            ServiceStatus::InQueue => "in_queue",
            ServiceStatus::InProgress => "in_progress",
            ServiceStatus::UnderReview => "under_review",
            ServiceStatus::RevisionRequested => "revision_requested",
            ServiceStatus::Approved => "approved",
            ServiceStatus::Completed => "completed",
            ServiceStatus::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, ServiceStatus::Completed | ServiceStatus::Cancelled)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AgencyService {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub base_price: Decimal,
    /// JSON Schema-ish answer template used to validate `order_briefs.payload`.
    pub brief_template: Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ServiceOrder {
    pub id: Uuid,
    pub order_number: String,
    pub client_id: Uuid,
    pub service_id: Uuid,
    pub assigned_staff_id: Option<Uuid>,
    pub status: ServiceStatus,
    pub price_charged: Decimal,
    pub revision_limit: i32,
    pub revisions_used: i32,
    pub due_at: Option<DateTime<Utc>>,
    pub cancelled_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct OrderBrief {
    pub id: Uuid,
    pub order_id: Uuid,
    pub payload: Value,
    pub submitted_by: Uuid,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct OrderDeliverable {
    pub id: Uuid,
    pub order_id: Uuid,
    pub version: i32,
    pub watermarked_preview_path: Option<String>,
    pub final_asset_path: String,
    pub notes: Option<String>,
    pub uploaded_by: Uuid,
    pub is_approved: bool,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct OrderRevision {
    pub id: Uuid,
    pub order_id: Uuid,
    pub deliverable_id: Uuid,
    pub requested_by: Uuid,
    pub client_feedback: String,
    pub created_at: DateTime<Utc>,
}
