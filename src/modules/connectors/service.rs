use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{DateTime, NaiveDateTime, Utc};
use csv::ReaderBuilder;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::dtos::{
    ConnectorSummary, CreateConnectorRequest, CreatedConnector, CsvOrder, ImportResult,
    WebhookResult,
};
use super::models::ConnectorRow;
use crate::common::tenant::TenantContext;
use crate::error::AppError;
use crate::modules::tenancy;
use crate::state::AppState;

struct EventRecord<'a> {
    tenant_id: Uuid,
    connector_id: Uuid,
    event_id: &'a str,
    topic: Option<&'a str>,
    payload: &'a Value,
    status: &'a str,
    error: Option<&'a str>,
}

struct NormalizedOrder {
    external_id: String,
    order_number: String,
    status: String,
    payment_method: Option<String>,
    total_amount: Decimal,
    cod_amount: Decimal,
    currency: String,
    placed_at: DateTime<Utc>,
    customer_name: Option<String>,
    customer_phone: Option<String>,
    customer_email: Option<String>,
    raw: Value,
}

pub async fn create_connector(
    state: &AppState,
    ctx: &TenantContext,
    payload: CreateConnectorRequest,
) -> Result<CreatedConnector, AppError> {
    ctx.require_manager()?;

    let kind = payload.kind.trim().to_lowercase();
    if !matches!(kind.as_str(), "woocommerce" | "laravel" | "csv") {
        return Err(AppError::Validation(
            "kind must be 'woocommerce', 'laravel' or 'csv'".into(),
        ));
    }

    let name = payload.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::Validation("name must be 1-120 characters".into()));
    }

    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());

    let mut tx = state.db.begin().await?;
    let row: ConnectorRow = sqlx::query_as(
        r#"
        INSERT INTO connectors (tenant_id, kind, name, webhook_secret_encrypted)
        VALUES ($1, $2::connector_kind, $3, pgp_sym_encrypt($4, $5))
        RETURNING id, kind::text, name, status::text, last_sync_at, created_at
        "#,
    )
    .bind(ctx.tenant_id)
    .bind(&kind)
    .bind(name)
    .bind(&secret)
    .bind(&state.cfg.connector_encryption_key)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("connectors_unique_name") => {
            AppError::InvalidState("a connector with that name already exists".into())
        }
        _ => AppError::Db(e),
    })?;

    tenancy::service::insert_audit(
        &mut tx,
        ctx.tenant_id,
        Some(ctx.user_id),
        "connector",
        row.id,
        "connector.created",
        None,
        Some(json!({ "kind": kind, "name": name })),
    )
    .await?;
    tx.commit().await?;

    let is_webhook = kind == "woocommerce";
    Ok(CreatedConnector {
        id: row.id,
        kind: row.kind,
        name: row.name,
        status: row.status,
        webhook_path: is_webhook.then(|| format!("/api/v1/webhooks/woocommerce/{}", row.id)),
        webhook_secret: is_webhook.then_some(secret),
        created_at: row.created_at,
    })
}

pub async fn list_connectors(
    state: &AppState,
    ctx: &TenantContext,
) -> Result<Vec<ConnectorSummary>, AppError> {
    let rows: Vec<ConnectorRow> = sqlx::query_as(
        r#"
        SELECT id, kind::text, name, status::text, last_sync_at, created_at
          FROM connectors
         WHERE tenant_id = $1
         ORDER BY created_at DESC
        "#,
    )
    .bind(ctx.tenant_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ConnectorSummary {
            id: row.id,
            kind: row.kind,
            name: row.name,
            status: row.status,
            last_sync_at: row.last_sync_at,
            created_at: row.created_at,
        })
        .collect())
}

pub async fn ingest_woocommerce(
    state: &AppState,
    connector_id: Uuid,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<WebhookResult, AppError> {
    let (tenant_id, kind, status, secret) = connector_secret(state, connector_id).await?;
    if kind != "woocommerce" {
        return Err(AppError::Unprocessable(
            "this connector does not accept WooCommerce webhooks".into(),
        ));
    }
    if status != "active" {
        return Err(AppError::Forbidden);
    }

    let event_id = webhook_event_id(headers, body);
    let topic = headers
        .get("x-wc-webhook-topic")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    if !woocommerce_signature_valid(&secret, headers, body) {
        let mut tx = state.db.begin().await?;
        let payload = raw_payload(body);
        insert_event(
            &mut tx,
            EventRecord {
                tenant_id,
                connector_id,
                event_id: &event_id,
                topic: topic.as_deref(),
                payload: &payload,
                status: "rejected",
                error: Some("invalid webhook signature"),
            },
        )
        .await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized);
    }

    let payload: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => {
            let mut tx = state.db.begin().await?;
            let raw = raw_payload(body);
            insert_event(
                &mut tx,
                EventRecord {
                    tenant_id,
                    connector_id,
                    event_id: &event_id,
                    topic: topic.as_deref(),
                    payload: &raw,
                    status: "failed",
                    error: Some("invalid JSON body"),
                },
            )
            .await?;
            tx.commit().await?;
            return Err(AppError::Unprocessable("invalid JSON body".into()));
        }
    };

    let mut tx = state.db.begin().await?;
    let Some(webhook_row_id) = insert_event(
        &mut tx,
        EventRecord {
            tenant_id,
            connector_id,
            event_id: &event_id,
            topic: topic.as_deref(),
            payload: &payload,
            status: "verified",
            error: None,
        },
    )
    .await?
    else {
        tx.commit().await?;
        return Ok(WebhookResult {
            status: "duplicate".into(),
            order_id: None,
        });
    };

    if !topic.as_deref().unwrap_or("").starts_with("order.") {
        finish_event(&mut tx, webhook_row_id).await?;
        tx.commit().await?;
        return Ok(WebhookResult {
            status: "ignored".into(),
            order_id: None,
        });
    }

    let order = normalize_woocommerce(&payload)?;
    let order_id = persist_order(
        &mut tx,
        tenant_id,
        connector_id,
        &order,
        topic.as_deref().unwrap_or("order.updated"),
    )
    .await?;
    finish_event(&mut tx, webhook_row_id).await?;
    tx.commit().await?;

    Ok(WebhookResult {
        status: "processed".into(),
        order_id: Some(order_id),
    })
}

pub async fn import_csv(
    state: &AppState,
    ctx: &TenantContext,
    connector_id: Uuid,
    body: &[u8],
) -> Result<ImportResult, AppError> {
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM connectors WHERE id = $1 AND tenant_id = $2")
            .bind(connector_id)
            .bind(ctx.tenant_id)
            .fetch_optional(&state.db)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let mut reader = ReaderBuilder::new().trim(csv::Trim::All).from_reader(body);
    let mut rows = Vec::new();
    for row in reader.deserialize::<CsvOrder>() {
        rows.push(row.map_err(|e| AppError::Validation(format!("invalid CSV row: {e}")))?);
    }
    if rows.is_empty() {
        return Err(AppError::Unprocessable("CSV has no order rows".into()));
    }

    let mut tx = state.db.begin().await?;
    let mut processed = 0usize;
    for row in &rows {
        let order = normalize_csv(row)?;
        persist_order(&mut tx, ctx.tenant_id, connector_id, &order, "csv.import").await?;
        processed += 1;
    }

    tenancy::service::insert_audit(
        &mut tx,
        ctx.tenant_id,
        Some(ctx.user_id),
        "connector",
        connector_id,
        "connector.csv_import",
        None,
        Some(json!({ "processed": processed })),
    )
    .await?;
    tx.commit().await?;

    Ok(ImportResult { processed })
}

async fn connector_secret(
    state: &AppState,
    connector_id: Uuid,
) -> Result<(Uuid, String, String, String), AppError> {
    let row: Option<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT tenant_id, kind::text, status::text,
               pgp_sym_decrypt(webhook_secret_encrypted, $2)::text
          FROM connectors
         WHERE id = $1
        "#,
    )
    .bind(connector_id)
    .bind(&state.cfg.connector_encryption_key)
    .fetch_optional(&state.db)
    .await?;

    row.ok_or(AppError::NotFound)
}

fn woocommerce_signature_valid(secret: &str, headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(raw) = headers
        .get("x-wc-webhook-signature")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Ok(signature) = STANDARD.decode(raw) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

fn webhook_event_id(headers: &HeaderMap, body: &[u8]) -> String {
    headers
        .get("x-wc-webhook-delivery-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| hex::encode(Sha256::digest(body)))
}

fn raw_payload(body: &[u8]) -> Value {
    match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => json!({ "raw": String::from_utf8_lossy(body) }),
    }
}

async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    event: EventRecord<'_>,
) -> Result<Option<Uuid>, AppError> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO connector_webhook_events
            (tenant_id, connector_id, event_id, topic, payload, status, error)
        VALUES ($1, $2, $3, $4, $5, $6::webhook_status, $7)
        ON CONFLICT (connector_id, event_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.connector_id)
    .bind(event.event_id)
    .bind(event.topic)
    .bind(event.payload)
    .bind(event.status)
    .bind(event.error)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(id)
}

async fn finish_event(tx: &mut Transaction<'_, Postgres>, event_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE connector_webhook_events
           SET status = 'processed',
               processed_at = NOW(),
               error = NULL
         WHERE id = $1
        "#,
    )
    .bind(event_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn normalize_woocommerce(payload: &Value) -> Result<NormalizedOrder, AppError> {
    let external_id = value_string(payload.get("id"))
        .ok_or_else(|| AppError::Unprocessable("order id is missing".into()))?;
    let order_number = value_string(payload.get("number")).unwrap_or_else(|| external_id.clone());
    let status = value_string(payload.get("status")).unwrap_or_else(|| "unknown".into());
    let payment_method = value_string(payload.get("payment_method"));
    let total_amount = parse_decimal(value_string(payload.get("total")).as_deref().unwrap_or("0"))?;
    let currency = value_string(payload.get("currency")).unwrap_or_else(|| "BDT".into());
    let placed_at = value_string(payload.get("date_created_gmt"))
        .or_else(|| value_string(payload.get("date_created")))
        .as_deref()
        .and_then(parse_date)
        .unwrap_or_else(Utc::now);

    let billing = payload.get("billing");
    let customer_name = billing.and_then(|value| {
        let first = value_string(value.get("first_name")).unwrap_or_default();
        let last = value_string(value.get("last_name")).unwrap_or_default();
        let name = format!("{first} {last}").trim().to_string();
        (!name.is_empty()).then_some(name)
    });
    let customer_phone = billing
        .and_then(|value| value_string(value.get("phone")))
        .as_deref()
        .and_then(normalize_phone);
    let customer_email = billing
        .and_then(|value| value_string(value.get("email")))
        .as_deref()
        .and_then(normalize_email);

    let cod_amount = if payment_method.as_deref() == Some("cod") {
        total_amount
    } else {
        Decimal::ZERO
    };

    Ok(NormalizedOrder {
        external_id,
        order_number,
        status,
        payment_method,
        total_amount,
        cod_amount,
        currency,
        placed_at,
        customer_name,
        customer_phone,
        customer_email,
        raw: payload.clone(),
    })
}

fn normalize_csv(row: &CsvOrder) -> Result<NormalizedOrder, AppError> {
    let external_id = row.order_id.trim();
    if external_id.is_empty() {
        return Err(AppError::Validation("CSV order_id is empty".into()));
    }

    let total_amount = parse_decimal(row.total.as_deref().unwrap_or("0"))?;
    let payment_method = row
        .payment_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let cod_amount = if payment_method.as_deref() == Some("cod") {
        total_amount
    } else {
        Decimal::ZERO
    };
    let placed_at = row
        .placed_at
        .as_deref()
        .and_then(parse_date)
        .unwrap_or_else(Utc::now);
    let currency = row
        .currency
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() == 3)
        .unwrap_or("BDT")
        .to_uppercase();

    Ok(NormalizedOrder {
        external_id: external_id.to_string(),
        order_number: row
            .order_number
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(external_id)
            .to_string(),
        status: row
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string(),
        payment_method,
        total_amount,
        cod_amount,
        currency,
        placed_at,
        customer_name: row
            .customer_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        customer_phone: row.phone.as_deref().and_then(normalize_phone),
        customer_email: row.email.as_deref().and_then(normalize_email),
        raw: json!({
            "order_id": row.order_id,
            "order_number": row.order_number,
            "status": row.status,
            "payment_method": row.payment_method,
            "total": row.total,
            "currency": row.currency,
            "placed_at": row.placed_at,
            "customer_name": row.customer_name,
            "phone": row.phone,
            "email": row.email,
            "address_1": row.address_1,
            "address_2": row.address_2,
            "city": row.city,
            "state": row.state,
        }),
    })
}

async fn persist_order(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    connector_id: Uuid,
    order: &NormalizedOrder,
    event_type: &str,
) -> Result<Uuid, AppError> {
    let customer_id = upsert_customer(tx, tenant_id, order).await?;

    let order_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO external_orders
            (tenant_id, connector_id, external_id, order_number, customer_id,
             status, payment_method, total_amount, cod_amount, currency, placed_at, raw)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (tenant_id, connector_id, external_id)
        DO UPDATE SET
            order_number = EXCLUDED.order_number,
            customer_id = EXCLUDED.customer_id,
            status = EXCLUDED.status,
            payment_method = EXCLUDED.payment_method,
            total_amount = EXCLUDED.total_amount,
            cod_amount = EXCLUDED.cod_amount,
            currency = EXCLUDED.currency,
            placed_at = EXCLUDED.placed_at,
            raw = EXCLUDED.raw,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(tenant_id)
    .bind(connector_id)
    .bind(&order.external_id)
    .bind(&order.order_number)
    .bind(customer_id)
    .bind(&order.status)
    .bind(order.payment_method.as_deref())
    .bind(order.total_amount)
    .bind(order.cod_amount)
    .bind(&order.currency)
    .bind(order.placed_at)
    .bind(&order.raw)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO order_events (tenant_id, order_id, event_type, payload)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(tenant_id)
    .bind(order_id)
    .bind(event_type)
    .bind(&order.raw)
    .execute(&mut **tx)
    .await?;

    sqlx::query("UPDATE connectors SET last_sync_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(connector_id)
        .execute(&mut **tx)
        .await?;

    Ok(order_id)
}

async fn upsert_customer(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    order: &NormalizedOrder,
) -> Result<Uuid, AppError> {
    if let Some(phone) = &order.customer_phone {
        return Ok(sqlx::query_scalar(
            r#"
            INSERT INTO customers (tenant_id, phone_normalized, email, name)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (tenant_id, phone_normalized) WHERE phone_normalized IS NOT NULL
            DO UPDATE SET
                email = COALESCE(EXCLUDED.email, customers.email),
                name = COALESCE(NULLIF(EXCLUDED.name, ''), customers.name),
                last_seen_at = NOW()
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(phone)
        .bind(order.customer_email.as_deref())
        .bind(order.customer_name.as_deref())
        .fetch_one(&mut **tx)
        .await?);
    }

    if let Some(email) = &order.customer_email {
        return Ok(sqlx::query_scalar(
            r#"
            INSERT INTO customers (tenant_id, email, name)
            VALUES ($1, $2, $3)
            ON CONFLICT (tenant_id, email) WHERE email IS NOT NULL
            DO UPDATE SET
                name = COALESCE(NULLIF(EXCLUDED.name, ''), customers.name),
                last_seen_at = NOW()
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(email)
        .bind(order.customer_name.as_deref())
        .fetch_one(&mut **tx)
        .await?);
    }

    Ok(
        sqlx::query_scalar("INSERT INTO customers (tenant_id, name) VALUES ($1, $2) RETURNING id")
            .bind(tenant_id)
            .bind(order.customer_name.as_deref())
            .fetch_one(&mut **tx)
            .await?,
    )
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(raw)) => Some(raw.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn parse_decimal(raw: &str) -> Result<Decimal, AppError> {
    let cleaned = raw.trim().replace(',', "");
    cleaned
        .parse::<Decimal>()
        .map_err(|_| AppError::Validation(format!("invalid amount '{raw}'")))
}

fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Some(parsed.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(parsed.and_utc());
        }
    }
    None
}

fn normalize_phone(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let normalized = if digits.len() == 13 && digits.starts_with("880") {
        format!("0{}", &digits[3..])
    } else if digits.len() == 10 && digits.starts_with('1') {
        format!("0{digits}")
    } else {
        digits
    };
    (normalized.len() <= 20).then_some(normalized)
}

fn normalize_email(raw: &str) -> Option<String> {
    let email = raw.trim().to_lowercase();
    (email.len() <= 255 && email.contains('@')).then_some(email)
}
