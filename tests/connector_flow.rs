//! Connector and order-ingest tests: WooCommerce webhook, signature, duplicate
//! delivery, CSV fallback and tenant isolation.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use erp_commerce::app::build_router;
use erp_commerce::common::auth::issue_token;
use erp_commerce::common::storage::Storage;
use erp_commerce::config::Config;
use erp_commerce::modules::auth::dtos::RegisterRequest;
use erp_commerce::modules::auth::service as auth_service;
use erp_commerce::state::AppState;

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

async fn fake_woocommerce() -> (String, tokio::task::JoinHandle<()>) {
    async fn orders(
        axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    ) -> Json<Value> {
        let page = params.get("page").map(String::as_str).unwrap_or("1");
        if page == "1" {
            Json(json!([{
                "id": 20231,
                "number": "ORD-20231",
                "status": "processing",
                "payment_method": "cod",
                "total": "1800.00",
                "currency": "BDT",
                "date_created_gmt": "2026-11-21T10:00:00",
                "billing": {
                    "first_name": "Sadia",
                    "last_name": "Rahman",
                    "phone": "01511000444",
                    "email": "sadia@example.com"
                }
            }]))
        } else {
            Json(json!([]))
        }
    }

    let app = Router::new().route("/wp-json/wc/v3/orders", get(orders));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let addr = listener.local_addr().expect("addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("fake server");
    });
    (format!("http://{addr}"), handle)
}

async fn register(app: &AppState, email: &str, business: &str) -> (Uuid, Uuid) {
    let profile = auth_service::register(
        app,
        RegisterRequest {
            email: email.into(),
            password: "correct-horse-battery-staple".into(),
            full_name: "Test Owner".into(),
            business_name: Some(business.into()),
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
    headers: &[(&str, &str)],
    body: Body,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant) = tenant {
        builder = builder.header("x-tenant-id", tenant.to_string());
    }
    for (name, value) in headers {
        builder = builder.header(*name, HeaderValue::from_str(value).expect("header"));
    }

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

fn sign(secret: &str, body: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(body.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

async fn create_connector(
    app: &AppState,
    token: &str,
    tenant: Uuid,
    kind: &str,
) -> (Uuid, Option<String>, Option<String>) {
    let (status, body) = call(
        app,
        "POST",
        "/api/v1/connectors",
        Some(token),
        Some(tenant),
        &[("content-type", "application/json")],
        Body::from(json!({ "kind": kind, "name": format!("{kind} main") }).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["data"]["id"]
        .as_str()
        .expect("connector id")
        .parse()
        .expect("uuid");
    let path = body["data"]["webhook_path"].as_str().map(str::to_string);
    let secret = body["data"]["webhook_secret"].as_str().map(str::to_string);
    (id, path, secret)
}

async fn create_rest_connector(app: &AppState, token: &str, tenant: Uuid, base_url: &str) -> Uuid {
    let (status, body) = call(
        app,
        "POST",
        "/api/v1/connectors",
        Some(token),
        Some(tenant),
        &[("content-type", "application/json")],
        Body::from(
            json!({
                "kind": "woocommerce",
                "name": "REST store",
                "base_url": base_url,
                "consumer_key": "ck_test",
                "consumer_secret": "cs_test"
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["rest_configured"], true);
    body["data"]["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("uuid")
}

fn woocommerce_order() -> String {
    json!({
        "id": 10231,
        "number": "ORD-10231",
        "status": "processing",
        "payment_method": "cod",
        "total": "1060.00",
        "currency": "BDT",
        "date_created_gmt": "2026-11-20T10:00:00",
        "billing": {
            "first_name": "Jahid",
            "last_name": "Hasan",
            "phone": "01712345678",
            "email": "jahid@example.com"
        },
        "line_items": [{ "name": "Polo Shirt", "quantity": 1 }]
    })
    .to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn woocommerce_webhook_is_signed_idempotent_and_visible(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, tenant) = register(&app, "owner@example.com", "Owner Co").await;
    let owner_token = token(&app, owner);
    let (connector_id, path, secret) =
        create_connector(&app, &owner_token, tenant, "woocommerce").await;
    let path = path.expect("webhook path");
    let secret = secret.expect("webhook secret");
    let body = woocommerce_order();
    let signature = sign(&secret, &body);

    let (status, response) = call(
        &app,
        "POST",
        &path,
        None,
        None,
        &[
            ("content-type", "application/json"),
            ("x-wc-webhook-signature", &signature),
            ("x-wc-webhook-delivery-id", "delivery-1"),
            ("x-wc-webhook-topic", "order.created"),
        ],
        Body::from(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "processed");
    assert!(response["data"]["order_id"].is_string());

    let (status, orders) = call(
        &app,
        "GET",
        "/api/v1/orders",
        Some(&owner_token),
        Some(tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{orders}");
    assert_eq!(orders["data"].as_array().expect("orders").len(), 1);
    assert_eq!(orders["data"][0]["total_amount"], "1060.00");
    assert_eq!(orders["data"][0]["cod_amount"], "1060.00");
    assert_eq!(orders["data"][0]["customer_phone"], "01712345678");

    let (status, replay) = call(
        &app,
        "POST",
        &path,
        None,
        None,
        &[
            ("content-type", "application/json"),
            ("x-wc-webhook-signature", &signature),
            ("x-wc-webhook-delivery-id", "delivery-1"),
            ("x-wc-webhook-topic", "order.created"),
        ],
        Body::from(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["data"]["status"], "duplicate");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_orders WHERE connector_id = $1")
            .bind(connector_id)
            .fetch_one(&app.db)
            .await
            .expect("order count");
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn bad_signature_is_rejected_and_recorded(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, tenant) = register(&app, "owner2@example.com", "Owner Co").await;
    let owner_token = token(&app, owner);
    let (_, path, _) = create_connector(&app, &owner_token, tenant, "woocommerce").await;
    let path = path.expect("webhook path");

    let (status, response) = call(
        &app,
        "POST",
        &path,
        None,
        None,
        &[
            ("content-type", "application/json"),
            ("x-wc-webhook-signature", "not-a-signature"),
            ("x-wc-webhook-delivery-id", "bad-1"),
            ("x-wc-webhook-topic", "order.created"),
        ],
        Body::from(woocommerce_order()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{response}");

    let rejected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM connector_webhook_events WHERE status = 'rejected'",
    )
    .fetch_one(&app.db)
    .await
    .expect("rejected events");
    assert_eq!(rejected, 1);

    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM external_orders")
        .fetch_one(&app.db)
        .await
        .expect("orders");
    assert_eq!(orders, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn csv_import_and_tenant_isolation(pool: PgPool) {
    let app = app(&pool).await;
    let (alice, alice_tenant) = register(&app, "alice@example.com", "Alice Co").await;
    let (bob, bob_tenant) = register(&app, "bob@example.com", "Bob Co").await;
    let alice_token = token(&app, alice);
    let bob_token = token(&app, bob);
    let (connector_id, _, _) = create_connector(&app, &alice_token, alice_tenant, "csv").await;

    let csv = "order_id,order_number,status,payment_method,total,currency,placed_at,customer_name,phone,email,address_1,address_2,city,state\nCSV-1,ORD-CSV-1,processing,cod,1500.00,BDT,2026-11-21T09:00:00,Nusrat Jahan,01812345678,nusrat@example.com,House 1,Road 2,Dhaka,Dhaka\n";
    let (status, response) = call(
        &app,
        "POST",
        &format!("/api/v1/connectors/{connector_id}/import"),
        Some(&alice_token),
        Some(alice_tenant),
        &[("content-type", "text/csv")],
        Body::from(csv),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["processed"], 1);

    let (status, orders) = call(
        &app,
        "GET",
        "/api/v1/orders",
        Some(&alice_token),
        Some(alice_tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(orders["data"].as_array().expect("orders").len(), 1);

    let (status, orders) = call(
        &app,
        "GET",
        "/api/v1/orders",
        Some(&bob_token),
        Some(bob_tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(orders["data"].as_array().expect("orders").len(), 0);

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/connectors/{connector_id}/import"),
        Some(&bob_token),
        Some(bob_tenant),
        &[("content-type", "text/csv")],
        Body::from(csv),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn woocommerce_rest_sync_and_health(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, tenant) = register(&app, "owner4@example.com", "Owner Co").await;
    let owner_token = token(&app, owner);
    let (base_url, server) = fake_woocommerce().await;
    let connector_id = create_rest_connector(&app, &owner_token, tenant, &base_url).await;

    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/v1/connectors/{connector_id}/test"),
        Some(&owner_token),
        Some(tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["ok"], true);

    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/v1/connectors/{connector_id}/health"),
        Some(&owner_token),
        Some(tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["rest_configured"], true);

    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/v1/connectors/{connector_id}/sync?pages=2"),
        Some(&owner_token),
        Some(tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["processed"], 1);

    let (status, orders) = call(
        &app,
        "GET",
        "/api/v1/orders",
        Some(&owner_token),
        Some(tenant),
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(orders["data"].as_array().expect("orders").len(), 1);
    assert_eq!(orders["data"][0]["external_id"], "20231");

    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn csv_import_handles_five_thousand_rows(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, tenant) = register(&app, "owner5@example.com", "Owner Co").await;
    let owner_token = token(&app, owner);
    let (connector_id, _, _) = create_connector(&app, &owner_token, tenant, "csv").await;

    let mut csv = String::from(
        "order_id,order_number,status,payment_method,total,currency,placed_at,customer_name,phone,email,address_1,address_2,city,state\n",
    );
    for i in 0..5_000 {
        writeln!(
            csv,
            "CSV-{i},ORD-{i},processing,cod,100.00,BDT,2026-11-21T09:00:00,Customer {i},01712345678,c{i}@example.com,House,Road,Dhaka,Dhaka"
        )
        .expect("write csv");
    }

    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/v1/connectors/{connector_id}/import"),
        Some(&owner_token),
        Some(tenant),
        &[("content-type", "text/csv")],
        Body::from(csv),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["processed"], 5_000);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_orders WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(&app.db)
            .await
            .expect("orders");
    assert_eq!(count, 5_000);
}

#[sqlx::test(migrations = "./migrations")]
async fn staff_cannot_create_connector(pool: PgPool) {
    let app = app(&pool).await;
    let (owner, tenant) = register(&app, "owner3@example.com", "Owner Co").await;
    let (staff, _) = register(&app, "staff@example.com", "Staff Co").await;
    let owner_token = token(&app, owner);
    let staff_token = token(&app, staff);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/team/invite",
        Some(&owner_token),
        Some(tenant),
        &[("content-type", "application/json")],
        Body::from(json!({ "email": "staff@example.com", "role": "staff" }).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let membership_id: Uuid =
        sqlx::query_scalar("SELECT id FROM memberships WHERE tenant_id = $1 AND user_id = $2")
            .bind(tenant)
            .bind(staff)
            .fetch_one(&app.db)
            .await
            .expect("membership");

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/team/invitations/{membership_id}/accept"),
        Some(&staff_token),
        None,
        &[],
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/connectors",
        Some(&staff_token),
        Some(tenant),
        &[("content-type", "application/json")],
        Body::from(json!({ "kind": "csv", "name": "staff connector" }).to_string()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
