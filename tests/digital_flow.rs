//! Integration tests for the P1 money paths. Each test gets a fresh database
//! with all migrations applied; nothing is mocked.

use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::error::AppError;
use erp_commerce::modules::digital::commerce;
use erp_commerce::modules::digital::dtos::{CheckoutItem, CheckoutRequest};
use erp_commerce::modules::digital::service;
use erp_commerce::state::AppState;

fn dec(raw: &str) -> Decimal {
    raw.parse().expect("valid decimal")
}

fn config() -> Config {
    Config {
        host: "127.0.0.1".into(),
        port: 0,
        database_url: String::new(),
        database_max_connections: 1,
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

async fn user(pool: &PgPool, email: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, full_name)
         VALUES ($1, 'x', 'Test User') RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

/// Credits the wallet and writes the matching ledger row, like seed_wallet().
async fn fund(pool: &PgPool, user_id: Uuid, amount: &str) {
    let amount = dec(amount);
    let mut tx = pool.begin().await.expect("begin");
    let balance: Decimal = sqlx::query_scalar(
        "UPDATE users SET wallet_balance = wallet_balance + $1
          WHERE id = $2 RETURNING wallet_balance",
    )
    .bind(amount)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .expect("credit balance");

    sqlx::query(
        "INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type, note)
         VALUES ($1, 'credit', $2, $3, 'manual', 'test funding')",
    )
    .bind(user_id)
    .bind(amount)
    .bind(balance)
    .execute(&mut *tx)
    .await
    .expect("ledger row");

    tx.commit().await.expect("commit");
}

async fn product(pool: &PgPool, slug: &str, price: &str) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO digital_products (title, slug, product_type, price)
         VALUES ($1, $1, 'premade_file', $2) RETURNING id",
    )
    .bind(slug)
    .bind(dec(price))
    .fetch_one(pool)
    .await
    .expect("insert product");

    sqlx::query(
        "INSERT INTO product_files
            (product_id, version, storage_path, file_size_bytes, mime_type, is_current)
         VALUES ($1, '1.0.0', $2, 1024, 'application/zip', true)",
    )
    .bind(id)
    .bind(format!("protected-products/{id}/1.0.0/{slug}.zip"))
    .execute(pool)
    .await
    .expect("insert product file");

    id
}

fn cart(product_id: Uuid, key: Option<&str>) -> CheckoutRequest {
    CheckoutRequest {
        items: vec![CheckoutItem {
            product_id,
            quantity: 1,
        }],
        payment_provider: "wallet".into(),
        idempotency_key: key.map(str::to_string),
    }
}

async fn balance(pool: &PgPool, user_id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT wallet_balance FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("balance")
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_without_funds_writes_nothing(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "broke@example.com").await;
    let product_id = product(&pool, "template-pack", "19.00").await;

    let err = commerce::checkout(&app, user_id, cart(product_id, None))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InsufficientFunds));

    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
        .fetch_one(&pool)
        .await
        .expect("count orders");
    assert_eq!(orders, 0, "a rejected checkout must not leave an order");
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_debits_grants_and_writes_one_ledger_row(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "buyer@example.com").await;
    let product_id = product(&pool, "template-pack", "19.00").await;
    fund(&pool, user_id, "100.00").await;

    let order = commerce::checkout(&app, user_id, cart(product_id, None))
        .await
        .expect("checkout");

    assert_eq!(order.status, "paid");
    assert_eq!(order.total_amount, dec("19.00"));
    assert_eq!(order.entitlements_created, 1);
    assert_eq!(balance(&pool, user_id).await, dec("81.00"));

    let (txn_type, amount, balance_after): (String, Decimal, Decimal) = sqlx::query_as(
        "SELECT txn_type::text, amount, balance_after
           FROM wallet_transactions WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("ledger row");

    assert_eq!(txn_type, "debit");
    assert_eq!(amount, dec("19.00"));
    assert_eq!(balance_after, dec("81.00"));

    let derived: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN txn_type IN ('credit','refund','adjustment')
                                  THEN amount ELSE -amount END), 0)
           FROM wallet_transactions WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("derived balance");
    assert_eq!(derived, balance(&pool, user_id).await);
}

#[sqlx::test(migrations = "./migrations")]
async fn replaying_an_idempotency_key_charges_once(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "buyer@example.com").await;
    let product_id = product(&pool, "template-pack", "19.00").await;
    fund(&pool, user_id, "100.00").await;

    let first = commerce::checkout(&app, user_id, cart(product_id, Some("key-1")))
        .await
        .expect("first checkout");
    let second = commerce::checkout(&app, user_id, cart(product_id, Some("key-1")))
        .await
        .expect("replay");

    assert_eq!(first.order_id, second.order_id);
    assert_eq!(second.entitlements_created, 1);
    assert_eq!(balance(&pool, user_id).await, dec("81.00"));

    let debits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_transactions
          WHERE user_id = $1 AND txn_type = 'debit'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("count debits");
    assert_eq!(debits, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn download_counter_is_atomic_and_respects_the_limit(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "buyer@example.com").await;
    let product_id = product(&pool, "template-pack", "19.00").await;
    fund(&pool, user_id, "100.00").await;

    commerce::checkout(&app, user_id, cart(product_id, None))
        .await
        .expect("checkout");

    let entitlement_id: Uuid =
        sqlx::query_scalar("SELECT id FROM user_entitlements WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("entitlement");

    sqlx::query("UPDATE user_entitlements SET max_downloads = 1 WHERE id = $1")
        .bind(entitlement_id)
        .execute(&pool)
        .await
        .expect("set limit");

    let first = service::download(&app, user_id, entitlement_id, "127.0.0.1", None)
        .await
        .expect("first download");
    assert!(first.download_url.contains("X-Amz-Signature"));
    assert!(!first.download_url.contains("storage_path"));

    let second = service::download(&app, user_id, entitlement_id, "127.0.0.1", None).await;
    assert!(matches!(second, Err(AppError::DownloadsExhausted)));

    let count: i32 =
        sqlx::query_scalar("SELECT download_count FROM user_entitlements WHERE id = $1")
            .bind(entitlement_id)
            .fetch_one(&pool)
            .await
            .expect("download count");
    assert_eq!(
        count, 1,
        "the rejected attempt must not increment the counter"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn refund_refuses_downloaded_orders_and_revokes_entitlements(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "buyer@example.com").await;
    let first_product = product(&pool, "template-pack", "19.00").await;
    let second_product = product(&pool, "launch-checklist", "9.00").await;
    fund(&pool, user_id, "100.00").await;

    let used = commerce::checkout(&app, user_id, cart(first_product, None))
        .await
        .expect("checkout");
    let entitlement_id: Uuid =
        sqlx::query_scalar("SELECT id FROM user_entitlements WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("entitlement");
    service::download(&app, user_id, entitlement_id, "127.0.0.1", None)
        .await
        .expect("download");

    let err = commerce::refund_order(&app, used.order_id)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidState(_)));

    let untouched = commerce::checkout(&app, user_id, cart(second_product, None))
        .await
        .expect("checkout");
    let refund = commerce::refund_order(&app, untouched.order_id)
        .await
        .expect("refund");
    assert_eq!(refund.status, "refunded");
    assert_eq!(refund.refunded_amount, dec("9.00"));
    assert_eq!(balance(&pool, user_id).await, dec("81.00"));

    let live: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_entitlements
          WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("count live entitlements");
    assert_eq!(live, 1, "the refunded order's entitlement must be revoked");

    let again = commerce::refund_order(&app, untouched.order_id)
        .await
        .unwrap_err();
    assert!(matches!(again, AppError::InvalidState(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_checkouts_cannot_overspend(pool: PgPool) {
    let app = app(&pool).await;
    let user_id = user(&pool, "racer@example.com").await;
    let product_id = product(&pool, "template-pack", "19.00").await;
    // Exactly enough for one order, not two.
    fund(&pool, user_id, "19.00").await;

    let (first, second) = tokio::join!(
        commerce::checkout(&app, user_id, cart(product_id, None)),
        commerce::checkout(&app, user_id, cart(product_id, None)),
    );

    let succeeded = [first.is_ok(), second.is_ok()]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    assert_eq!(succeeded, 1, "only one checkout may win");

    let loser = if first.is_ok() { second } else { first };
    assert!(matches!(loser, Err(AppError::InsufficientFunds)));

    assert_eq!(balance(&pool, user_id).await, dec("0.00"));

    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
        .fetch_one(&pool)
        .await
        .expect("count orders");
    assert_eq!(orders, 1, "the losing transaction must roll its order back");
}
