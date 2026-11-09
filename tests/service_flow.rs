//! Integration tests for the P2 entry points: uploads, order creation, the
//! role-scoped board and the order detail contract.

use std::sync::Arc;

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use erp_commerce::common::extractors::{AuthUser, Role};
use erp_commerce::common::pagination::Page;
use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::error::AppError;
use erp_commerce::modules::services::dtos::{
    AssignRequest, CancelRequest, CreateServiceOrderRequest, RequestRevisionRequest,
    SubmitBriefRequest, SubmitDeliverableRequest,
};
use erp_commerce::modules::services::models::ServiceStatus;
use erp_commerce::modules::services::{service, transitions};
use erp_commerce::modules::uploads::dtos::PresignRequest;
use erp_commerce::modules::uploads::service as uploads;
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

async fn user(pool: &PgPool, email: &str, role: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, full_name, role)
         VALUES ($1, 'x', 'Test User', $2::user_role) RETURNING id",
    )
    .bind(email)
    .bind(role)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

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
    .expect("credit");
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
    .expect("ledger");
    tx.commit().await.expect("commit");
}

async fn agency_service(pool: &PgPool, slug: &str, price: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO agency_services (title, slug, base_price, brief_template)
         VALUES ($1, $1, $2, '{\"required\": [\"brand_name\"]}'::jsonb) RETURNING id",
    )
    .bind(slug)
    .bind(dec(price))
    .fetch_one(pool)
    .await
    .expect("insert service")
}

async fn balance(pool: &PgPool, user_id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT wallet_balance FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("balance")
}

fn actor(id: Uuid, role: Role) -> AuthUser {
    AuthUser { id, role }
}

#[sqlx::test(migrations = "./migrations")]
async fn create_order_debits_the_wallet_and_writes_a_ledger_row(pool: PgPool) {
    let app = app(&pool).await;
    let client = user(&pool, "client@example.com", "client").await;
    fund(&pool, client, "100.00").await;
    let service_id = agency_service(&pool, "logo-design", "40.00").await;

    let order = service::create_order(&app, client, CreateServiceOrderRequest { service_id })
        .await
        .expect("create order");

    assert_eq!(order.status, ServiceStatus::PendingBrief);
    assert_eq!(order.price_charged, dec("40.00"));
    assert!(order.brief_required);
    assert_eq!(balance(&pool, client).await, dec("60.00"));

    let (txn_type, reference_type, amount): (String, String, Decimal) = sqlx::query_as(
        "SELECT txn_type::text, reference_type, amount
           FROM wallet_transactions WHERE user_id = $1 AND txn_type = 'debit'",
    )
    .bind(client)
    .fetch_one(&pool)
    .await
    .expect("ledger row");

    assert_eq!(txn_type, "debit");
    assert_eq!(reference_type, "service_order");
    assert_eq!(amount, dec("40.00"));
}

#[sqlx::test(migrations = "./migrations")]
async fn create_order_without_funds_writes_nothing(pool: PgPool) {
    let app = app(&pool).await;
    let client = user(&pool, "client@example.com", "client").await;
    let service_id = agency_service(&pool, "logo-design", "250.00").await;

    let err = service::create_order(&app, client, CreateServiceOrderRequest { service_id })
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InsufficientFunds));

    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_orders")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(orders, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn inactive_service_is_unprocessable(pool: PgPool) {
    let app = app(&pool).await;
    let client = user(&pool, "client@example.com", "client").await;
    fund(&pool, client, "100.00").await;
    let service_id = agency_service(&pool, "retired", "40.00").await;
    sqlx::query("UPDATE agency_services SET is_active = false WHERE id = $1")
        .bind(service_id)
        .execute(&pool)
        .await
        .expect("deactivate");

    let err = service::create_order(&app, client, CreateServiceOrderRequest { service_id })
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Unprocessable(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn the_board_is_role_scoped_and_filterable(pool: PgPool) {
    let app = app(&pool).await;
    let alice = user(&pool, "alice@example.com", "client").await;
    let bob = user(&pool, "bob@example.com", "client").await;
    let staff = user(&pool, "staff@example.com", "staff").await;
    let admin = user(&pool, "admin@example.com", "admin").await;
    fund(&pool, alice, "100.00").await;
    fund(&pool, bob, "100.00").await;
    let service_id = agency_service(&pool, "logo-design", "40.00").await;

    let first = service::create_order(&app, alice, CreateServiceOrderRequest { service_id })
        .await
        .expect("alice order");
    service::create_order(&app, bob, CreateServiceOrderRequest { service_id })
        .await
        .expect("bob order");

    sqlx::query(
        "UPDATE service_orders SET assigned_staff_id = $1, status = 'in_progress'
          WHERE id = $2",
    )
    .bind(staff)
    .bind(first.id)
    .execute(&pool)
    .await
    .expect("assign");

    let page = Page::new(None, None);

    let (alice_orders, alice_total) =
        service::list_orders(&app, &actor(alice, Role::Client), &page, None, false)
            .await
            .expect("alice list");
    assert_eq!(alice_total, 1);
    assert_eq!(alice_orders[0].id, first.id);

    let (staff_orders, staff_total) =
        service::list_orders(&app, &actor(staff, Role::Staff), &page, None, false)
            .await
            .expect("staff list");
    assert_eq!(staff_total, 1);
    assert_eq!(staff_orders[0].id, first.id);

    let (admin_orders, admin_total) =
        service::list_orders(&app, &actor(admin, Role::Admin), &page, None, false)
            .await
            .expect("admin list");
    assert_eq!(admin_total, 2);
    assert_eq!(admin_orders.len(), 2);

    let (in_progress, _) = service::list_orders(
        &app,
        &actor(admin, Role::Admin),
        &page,
        Some(ServiceStatus::InProgress),
        false,
    )
    .await
    .expect("filtered list");
    assert_eq!(in_progress.len(), 1);

    let (mine, _) = service::list_orders(&app, &actor(staff, Role::Staff), &page, None, true)
        .await
        .expect("assignee=me");
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].id, first.id);

    // The admin is not the assignee, so `assignee=me` is empty for them.
    let (admin_mine, _) = service::list_orders(&app, &actor(admin, Role::Admin), &page, None, true)
        .await
        .expect("admin assignee=me");
    assert!(admin_mine.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn order_detail_exposes_actions_only_to_people_who_can_see_it(pool: PgPool) {
    let app = app(&pool).await;
    let alice = user(&pool, "alice@example.com", "client").await;
    let bob = user(&pool, "bob@example.com", "client").await;
    fund(&pool, alice, "100.00").await;
    let service_id = agency_service(&pool, "logo-design", "40.00").await;

    let order = service::create_order(&app, alice, CreateServiceOrderRequest { service_id })
        .await
        .expect("order");

    let detail = service::get_order(&app, &actor(alice, Role::Client), order.id)
        .await
        .expect("owner view");
    assert_eq!(detail.status, ServiceStatus::PendingBrief);
    assert_eq!(detail.allowed_actions, vec!["submit_brief", "cancel"]);
    assert!(detail.brief.is_none());
    assert!(detail.deliverables.is_empty());

    let stranger = service::get_order(&app, &actor(bob, Role::Client), order.id).await;
    assert!(matches!(stranger, Err(AppError::NotFound)));
}

#[sqlx::test(migrations = "./migrations")]
async fn presign_enforces_the_allow_list(pool: PgPool) {
    let app = app(&pool).await;
    let client = user(&pool, "client@example.com", "client").await;

    let ok = uploads::presign(
        &app,
        client,
        PresignRequest {
            purpose: "brief_asset".into(),
            filename: "logo.png".into(),
            content_type: "image/png".into(),
            size_bytes: 1024,
        },
    )
    .await
    .expect("presign png");

    assert!(ok
        .key
        .starts_with(&format!("user-uploads/{client}/briefs/")));
    assert!(ok.key.ends_with(".png"));
    assert!(ok.upload_url.contains("X-Amz-Signature"));
    assert_eq!(ok.max_size_bytes, 10 * 1024 * 1024);

    let gif = uploads::presign(
        &app,
        client,
        PresignRequest {
            purpose: "brief_asset".into(),
            filename: "logo.gif".into(),
            content_type: "image/gif".into(),
            size_bytes: 1024,
        },
    )
    .await;
    assert!(matches!(gif, Err(AppError::UnsupportedMediaType)));

    let huge = uploads::presign(
        &app,
        client,
        PresignRequest {
            purpose: "brief_asset".into(),
            filename: "logo.png".into(),
            content_type: "image/png".into(),
            size_bytes: 11 * 1024 * 1024,
        },
    )
    .await;
    assert!(matches!(huge, Err(AppError::PayloadTooLarge)));
}

#[sqlx::test(migrations = "./migrations")]
async fn the_whole_pipeline_runs_from_brief_to_download(pool: PgPool) {
    let app = app(&pool).await;
    let alice = user(&pool, "alice@example.com", "client").await;
    let staff = user(&pool, "staff@example.com", "staff").await;
    fund(&pool, alice, "100.00").await;
    let service_id = agency_service(&pool, "logo-design", "40.00").await;

    let order = service::create_order(&app, alice, CreateServiceOrderRequest { service_id })
        .await
        .expect("order");
    let client = actor(alice, Role::Client);

    let missing = transitions::submit_brief(
        &app,
        &client,
        order.id,
        SubmitBriefRequest {
            payload: json!({}),
            uploaded_asset_keys: vec![],
        },
    )
    .await
    .unwrap_err();
    match missing {
        AppError::BriefIncomplete(keys) => assert_eq!(keys, vec!["brand_name".to_string()]),
        other => panic!("expected BriefIncomplete, got {other:?}"),
    }

    let brief = transitions::submit_brief(
        &app,
        &client,
        order.id,
        SubmitBriefRequest {
            payload: json!({ "brand_name": "Acme" }),
            uploaded_asset_keys: vec![format!("user-uploads/{alice}/briefs/logo.png")],
        },
    )
    .await
    .expect("brief");
    assert_eq!(brief.status, ServiceStatus::InQueue);

    let assigned = transitions::assign(
        &app,
        &actor(alice, Role::Admin),
        order.id,
        AssignRequest {
            assigned_staff_id: staff,
            due_at: Utc::now() + Duration::days(2),
        },
    )
    .await
    .expect("assign");
    assert_eq!(assigned.status, ServiceStatus::InProgress);

    let staff_actor = actor(staff, Role::Staff);
    let first = transitions::submit_deliverable(
        &app,
        &staff_actor,
        order.id,
        SubmitDeliverableRequest {
            final_asset_key: format!("service-deliverables/{}/final/v1.zip", order.id),
            preview_asset_key: format!("service-deliverables/{}/raw/v1.png", order.id),
            notes: Some("first pass".into()),
        },
    )
    .await
    .expect("deliverable 1");
    assert_eq!(first.version, 1);
    assert_eq!(first.preview_status, "processing");

    // The worker would set this once the watermark exists.
    sqlx::query("UPDATE service_orders SET status = 'under_review' WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .expect("worker transition");

    let revision = transitions::request_revision(
        &app,
        &client,
        order.id,
        RequestRevisionRequest {
            deliverable_id: first.deliverable_id,
            feedback: "logo bigger".into(),
        },
    )
    .await
    .expect("revision");
    assert_eq!(revision.revisions_used, 1);

    let second = transitions::submit_deliverable(
        &app,
        &staff_actor,
        order.id,
        SubmitDeliverableRequest {
            final_asset_key: format!("service-deliverables/{}/final/v2.zip", order.id),
            preview_asset_key: format!("service-deliverables/{}/raw/v2.png", order.id),
            notes: None,
        },
    )
    .await
    .expect("deliverable 2");
    assert_eq!(second.version, 2);

    sqlx::query("UPDATE service_orders SET status = 'under_review' WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .expect("worker transition");

    let approved = transitions::approve(&app, &client, order.id)
        .await
        .expect("approve");
    assert_eq!(approved.status, ServiceStatus::Completed);
    assert_eq!(approved.approved_deliverable_id, second.deliverable_id);
    assert!(approved.download_available);

    let download = service::deliverable_download(&app, &client, order.id, second.deliverable_id)
        .await
        .expect("download");
    assert!(download.download_url.contains("X-Amz-Signature"));
    assert!(download.filename.ends_with(".zip"));

    // A completed order is terminal.
    let cancelled =
        transitions::cancel(&app, &client, order.id, CancelRequest { reason: None }).await;
    assert!(matches!(cancelled, Err(AppError::InvalidState(_))));
}

#[sqlx::test(migrations = "./migrations")]
async fn cancel_refunds_once_and_is_terminal(pool: PgPool) {
    let app = app(&pool).await;
    let alice = user(&pool, "alice@example.com", "client").await;
    fund(&pool, alice, "100.00").await;
    let service_id = agency_service(&pool, "logo-design", "40.00").await;

    let order = service::create_order(&app, alice, CreateServiceOrderRequest { service_id })
        .await
        .expect("order");
    assert_eq!(balance(&pool, alice).await, dec("60.00"));

    let cancelled = transitions::cancel(
        &app,
        &actor(alice, Role::Client),
        order.id,
        CancelRequest {
            reason: Some("changed my mind".into()),
        },
    )
    .await
    .expect("cancel");
    assert_eq!(cancelled.status, ServiceStatus::Cancelled);
    assert_eq!(cancelled.refunded, dec("40.00"));
    assert_eq!(balance(&pool, alice).await, dec("100.00"));

    let ledger: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM wallet_transactions
          WHERE user_id = $1 AND txn_type = 'refund' AND reference_type = 'service_order'",
    )
    .bind(alice)
    .fetch_one(&pool)
    .await
    .expect("ledger");
    assert_eq!(ledger, 1);

    let again = transitions::cancel(
        &app,
        &actor(alice, Role::Client),
        order.id,
        CancelRequest { reason: None },
    )
    .await;
    assert!(matches!(again, Err(AppError::InvalidState(_))));
}
