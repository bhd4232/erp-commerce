//! Integration tests for top-ups, webhooks, wallet reads and invoices.

use std::sync::Arc;

use axum::body::Bytes;
use axum::http::HeaderMap;
use chrono::Utc;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use erp_commerce::common::pagination::Page;
use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::error::AppError;
use erp_commerce::modules::finance::dtos::TopupRequest;
use erp_commerce::modules::finance::{service, webhooks};
use erp_commerce::state::AppState;

fn dec(raw: &str) -> Decimal {
    raw.parse().expect("valid decimal")
}

fn config() -> Config {
    Config {
        host: "127.0.0.1".into(),
        port: 0,
        database_url: String::new(),
        database_max_connections: 2,
        redis_url: String::new(),
        jwt_secret: "test-secret-that-is-long-enough-32".into(),
        jwt_ttl_seconds: 3600,
        s3_endpoint: "http://127.0.0.1:9000".into(),
        s3_region: "auto".into(),
        s3_bucket: "test-bucket".into(),
        s3_access_key: "test".into(),
        s3_secret_key: "test".into(),
        signed_url_ttl_seconds: 900,
        signed_upload_ttl_seconds: 300,
        payment_webhook_secret: "test-webhook-secret".into(),
        payment_redirect_base: None,
    }
}

async fn app(pool: &PgPool) -> AppState {
    let cfg = config();
    let storage = Storage::new(&cfg).await;
    AppState {
        db: pool.clone(),
        cfg: Arc::new(cfg),
        storage,
    }
}

async fn user(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, full_name)
         VALUES ($1, 'x', 'Test User') RETURNING id",
    )
    .bind(format!(
        "user{}@example.com",
        Utc::now().timestamp_nanos_opt().unwrap()
    ))
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn balance(pool: &PgPool, user_id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT wallet_balance FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("balance")
}

async fn topup(app: &AppState, user_id: Uuid, amount: &str) -> Uuid {
    service::topup(
        app,
        user_id,
        TopupRequest {
            amount: dec(amount),
            currency: Some("USD".into()),
            provider: "stripe".into(),
        },
    )
    .await
    .expect("topup")
    .topup_id
}

fn event_body(event_id: &str, topup_id: Uuid, amount: &str) -> String {
    format!(
        r#"{{"id":"{event_id}","type":"payment.succeeded","data":{{"topup_id":"{topup_id}","amount":"{amount}","currency":"USD","gateway_ref":"ch_1"}}}}"#
    )
}

fn stripe_header(secret: &str, body: &[u8]) -> HeaderMap {
    let timestamp = Utc::now().timestamp();
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);

    let mut headers = HeaderMap::new();
    headers.insert(
        "stripe-signature",
        format!(
            "t={timestamp},v1={}",
            hex::encode(mac.finalize().into_bytes())
        )
        .parse()
        .unwrap(),
    );
    headers
}

#[sqlx::test(migrations = "./migrations")]
async fn topup_validates_amount_provider_and_precision(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool).await;

    let pending = service::topup(
        &app,
        user_id,
        TopupRequest {
            amount: dec("50.00"),
            currency: None,
            provider: "stripe".into(),
        },
    )
    .await
    .expect("topup");
    assert_eq!(pending.status, "pending");
    assert_eq!(pending.amount, dec("50.00"));
    assert!(pending.redirect_url.is_none());

    let too_small = service::topup(
        &app,
        user_id,
        TopupRequest {
            amount: dec("0.50"),
            currency: None,
            provider: "stripe".into(),
        },
    )
    .await;
    assert!(matches!(too_small, Err(AppError::Unprocessable(_))));

    let bad_provider = service::topup(
        &app,
        user_id,
        TopupRequest {
            amount: dec("10.00"),
            currency: None,
            provider: "paypal".into(),
        },
    )
    .await;
    assert!(matches!(bad_provider, Err(AppError::Unprocessable(_))));

    let bad_precision = service::topup(
        &app,
        user_id,
        TopupRequest {
            amount: dec("10.001"),
            currency: None,
            provider: "stripe".into(),
        },
    )
    .await;
    assert!(matches!(bad_precision, Err(AppError::Validation(_))));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_valid_webhook_credits_once_and_issues_an_invoice(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool).await;
    let topup_id = topup(&app, user_id, "50.00").await;

    let body = event_body("evt_1", topup_id, "50.00");
    let headers = stripe_header("test-webhook-secret", body.as_bytes());

    webhooks::handle(&app, "stripe", &headers, &Bytes::from(body.clone()))
        .await
        .expect("webhook");

    assert_eq!(balance(&pool, user_id).await, dec("50.00"));

    let (txn_type, gateway, amount): (String, Option<String>, Decimal) = sqlx::query_as(
        "SELECT txn_type::text, gateway, amount FROM wallet_transactions
          WHERE user_id = $1 AND txn_type = 'credit'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("ledger");
    assert_eq!(txn_type, "credit");
    assert_eq!(gateway.as_deref(), Some("stripe"));
    assert_eq!(amount, dec("50.00"));

    let status: String = sqlx::query_scalar("SELECT status::text FROM topup_intents WHERE id = $1")
        .bind(topup_id)
        .fetch_one(&pool)
        .await
        .expect("intent");
    assert_eq!(status, "paid");

    let invoices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("invoices");
    assert_eq!(invoices, 1);

    // Replaying the same event id must not credit twice.
    webhooks::handle(&app, "stripe", &headers, &Bytes::from(body))
        .await
        .expect("replay");
    assert_eq!(balance(&pool, user_id).await, dec("50.00"));

    let credits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_transactions WHERE user_id = $1 AND txn_type = 'credit'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("count credits");
    assert_eq!(credits, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_bad_signature_or_mismatched_amount_never_credits(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool).await;
    let topup_id = topup(&app, user_id, "50.00").await;

    let body = event_body("evt_bad", topup_id, "50.00");
    let mut headers = HeaderMap::new();
    headers.insert("stripe-signature", "t=1,v1=deadbeef".parse().unwrap());

    let rejected = webhooks::handle(&app, "stripe", &headers, &Bytes::from(body.clone())).await;
    assert!(matches!(rejected, Err(AppError::Validation(_))));
    assert_eq!(balance(&pool, user_id).await, dec("0"));

    let mismatched = event_body("evt_amount", topup_id, "10.00");
    let headers = stripe_header("test-webhook-secret", mismatched.as_bytes());
    let err = webhooks::handle(&app, "stripe", &headers, &Bytes::from(mismatched))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Unprocessable(_)));
    assert_eq!(balance(&pool, user_id).await, dec("0"));

    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payment_webhooks WHERE provider = 'stripe' AND status = 'failed'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed webhooks");
    assert_eq!(failed, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn wallet_and_transaction_reads_reflect_the_credit(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool).await;
    let topup_id = topup(&app, user_id, "25.00").await;

    let body = event_body("evt_read", topup_id, "25.00");
    let headers = stripe_header("test-webhook-secret", body.as_bytes());
    webhooks::handle(&app, "stripe", &headers, &Bytes::from(body))
        .await
        .expect("webhook");

    let wallet = service::wallet(&app, user_id).await.expect("wallet");
    assert_eq!(wallet.wallet_balance, dec("25.00"));
    assert_eq!(wallet.pending_topups, 0);

    let page = Page::new(None, None);
    let (transactions, total) = service::transactions(&app, user_id, &page)
        .await
        .expect("transactions");
    assert_eq!(total, 1);
    assert_eq!(transactions[0].amount, dec("25.00"));

    let (invoices, invoice_total) = service::invoices(&app, user_id, &page)
        .await
        .expect("invoices");
    assert_eq!(invoice_total, 1);

    let detail = service::invoice(&app, user_id, false, invoices[0].id)
        .await
        .expect("invoice detail");
    assert_eq!(detail.summary.total, dec("25.00"));
    assert!(detail.pdf_url.is_none());
}
