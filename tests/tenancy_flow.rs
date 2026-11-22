//! Phase 1 tenancy tests: signup provisioning, header isolation, invite/accept,
//! and role guards. Each test gets a fresh database with all migrations.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use erp_commerce::app::build_router;
use erp_commerce::common::auth::issue_token;
use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::error::AppError;
use erp_commerce::modules::auth::dtos::RegisterRequest;
use erp_commerce::modules::auth::service as auth_service;
use erp_commerce::state::AppState;

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
        connector_encryption_key: "test-connector-encryption-key-32".into(),
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

async fn register(
    app: &AppState,
    email: &str,
    full_name: &str,
    business_name: &str,
) -> (Uuid, Uuid) {
    let profile = auth_service::register(
        app,
        RegisterRequest {
            email: email.into(),
            password: "correct-horse-battery-staple".into(),
            full_name: full_name.into(),
            business_name: Some(business_name.into()),
        },
    )
    .await
    .expect("register");

    let tenant_id: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM memberships WHERE user_id = $1 AND status = 'active'",
    )
    .bind(profile.id)
    .fetch_one(&app.db)
    .await
    .expect("tenant");

    (profile.id, tenant_id)
}

fn token(app: &AppState, user_id: Uuid) -> String {
    issue_token(user_id, "client", &app.cfg.jwt_secret, 3600).expect("token")
}

async fn call(
    app: &AppState,
    method: &str,
    uri: &str,
    token: Option<&str>,
    tenant: Option<Uuid>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        builder = builder.header("x-tenant-id", tenant.to_string());
    }

    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };

    let response = build_router(app.clone())
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[sqlx::test(migrations = "./migrations")]
async fn signup_creates_tenant_owner_and_audit(pool: PgPool) {
    let app = app(&pool).await;
    let (user_id, tenant_id) = register(&app, "owner@example.com", "Owner One", "Acme").await;

    let (name, role, status): (String, String, String) = sqlx::query_as(
        r#"
        SELECT t.name, m.role::text, m.status::text
          FROM memberships m
          JOIN tenants t ON t.id = m.tenant_id
         WHERE m.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&app.db)
    .await
    .expect("membership");

    assert_eq!(name, "Acme");
    assert_eq!(role, "owner");
    assert_eq!(status, "active");

    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&app.db)
            .await
            .expect("audit count");
    assert_eq!(audit_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn tenant_header_is_required_and_cross_tenant_is_404(pool: PgPool) {
    let app = app(&pool).await;
    let (alice, alice_tenant) = register(&app, "alice@example.com", "Alice", "Alice Co").await;
    let (_, bob_tenant) = register(&app, "bob@example.com", "Bob", "Bob Co").await;
    let alice_token = token(&app, alice);

    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/tenants/current",
        Some(&alice_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/tenants/current",
        Some(&alice_token),
        Some(bob_tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = call(
        &app,
        "GET",
        "/api/v1/tenants/current",
        Some(&alice_token),
        Some(alice_tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["name"], "Alice Co");
    assert_eq!(body["data"]["role"], "owner");
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_invites_and_invitee_accepts(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, owner_tenant) = register(&app, "owner2@example.com", "Owner", "Owner Co").await;
    let (staff, _) = register(&app, "staff@example.com", "Staff", "Staff Co").await;

    let owner_token = token(&app, owner);
    let staff_token = token(&app, staff);

    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/team/invite",
        Some(&owner_token),
        Some(owner_tenant),
        Some(json!({ "email": "staff@example.com", "role": "staff" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["status"], "invited");

    let membership_id: Uuid =
        sqlx::query_scalar("SELECT id FROM memberships WHERE tenant_id = $1 AND user_id = $2")
            .bind(owner_tenant)
            .bind(staff)
            .fetch_one(&app.db)
            .await
            .expect("invite row");

    let (status, body) = call(
        &app,
        "GET",
        "/api/v1/team/invitations",
        Some(&staff_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("invitations").len(), 1);

    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/v1/team/invitations/{membership_id}/accept"),
        Some(&staff_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "Owner Co");

    let (status, body) = call(
        &app,
        "GET",
        "/api/v1/team",
        Some(&staff_token),
        Some(owner_tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("team").len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_signup_leaves_no_orphan_tenant(pool: PgPool) {
    let app = app(&pool).await;
    register(&app, "dup@example.com", "Dup", "First Co").await;

    let tenants_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenants")
        .fetch_one(&app.db)
        .await
        .expect("tenants before");

    let err = auth_service::register(
        &app,
        RegisterRequest {
            email: "dup@example.com".into(),
            password: "correct-horse-battery-staple".into(),
            full_name: "Dup Again".into(),
            business_name: Some("Second Co".into()),
        },
    )
    .await
    .expect_err("duplicate email");
    assert!(matches!(err, AppError::EmailTaken));

    let tenants_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenants")
        .fetch_one(&app.db)
        .await
        .expect("tenants after");
    assert_eq!(tenants_before, tenants_after);

    let memberships: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships")
        .fetch_one(&app.db)
        .await
        .expect("memberships");
    assert_eq!(memberships, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn staff_cannot_invite_or_change_roles(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, owner_tenant) = register(&app, "owner3@example.com", "Owner", "Owner Co").await;
    let (staff, _) = register(&app, "staff2@example.com", "Staff", "Staff Co").await;
    let (other, other_tenant) = register(&app, "other@example.com", "Other", "Other Co").await;

    let owner_token = token(&app, owner);
    let staff_token = token(&app, staff);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/team/invite",
        Some(&owner_token),
        Some(owner_tenant),
        Some(json!({ "email": "staff2@example.com", "role": "staff" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let membership_id: Uuid =
        sqlx::query_scalar("SELECT id FROM memberships WHERE tenant_id = $1 AND user_id = $2")
            .bind(owner_tenant)
            .bind(staff)
            .fetch_one(&app.db)
            .await
            .expect("invite row");

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/team/invitations/{membership_id}/accept"),
        Some(&staff_token),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/team/invite",
        Some(&staff_token),
        Some(owner_tenant),
        Some(json!({ "email": "other@example.com", "role": "staff" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(
        &app,
        "PATCH",
        &format!("/api/v1/team/{owner}"),
        Some(&staff_token),
        Some(owner_tenant),
        Some(json!({ "role": "staff" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The other user's tenant is not visible through the owner's header.
    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/team",
        Some(&owner_token),
        Some(other_tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = other;
}
