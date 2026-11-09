//! Agency service business rules.
//!
//! This is the authoritative implementation of the state machine in
//! docs/WORKFLOWS.md section 1. Every transition re-reads the order row with
//! `SELECT ... FOR UPDATE` before validating; never trust a status passed by
//! the client.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

use super::dtos::{
    Approved, AssignRequest, BriefSubmitted, CancelRequest, Cancelled, CreateServiceOrderRequest,
    DeliverableSubmitted, DeliverableView, DeliveryDownload, RequestRevisionRequest,
    RevisionRequested, ServiceOrderDetail, ServiceOrderSummary, SubmitBriefRequest,
    SubmitDeliverableRequest,
};
use super::models::ServiceStatus;
use crate::common::extractors::{AuthUser, Role};
use crate::common::pagination::Page;
use crate::error::AppError;
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

#[derive(sqlx::FromRow)]
struct OrderSummaryRow {
    id: Uuid,
    order_number: String,
    status: ServiceStatus,
    price_charged: Decimal,
    revision_limit: i32,
    revisions_used: i32,
    assigned_staff_id: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<OrderSummaryRow> for ServiceOrderSummary {
    fn from(row: OrderSummaryRow) -> Self {
        Self {
            id: row.id,
            order_number: row.order_number,
            brief_required: row.status == ServiceStatus::PendingBrief,
            status: row.status,
            price_charged: row.price_charged,
            revision_limit: row.revision_limit,
            revisions_used: row.revisions_used,
            assigned_staff_id: row.assigned_staff_id,
            due_at: row.due_at,
            created_at: row.created_at,
        }
    }
}

pub async fn create_order(
    state: &AppState,
    client_id: Uuid,
    req: CreateServiceOrderRequest,
) -> Result<ServiceOrderSummary, AppError> {
    let mut tx = state.db.begin().await?;

    let price: Option<Decimal> =
        sqlx::query_scalar("SELECT base_price FROM agency_services WHERE id = $1 AND is_active")
            .bind(req.service_id)
            .fetch_optional(&mut *tx)
            .await?;
    let price = price.ok_or_else(|| AppError::Unprocessable("service is not available".into()))?;

    let (id, order_number, revision_limit, created_at): (Uuid, String, i32, DateTime<Utc>) =
        sqlx::query_as(
            r#"
            INSERT INTO service_orders
                (order_number, client_id, service_id, status, price_charged)
            VALUES (next_order_number('SVC'), $1, $2, 'pending_brief', $3)
            RETURNING id, order_number, revision_limit, created_at
            "#,
        )
        .bind(client_id)
        .bind(req.service_id)
        .bind(price)
        .fetch_one(&mut *tx)
        .await?;

    let balance_after: Option<Decimal> = sqlx::query_scalar(
        r#"
        UPDATE users
           SET wallet_balance = wallet_balance - $1,
               updated_at = NOW()
         WHERE id = $2 AND wallet_balance >= $1
        RETURNING wallet_balance
        "#,
    )
    .bind(price)
    .bind(client_id)
    .fetch_optional(&mut *tx)
    .await?;
    let balance_after = balance_after.ok_or(AppError::InsufficientFunds)?;

    sqlx::query(
        r#"
        INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type,
             reference_id, note, gateway, currency)
        VALUES ($1, 'debit', $2, $3, 'service_order', $4, $5, 'internal', 'USD')
        "#,
    )
    .bind(client_id)
    .bind(price)
    .bind(balance_after)
    .bind(id)
    .bind(format!("service order {order_number}"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(ServiceOrderSummary {
        id,
        order_number,
        status: ServiceStatus::PendingBrief,
        price_charged: price,
        revision_limit,
        revisions_used: 0,
        assigned_staff_id: None,
        due_at: None,
        created_at,
        brief_required: true,
    })
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
    state: &AppState,
    actor: &AuthUser,
    page: &Page,
    status: Option<ServiceStatus>,
    assignee_me: bool,
) -> Result<(Vec<ServiceOrderSummary>, i64), AppError> {
    let role = actor.role.as_str();

    let rows = sqlx::query_as::<_, OrderSummaryRow>(
        r#"
        SELECT o.id, o.order_number, o.status, o.price_charged, o.revision_limit,
               o.revisions_used, o.assigned_staff_id, o.due_at, o.created_at
          FROM service_orders o
         WHERE ($1 = 'admin'
                OR ($1 = 'staff' AND o.assigned_staff_id = $2)
                OR ($1 = 'client' AND o.client_id = $2))
           AND ($3::service_status IS NULL OR o.status = $3)
           AND ($4::boolean = FALSE OR o.assigned_staff_id = $2)
         ORDER BY o.created_at DESC
         LIMIT $5 OFFSET $6
        "#,
    )
    .bind(role)
    .bind(actor.id)
    .bind(status)
    .bind(assignee_me)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
          FROM service_orders o
         WHERE ($1 = 'admin'
                OR ($1 = 'staff' AND o.assigned_staff_id = $2)
                OR ($1 = 'client' AND o.client_id = $2))
           AND ($3::service_status IS NULL OR o.status = $3)
           AND ($4::boolean = FALSE OR o.assigned_staff_id = $2)
        "#,
    )
    .bind(role)
    .bind(actor.id)
    .bind(status)
    .bind(assignee_me)
    .fetch_one(&state.db)
    .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total))
}

#[derive(sqlx::FromRow)]
struct OrderDetailRow {
    id: Uuid,
    order_number: String,
    client_id: Uuid,
    assigned_staff_id: Option<Uuid>,
    status: ServiceStatus,
    revision_limit: i32,
    revisions_used: i32,
    due_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct DeliverableRow {
    id: Uuid,
    version: i32,
    watermarked_preview_path: Option<String>,
    notes: Option<String>,
    is_approved: bool,
}

pub async fn get_order(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
) -> Result<ServiceOrderDetail, AppError> {
    let order = sqlx::query_as::<_, OrderDetailRow>(
        r#"
        SELECT id, order_number, client_id, assigned_staff_id, status,
               revision_limit, revisions_used, due_at
          FROM service_orders
         WHERE id = $1
        "#,
    )
    .bind(order_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let visible = match actor.role {
        Role::Admin => true,
        Role::Client => order.client_id == actor.id,
        Role::Staff => order.assigned_staff_id == Some(actor.id),
    };
    if !visible {
        return Err(AppError::NotFound);
    }

    let is_owner = order.client_id == actor.id;

    let brief: Option<Value> =
        sqlx::query_scalar("SELECT payload FROM order_briefs WHERE order_id = $1")
            .bind(order_id)
            .fetch_optional(&state.db)
            .await?;

    let rows = sqlx::query_as::<_, DeliverableRow>(
        r#"
        SELECT id, version, watermarked_preview_path, notes, is_approved
          FROM order_deliverables
         WHERE order_id = $1
         ORDER BY version DESC
        "#,
    )
    .bind(order_id)
    .fetch_all(&state.db)
    .await?;

    let mut deliverables = Vec::with_capacity(rows.len());
    for row in rows {
        let preview_url = match &row.watermarked_preview_path {
            Some(path) => Some(
                state
                    .storage
                    .presign_get(path, filename_of(path), mime_for(path))
                    .await?,
            ),
            None => None,
        };

        deliverables.push(DeliverableView {
            id: row.id,
            version: row.version,
            preview_url,
            notes: row.notes,
            is_approved: row.is_approved,
        });
    }

    Ok(ServiceOrderDetail {
        id: order.id,
        order_number: order.order_number,
        allowed_actions: allowed_actions(order.status, actor.role, is_owner),
        status: order.status,
        brief,
        deliverables,
        revisions_used: order.revisions_used,
        revision_limit: order.revision_limit,
        due_at: order.due_at,
    })
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

fn filename_of(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or("preview")
}

fn mime_for(key: &str) -> &'static str {
    match key.rsplit('.').next() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_actions_follow_status_and_ownership() {
        assert_eq!(
            allowed_actions(ServiceStatus::PendingBrief, Role::Client, true),
            vec!["submit_brief", "cancel"]
        );
        assert!(allowed_actions(ServiceStatus::PendingBrief, Role::Client, false).is_empty());
        assert_eq!(
            allowed_actions(ServiceStatus::UnderReview, Role::Client, true),
            vec!["revision", "approve"]
        );
        // A completed order is terminal, but the owner can still download it.
        assert_eq!(
            allowed_actions(ServiceStatus::Completed, Role::Client, true),
            vec!["download"]
        );
        assert!(allowed_actions(ServiceStatus::Completed, Role::Client, false).is_empty());
    }

    #[test]
    fn staff_and_admin_actions() {
        assert_eq!(
            allowed_actions(ServiceStatus::InProgress, Role::Staff, false),
            vec!["submit_deliverable"]
        );
        assert_eq!(
            allowed_actions(ServiceStatus::InQueue, Role::Admin, false),
            vec!["assign", "cancel"]
        );
        assert!(allowed_actions(ServiceStatus::UnderReview, Role::Staff, false).is_empty());
    }
}
