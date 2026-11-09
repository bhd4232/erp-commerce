//! Service-order transitions (docs/WORKFLOWS.md section 1).
//!
//! Every function locks the order row with `FOR UPDATE` before checking the
//! current status. The status a client sends is never trusted.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

use super::dtos::{
    Approved, AssignRequest, BriefSubmitted, CancelRequest, Cancelled, DeliverableSubmitted,
    RequestRevisionRequest, RevisionRequested, ServiceOrderSummary,
};
use super::models::ServiceStatus;
use super::service::OrderSummaryRow;
use crate::common::extractors::{AuthUser, Role};
use crate::error::AppError;
use crate::state::AppState;

pub async fn submit_brief(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
    req: super::dtos::SubmitBriefRequest,
) -> Result<BriefSubmitted, AppError> {
    actor.require_role(&[Role::Client])?;

    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, ServiceStatus, Uuid)> = sqlx::query_as(
        "SELECT client_id, status, service_id
           FROM service_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (client_id, status, service_id) = row.ok_or(AppError::NotFound)?;

    if client_id != actor.id {
        return Err(AppError::Forbidden);
    }
    if status != ServiceStatus::PendingBrief {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }

    let own_prefix = format!("user-uploads/{}/", actor.id);
    if req
        .uploaded_asset_keys
        .iter()
        .any(|key| !key.starts_with(&own_prefix))
    {
        return Err(AppError::Forbidden);
    }

    let template: Value =
        sqlx::query_scalar("SELECT brief_template FROM agency_services WHERE id = $1")
            .bind(service_id)
            .fetch_one(&mut *tx)
            .await?;

    let missing = missing_required(&template, &req.payload);
    if !missing.is_empty() {
        return Err(AppError::BriefIncomplete(missing));
    }

    let submitted_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO order_briefs (order_id, payload, submitted_by)
         VALUES ($1, $2, $3) RETURNING submitted_at",
    )
    .bind(order_id)
    .bind(&req.payload)
    .bind(actor.id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE service_orders SET status = 'in_queue', updated_at = NOW() WHERE id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(BriefSubmitted {
        status: ServiceStatus::InQueue,
        submitted_at,
    })
}

pub async fn assign(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
    req: AssignRequest,
) -> Result<ServiceOrderSummary, AppError> {
    actor.require_role(&[Role::Admin, Role::Staff])?;

    if req.due_at <= Utc::now() {
        return Err(AppError::Unprocessable(
            "due_at must be in the future".into(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let row: Option<(ServiceStatus, Option<Uuid>)> = sqlx::query_as(
        "SELECT status, assigned_staff_id
           FROM service_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, assigned_staff_id) = row.ok_or(AppError::NotFound)?;

    if !matches!(status, ServiceStatus::InQueue | ServiceStatus::InProgress) {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }
    if actor.role == Role::Staff
        && (assigned_staff_id.is_some() && assigned_staff_id != Some(actor.id)
            || req.assigned_staff_id != actor.id)
    {
        return Err(AppError::Forbidden);
    }

    let assignee: Option<(String, bool)> =
        sqlx::query_as("SELECT role::text, is_active FROM users WHERE id = $1")
            .bind(req.assigned_staff_id)
            .fetch_optional(&mut *tx)
            .await?;

    match assignee {
        Some((role, true)) if role == "staff" => {}
        _ => {
            return Err(AppError::Unprocessable(
                "assignee must be an active staff user".into(),
            ))
        }
    }

    let summary = sqlx::query_as::<_, OrderSummaryRow>(
        r#"
        UPDATE service_orders
           SET assigned_staff_id = $1,
               due_at = $2,
               status = CASE WHEN status = 'in_queue'
                             THEN 'in_progress'::service_status
                             ELSE status END,
               updated_at = NOW()
         WHERE id = $3
        RETURNING id, order_number, status, price_charged, revision_limit,
                  revisions_used, assigned_staff_id, due_at, created_at
        "#,
    )
    .bind(req.assigned_staff_id)
    .bind(req.due_at)
    .bind(order_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(summary.into())
}

pub async fn submit_deliverable(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
    req: super::dtos::SubmitDeliverableRequest,
) -> Result<DeliverableSubmitted, AppError> {
    actor.require_role(&[Role::Staff, Role::Admin])?;

    let mut tx = state.db.begin().await?;

    let row: Option<(ServiceStatus, Option<Uuid>)> = sqlx::query_as(
        "SELECT status, assigned_staff_id
           FROM service_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, assigned_staff_id) = row.ok_or(AppError::NotFound)?;

    if !matches!(
        status,
        ServiceStatus::InProgress | ServiceStatus::RevisionRequested
    ) {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }
    if actor.role == Role::Staff && assigned_staff_id != Some(actor.id) {
        return Err(AppError::Forbidden);
    }

    let final_prefix = format!("service-deliverables/{order_id}/final/");
    let raw_prefix = format!("service-deliverables/{order_id}/raw/");
    if !req.final_asset_key.starts_with(&final_prefix)
        || !req.preview_asset_key.starts_with(&raw_prefix)
    {
        return Err(AppError::Unprocessable(
            "asset keys must live under this order's final/ and raw/ prefixes".into(),
        ));
    }

    let version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM order_deliverables WHERE order_id = $1",
    )
    .bind(order_id)
    .fetch_one(&mut *tx)
    .await?;

    let deliverable_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO order_deliverables
            (order_id, version, source_preview_path, final_asset_path, notes, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(order_id)
    .bind(version)
    .bind(&req.preview_asset_key)
    .bind(&req.final_asset_key)
    .bind(&req.notes)
    .bind(actor.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(DeliverableSubmitted {
        deliverable_id,
        version,
        status,
        preview_status: "processing".to_string(),
    })
}

pub async fn request_revision(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
    req: RequestRevisionRequest,
) -> Result<RevisionRequested, AppError> {
    actor.require_role(&[Role::Client])?;

    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, ServiceStatus, i32, i32)> = sqlx::query_as(
        "SELECT client_id, status, revisions_used, revision_limit
           FROM service_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (client_id, status, revisions_used, revision_limit) = row.ok_or(AppError::NotFound)?;

    if client_id != actor.id {
        return Err(AppError::Forbidden);
    }
    if status != ServiceStatus::UnderReview {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }
    if revisions_used >= revision_limit {
        return Err(AppError::RevisionLimitReached);
    }

    let belongs: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM order_deliverables WHERE id = $1 AND order_id = $2")
            .bind(req.deliverable_id)
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?;
    if belongs.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO order_revisions (order_id, deliverable_id, requested_by, client_feedback)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(order_id)
    .bind(req.deliverable_id)
    .bind(actor.id)
    .bind(&req.feedback)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE service_orders
            SET revisions_used = revisions_used + 1,
                status = 'revision_requested',
                updated_at = NOW()
          WHERE id = $1",
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(RevisionRequested {
        status: ServiceStatus::RevisionRequested,
        revisions_used: revisions_used + 1,
        revision_limit,
    })
}

pub async fn approve(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
) -> Result<Approved, AppError> {
    actor.require_role(&[Role::Client])?;

    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, ServiceStatus)> =
        sqlx::query_as("SELECT client_id, status FROM service_orders WHERE id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (client_id, status) = row.ok_or(AppError::NotFound)?;

    if client_id != actor.id {
        return Err(AppError::Forbidden);
    }
    if status != ServiceStatus::UnderReview {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }

    let deliverable_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM order_deliverables
          WHERE order_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let deliverable_id = deliverable_id.ok_or(AppError::NotFound)?;

    sqlx::query(
        "UPDATE order_deliverables
            SET is_approved = TRUE, approved_at = NOW()
          WHERE id = $1",
    )
    .bind(deliverable_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE service_orders SET status = 'completed', updated_at = NOW() WHERE id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Approved {
        status: ServiceStatus::Completed,
        approved_deliverable_id: deliverable_id,
        download_available: true,
    })
}

pub async fn cancel(
    state: &AppState,
    actor: &AuthUser,
    order_id: Uuid,
    req: CancelRequest,
) -> Result<Cancelled, AppError> {
    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, ServiceStatus, Decimal)> = sqlx::query_as(
        "SELECT client_id, status, price_charged
           FROM service_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (client_id, status, price) = row.ok_or(AppError::NotFound)?;

    if status.is_terminal() {
        return Err(AppError::InvalidState(format!(
            "order is '{}'",
            status.as_str()
        )));
    }

    match actor.role {
        Role::Admin => {}
        Role::Client if actor.id == client_id => {
            if !matches!(status, ServiceStatus::PendingBrief | ServiceStatus::InQueue) {
                return Err(AppError::InvalidState(format!(
                    "order is '{}'",
                    status.as_str()
                )));
            }
        }
        _ => return Err(AppError::Forbidden),
    }

    let balance_after: Decimal = sqlx::query_scalar(
        "UPDATE users SET wallet_balance = wallet_balance + $1, updated_at = NOW()
          WHERE id = $2
        RETURNING wallet_balance",
    )
    .bind(price)
    .bind(client_id)
    .fetch_one(&mut *tx)
    .await?;

    let order_number: String =
        sqlx::query_scalar("SELECT order_number FROM service_orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query(
        r#"
        INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type,
             reference_id, note, gateway, currency)
        VALUES ($1, 'refund', $2, $3, 'service_order', $4, $5, 'internal', 'USD')
        "#,
    )
    .bind(client_id)
    .bind(price)
    .bind(balance_after)
    .bind(order_id)
    .bind(format!("cancel {order_number}"))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE service_orders
            SET status = 'cancelled', cancelled_reason = $1, updated_at = NOW()
          WHERE id = $2",
    )
    .bind(&req.reason)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Cancelled {
        status: ServiceStatus::Cancelled,
        refunded: price,
    })
}

fn missing_required(template: &Value, payload: &Value) -> Vec<String> {
    let required = template
        .get("required")
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let object = payload.as_object();
    required
        .into_iter()
        .filter(|key| match object {
            Some(map) => map.get(key).map(Value::is_null).unwrap_or(true),
            None => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_required_lists_every_absent_key() {
        let template = json!({ "required": ["brand_name", "style", "references"] });
        let payload = json!({ "brand_name": "Acme", "style": null });

        assert_eq!(
            missing_required(&template, &payload),
            vec!["style".to_string(), "references".to_string()]
        );
    }

    #[test]
    fn a_complete_payload_has_no_missing_keys() {
        let template = json!({ "required": ["brand_name"] });
        let payload = json!({ "brand_name": "Acme" });
        assert!(missing_required(&template, &payload).is_empty());
    }

    #[test]
    fn a_template_without_required_keys_accepts_anything() {
        let template = json!({});
        let payload = json!({});
        assert!(missing_required(&template, &payload).is_empty());
    }
}
