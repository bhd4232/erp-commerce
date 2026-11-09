//! Payment webhooks: signature verification, idempotency and the wallet credit
//! (docs/BILLING_WALLET_SPEC.md section 5).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use chrono::Utc;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

const TOLERANCE_SECONDS: i64 = 300;

pub fn router() -> Router<AppState> {
    Router::new().route("/payment/{provider}", post(receive))
}

async fn receive(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    handle(&state, &provider, &headers, &body).await?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
struct WebhookEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: WebhookData,
}

#[derive(Debug, Deserialize)]
struct WebhookData {
    topup_id: Uuid,
    amount: Decimal,
    currency: String,
    gateway_ref: Option<String>,
}

#[derive(sqlx::FromRow)]
struct IntentRow {
    id: Uuid,
    user_id: Uuid,
    amount: Decimal,
    currency: String,
    status: String,
    expires_at: chrono::DateTime<Utc>,
}

pub async fn handle(
    state: &AppState,
    provider: &str,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<(), AppError> {
    if !matches!(provider, "stripe" | "bkash" | "sslcommerz") {
        return Err(AppError::NotFound);
    }

    let secret = state.cfg.payment_webhook_secret.trim();
    if secret.is_empty() {
        return Err(AppError::Internal(
            "PAYMENT_WEBHOOK_SECRET is not configured".into(),
        ));
    }

    let valid = match provider {
        "stripe" => headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
            .map(|header| verify_stripe(secret, header, body))
            .unwrap_or(false),
        _ => headers
            .get("x-webhook-secret")
            .and_then(|v| v.to_str().ok())
            .map(|header| constant_time_eq(secret, header))
            .unwrap_or(false),
    };

    let parsed: Option<Value> = serde_json::from_slice(body).ok();
    let event_id = parsed
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    if !valid {
        sqlx::query(
            "INSERT INTO payment_webhooks
                (provider, event_id, payload, signature_valid, status)
             VALUES ($1, $2, $3, FALSE, 'rejected')
             ON CONFLICT (provider, event_id) DO NOTHING",
        )
        .bind(provider)
        .bind(&event_id)
        .bind(parsed.unwrap_or_else(|| json!({})))
        .execute(&state.db)
        .await?;

        return Err(AppError::Validation("invalid webhook signature".into()));
    }

    let event: WebhookEvent = serde_json::from_slice(body)
        .map_err(|e| AppError::Validation(format!("invalid webhook payload: {e}")))?;

    let webhook_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO payment_webhooks
            (provider, event_id, event_type, payload, signature_valid, status)
        VALUES ($1, $2, $3, $4, TRUE, 'verified')
        ON CONFLICT (provider, event_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(provider)
    .bind(&event.id)
    .bind(&event.event_type)
    .bind(parsed.unwrap_or_else(|| json!({})))
    .fetch_optional(&state.db)
    .await?;

    let Some(webhook_id) = webhook_id else {
        // Already seen: acknowledge without touching the ledger.
        return Ok(());
    };

    if event.event_type != "payment.succeeded" {
        mark(&state.db, webhook_id, "processed", None).await?;
        return Ok(());
    }

    let mut tx = state.db.begin().await?;
    match apply(&mut tx, provider, &event).await {
        Ok(()) => {
            mark(&mut *tx, webhook_id, "processed", None).await?;
            tx.commit().await?;
            Ok(())
        }
        Err(err) => {
            tx.rollback().await.ok();
            let _ = mark(&state.db, webhook_id, "failed", Some(err.to_string())).await;
            Err(err)
        }
    }
}

async fn apply(
    conn: &mut sqlx::PgConnection,
    provider: &str,
    event: &WebhookEvent,
) -> Result<(), AppError> {
    let intent = sqlx::query_as::<_, IntentRow>(
        "SELECT id, user_id, amount, currency, status::text AS status, expires_at
           FROM topup_intents WHERE id = $1 FOR UPDATE",
    )
    .bind(event.data.topup_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;

    match intent.status.as_str() {
        "paid" => return Ok(()),
        "pending" => {}
        other => return Err(AppError::InvalidState(format!("top-up is '{other}'"))),
    }

    if intent.expires_at <= Utc::now() {
        sqlx::query("UPDATE topup_intents SET status = 'expired' WHERE id = $1")
            .bind(intent.id)
            .execute(&mut *conn)
            .await?;
        return Ok(());
    }

    if intent.amount != event.data.amount
        || !intent.currency.eq_ignore_ascii_case(&event.data.currency)
    {
        return Err(AppError::Unprocessable(
            "amount or currency does not match the top-up".into(),
        ));
    }

    let gateway_ref = event
        .data
        .gateway_ref
        .clone()
        .unwrap_or_else(|| event.id.clone());

    let balance_after: Decimal = sqlx::query_scalar(
        "UPDATE users
            SET wallet_balance = wallet_balance + $1, updated_at = NOW()
          WHERE id = $2
        RETURNING wallet_balance",
    )
    .bind(intent.amount)
    .bind(intent.user_id)
    .fetch_one(&mut *conn)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type,
             reference_id, note, gateway, gateway_ref, currency)
        VALUES ($1, 'credit', $2, $3, 'topup', $4, $5, $6, $7, $8)
        "#,
    )
    .bind(intent.user_id)
    .bind(intent.amount)
    .bind(balance_after)
    .bind(intent.id)
    .bind(format!("top-up {}", intent.id))
    .bind(provider)
    .bind(&gateway_ref)
    .bind(&intent.currency)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "UPDATE topup_intents
            SET status = 'paid', paid_at = NOW(), provider_ref = $2
          WHERE id = $1",
    )
    .bind(intent.id)
    .bind(&gateway_ref)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO invoices
            (invoice_number, user_id, order_id, currency, subtotal, tax_amount,
             total, status, issued_at, paid_at)
        VALUES
            (next_invoice_number(), $1, NULL, $2, $3, 0, $3, 'paid', NOW(), NOW())
        "#,
    )
    .bind(intent.user_id)
    .bind(&intent.currency)
    .bind(intent.amount)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn mark(
    executor: impl sqlx::PgExecutor<'_>,
    webhook_id: Uuid,
    status: &str,
    error: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE payment_webhooks
            SET status = $2::webhook_status, processed_at = NOW(), error = $3
          WHERE id = $1",
    )
    .bind(webhook_id)
    .bind(status)
    .bind(error)
    .execute(executor)
    .await?;
    Ok(())
}

fn verify_stripe(secret: &str, header: &str, body: &[u8]) -> bool {
    let mut timestamp = None;
    let mut signature = None;

    for part in header.split(',') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("t=") {
            timestamp = value.parse::<i64>().ok();
        } else if let Some(value) = part.strip_prefix("v1=") {
            signature = Some(value);
        }
    }

    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return false;
    };
    if (Utc::now().timestamp() - timestamp).abs() > TOLERANCE_SECONDS {
        return false;
    }

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);

    match hex::decode(signature) {
        Ok(bytes) => mac.verify_slice(&bytes).is_ok(),
        Err(_) => false,
    }
}

fn constant_time_eq(expected: &str, provided: &str) -> bool {
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripe_header(secret: &str, timestamp: i64, body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{timestamp}.").as_bytes());
        mac.update(body);
        format!(
            "t={timestamp},v1={}",
            hex::encode(mac.finalize().into_bytes())
        )
    }

    #[test]
    fn stripe_signature_round_trip() {
        let body = br#"{"id":"evt_1"}"#;
        let header = stripe_header("s3cret", Utc::now().timestamp(), body);
        assert!(verify_stripe("s3cret", &header, body));
        assert!(!verify_stripe("wrong", &header, body));
    }

    #[test]
    fn stale_stripe_signature_is_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let header = stripe_header("s3cret", Utc::now().timestamp() - 3600, body);
        assert!(!verify_stripe("s3cret", &header, body));
    }

    #[test]
    fn shared_secret_comparison() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }
}
