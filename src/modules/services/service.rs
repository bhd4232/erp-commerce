//! Agency service business rules.
//!
//! This is the authoritative implementation of the state machine in
//! docs/WORKFLOWS.md section 1. Every transition re-reads the order row with
//! `SELECT ... FOR UPDATE` before validating; never trust a status passed by
//! the client.

use uuid::Uuid;

use super::dtos::{
    Approved, AssignRequest, BriefSubmitted, CancelRequest, Cancelled, CreateServiceOrderRequest,
    DeliverableSubmitted, DeliveryDownload, RequestRevisionRequest, RevisionRequested,
    ServiceOrderDetail, ServiceOrderSummary, SubmitBriefRequest, SubmitDeliverableRequest,
};
use super::models::ServiceStatus;
use crate::common::extractors::{AuthUser, Role};
use crate::error::AppError;
use crate::modules::digital::service::Page;
use crate::state::AppState;

/// Pure state/role check used by `GET /services/orders/{id}` so the client
/// never re-implements the rules.
pub fn allowed_actions(status: ServiceStatus, role: Role, is_owner: bool) -> Vec<&'static str> {
    match (status, role) {
        (ServiceStatus::PendingBrief, Role::Client) if is_owner => vec!["submit_brief", "cancel"],
        (ServiceStatus::InQueue, _) if role == Role::Admin => vec!["assign", "cancel"],
        (ServiceStatus::InQueue, Role::Client) if is_owner => vec!["cancel"],
        (ServiceStatus::InProgress, _) if role == Role::Admin => vec!["assign", "cancel"],
        (ServiceStatus::InProgress, Role::Staff) => vec!["submit_deliverable"],
        (ServiceStatus::UnderReview, Role::Client) if is_owner => vec!["revision", "approve"],
        (ServiceStatus::RevisionRequested, Role::Staff) => vec!["submit_deliverable"],
        (ServiceStatus::Completed, Role::Client) if is_owner => vec!["download"],
        (ServiceStatus::Cancelled, _) | (ServiceStatus::Completed, _) => vec![],
        (_, Role::Admin) => vec!["cancel"],
        _ => vec![],
    }
}

pub async fn create_order(
    _state: &AppState,
    _client_id: Uuid,
    _req: CreateServiceOrderRequest,
) -> Result<ServiceOrderSummary, AppError> {
    // TODO: load active agency_service, generate order_number, debit wallet,
    // insert service_orders with status = 'pending_brief'.
    Err(AppError::NotImplemented("services::create_order"))
}

pub async fn submit_brief(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _req: SubmitBriefRequest,
) -> Result<BriefSubmitted, AppError> {
    // Guards: owner, status == pending_brief, payload matches brief_template.
    // Then: INSERT order_briefs, status -> in_queue.
    Err(AppError::NotImplemented("services::submit_brief"))
}

pub async fn assign(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _req: AssignRequest,
) -> Result<ServiceOrderSummary, AppError> {
    // Allowed from in_queue (-> in_progress) or in_progress (reassignment).
    // Assignee must have role = 'staff'; due_at must be in the future.
    Err(AppError::NotImplemented("services::assign"))
}

pub async fn submit_deliverable(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _req: SubmitDeliverableRequest,
) -> Result<DeliverableSubmitted, AppError> {
    // Allowed from in_progress and revision_requested.
    // Insert order_deliverables with version = max(version) + 1, keep the order
    // in in_progress, enqueue the watermark worker. The order moves to
    // under_review only when the worker has written the preview.
    Err(AppError::NotImplemented("services::submit_deliverable"))
}

pub async fn request_revision(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _req: RequestRevisionRequest,
) -> Result<RevisionRequested, AppError> {
    // SELECT ... FOR UPDATE; require status == under_review and
    // revisions_used < revision_limit; increment; INSERT order_revisions.
    Err(AppError::NotImplemented("services::request_revision"))
}

pub async fn approve(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
) -> Result<Approved, AppError> {
    // Require status == under_review; set is_approved/approved_at on the latest
    // deliverable; status -> completed.
    Err(AppError::NotImplemented("services::approve"))
}

pub async fn cancel(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _req: CancelRequest,
) -> Result<Cancelled, AppError> {
    // Client may cancel only pending_brief / in_queue; admin any non-terminal.
    // Refund through the wallet ledger in the same transaction.
    Err(AppError::NotImplemented("services::cancel"))
}

pub async fn list_orders(
    _state: &AppState,
    _actor: &AuthUser,
    _page: &Page,
) -> Result<Vec<ServiceOrderSummary>, AppError> {
    // Role-scoped: client -> own, staff -> assigned, admin -> all.
    Err(AppError::NotImplemented("services::list_orders"))
}

pub async fn get_order(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
) -> Result<ServiceOrderDetail, AppError> {
    Err(AppError::NotImplemented("services::get_order"))
}

pub async fn deliverable_download(
    _state: &AppState,
    _actor: &AuthUser,
    _order_id: Uuid,
    _deliverable_id: Uuid,
) -> Result<DeliveryDownload, AppError> {
    // Sign the final asset ONLY when status == completed and is_approved.
    // While under_review / revision_requested this must be 403, not a preview.
    Err(AppError::NotImplemented("services::deliverable_download"))
}
