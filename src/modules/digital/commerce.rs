//! Checkout and refund: every path that moves money.
//!
//! Each function is one transaction. A partial write here is a bug, not a
//! slowdown — see docs/BILLING_WALLET_SPEC.md section 4.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::dtos::{CheckoutRequest, CheckoutResponse, RefundResponse};
use crate::error::AppError;
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct PriceRow {
    id: Uuid,
    price: Decimal,
    is_active: bool,
}

#[derive(sqlx::FromRow)]
struct ExistingOrder {
    id: Uuid,
    order_number: String,
    status: String,
    total_amount: Decimal,
}

#[derive(sqlx::FromRow)]
struct RefundableOrder {
    id: Uuid,
    order_number: String,
    status: String,
    total_amount: Decimal,
    currency: String,
    created_at: DateTime<Utc>,
    user_id: Uuid,
}

fn validate_checkout(req: &CheckoutRequest) -> Result<(), AppError> {
    if req.items.is_empty() {
        return Err(AppError::Unprocessable("cart is empty".into()));
    }
    if req.payment_provider != "wallet" {
        return Err(AppError::Unprocessable(
            "only wallet checkout is supported in v1".into(),
        ));
    }

    let mut seen = HashSet::new();
    for item in &req.items {
        if !(1..=99).contains(&item.quantity) {
            return Err(AppError::Validation("quantity must be 1-99".into()));
        }
        if !seen.insert(item.product_id) {
            return Err(AppError::Validation(
                "duplicate product in cart".into(),
            ));
        }
    }

    if let Some(key) = req.idempotency_key.as_deref() {
        if key.is_empty() || key.len() > 100 {
            return Err(AppError::Validation(
                "idempotency_key must be 1-100 characters".into(),
            ));
        }
    }

    Ok(())
}

pub async fn checkout(
    state: &AppState,
    user_id: Uuid,
    req: CheckoutRequest,
) -> Result<CheckoutResponse, AppError> {
    validate_checkout(&req)?;

    let mut tx = state.db.begin().await?;

    let ids: Vec<Uuid> = req.items.iter().map(|i| i.product_id).collect();
    let rows = sqlx::query_as::<_, PriceRow>(
        "SELECT id, price, is_active FROM digital_products WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut total = Decimal::ZERO;
    let mut prices: HashMap<Uuid, Decimal> = HashMap::new();
    for item in &req.items {
        let row = rows
            .iter()
            .find(|r| r.id == item.product_id)
            .ok_or_else(|| AppError::Unprocessable(format!("unknown product {}", item.product_id)))?;

        if !row.is_active {
            return Err(AppError::Unprocessable(format!(
                "product {} is not for sale",
                item.product_id
            )));
        }

        total += row.price * Decimal::from(item.quantity);
        prices.insert(item.product_id, row.price);
    }

    if let Some(key) = req.idempotency_key.as_deref() {
        let existing = sqlx::query_as::<_, ExistingOrder>(
            r#"
            SELECT id, order_number, status::text AS status, total_amount
              FROM orders
             WHERE idempotency_key = $1 AND user_id = $2
            "#,
        )
        .bind(key)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(order) = existing {
            if order.total_amount != total {
                return Err(AppError::InvalidState(
                    "idempotency key reused with a different cart".into(),
                ));
            }
            tx.commit().await?;
            return replay(state, user_id, order).await;
        }
    }

    let inserted = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        INSERT INTO orders
            (order_number, user_id, status, currency, total_amount, idempotency_key)
        VALUES
            (next_order_number('DIG'), $1, 'paid', 'USD', $2, $3)
        ON CONFLICT (idempotency_key) DO NOTHING
        RETURNING id, order_number
        "#,
    )
    .bind(user_id)
    .bind(total)
    .bind(req.idempotency_key.as_deref())
    .fetch_optional(&mut *tx)
    .await?;

    let (order_id, order_number) = match inserted {
        Some(row) => row,
        None => {
            // Lost a concurrent idempotency race. Roll back so this request
            // writes nothing, then return the winner's order.
            tx.rollback().await?;
            let order = sqlx::query_as::<_, ExistingOrder>(
                r#"
                SELECT id, order_number, status::text AS status, total_amount
                  FROM orders
                 WHERE idempotency_key = $1 AND user_id = $2
                "#,
            )
            .bind(req.idempotency_key.as_deref())
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::InvalidState(
                "idempotency conflict without a winning order".into(),
            ))?;

            if order.total_amount != total {
                return Err(AppError::InvalidState(
                    "idempotency key reused with a different cart".into(),
                ));
            }
            return replay(state, user_id, order).await;
        }
    };

    let balance_after: Option<Decimal> = sqlx::query_scalar(
        r#"
        UPDATE users
           SET wallet_balance = wallet_balance - $1,
               updated_at = NOW()
         WHERE id = $2 AND wallet_balance >= $1
        RETURNING wallet_balance
        "#,
    )
    .bind(total)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let balance_after = balance_after.ok_or(AppError::InsufficientFunds)?;

    for item in &req.items {
        let unit_price = *prices.get(&item.product_id).ok_or_else(|| {
            AppError::Unprocessable(format!("unknown product {}", item.product_id))
        })?;

        sqlx::query(
            "INSERT INTO order_items (order_id, product_id, unit_price, quantity)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(order_id)
        .bind(item.product_id)
        .bind(unit_price)
        .bind(item.quantity)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO payments (order_id, provider, amount, paid_at)
         VALUES ($1, 'wallet', $2, NOW())",
    )
    .bind(order_id)
    .bind(total)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type,
             reference_id, note, gateway, currency)
        VALUES ($1, 'debit', $2, $3, 'digital_order', $4, $5, 'internal', 'USD')
        "#,
    )
    .bind(user_id)
    .bind(total)
    .bind(balance_after)
    .bind(order_id)
    .bind(format!("checkout {order_number}"))
    .execute(&mut *tx)
    .await?;

    let mut entitlements_created = 0usize;
    for item in &req.items {
        let granted: Option<Uuid> = sqlx::query_scalar(
            r#"
            INSERT INTO user_entitlements (user_id, product_id, order_ref)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, product_id, order_ref) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(item.product_id)
        .bind(&order_number)
        .fetch_optional(&mut *tx)
        .await?;

        if granted.is_some() {
            entitlements_created += 1;
        }
    }

    tx.commit().await?;

    Ok(CheckoutResponse {
        order_id,
        order_number,
        status: "paid".to_string(),
        total_amount: total,
        entitlements_created,
    })
}

async fn replay(
    state: &AppState,
    user_id: Uuid,
    order: ExistingOrder,
) -> Result<CheckoutResponse, AppError> {
    let created: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_entitlements WHERE user_id = $1 AND order_ref = $2",
    )
    .bind(user_id)
    .bind(&order.order_number)
    .fetch_one(&state.db)
    .await?;

    Ok(CheckoutResponse {
        order_id: order.id,
        order_number: order.order_number,
        status: order.status,
        total_amount: order.total_amount,
        entitlements_created: created as usize,
    })
}

/// Full refund of a paid digital order (admin only).
///
/// Refused when the entitlement has been downloaded or the 24 h window has
/// passed. The ledger row, the balance, the order status and the entitlement
/// revoke all happen in one transaction.
pub async fn refund_order(
    state: &AppState,
    order_id: Uuid,
) -> Result<RefundResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let order = sqlx::query_as::<_, RefundableOrder>(
        r#"
        SELECT id, order_number, status::text AS status, total_amount, currency,
               created_at, user_id
          FROM orders
         WHERE id = $1
           FOR UPDATE
        "#,
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let RefundableOrder {
        id,
        order_number,
        status,
        total_amount: total,
        currency,
        created_at,
        user_id,
    } = order;

    if status != "paid" {
        return Err(AppError::InvalidState(format!(
            "order is '{status}' and cannot be refunded"
        )));
    }
    if created_at < Utc::now() - Duration::hours(24) {
        return Err(AppError::InvalidState(
            "refund window of 24 hours has passed".into(),
        ));
    }

    let downloads: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
          FROM audit_download_logs l
          JOIN user_entitlements e ON e.id = l.entitlement_id
         WHERE e.order_ref = $1
        "#,
    )
    .bind(&order_number)
    .fetch_one(&mut *tx)
    .await?;

    if downloads > 0 {
        return Err(AppError::InvalidState(
            "entitlement has already been downloaded".into(),
        ));
    }

    let balance_after: Decimal = sqlx::query_scalar(
        "UPDATE users SET wallet_balance = wallet_balance + $1, updated_at = NOW()
          WHERE id = $2
        RETURNING wallet_balance",
    )
    .bind(total)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO wallet_transactions
            (user_id, txn_type, amount, balance_after, reference_type,
             reference_id, note, gateway, currency)
        VALUES ($1, 'refund', $2, $3, 'digital_order', $4, $5, 'internal', $6)
        "#,
    )
    .bind(user_id)
    .bind(total)
    .bind(balance_after)
    .bind(id)
    .bind(format!("refund {order_number}"))
    .bind(&currency)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE user_entitlements SET expires_at = NOW()
          WHERE order_ref = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(&order_number)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE orders SET status = 'refunded', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(RefundResponse {
        order_id: id,
        order_number,
        status: "refunded".to_string(),
        refunded_amount: total,
        currency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::digital::dtos::CheckoutItem;

    fn item(product_id: Uuid, quantity: i32) -> CheckoutItem {
        CheckoutItem {
            product_id,
            quantity,
        }
    }

    fn request(items: Vec<CheckoutItem>, provider: &str) -> CheckoutRequest {
        CheckoutRequest {
            items,
            payment_provider: provider.to_string(),
            idempotency_key: None,
        }
    }

    #[test]
    fn empty_cart_is_unprocessable() {
        assert!(matches!(
            validate_checkout(&request(vec![], "wallet")).unwrap_err(),
            AppError::Unprocessable(_)
        ));
    }

    #[test]
    fn only_wallet_is_supported() {
        assert!(matches!(
            validate_checkout(&request(vec![item(Uuid::new_v4(), 1)], "card")).unwrap_err(),
            AppError::Unprocessable(_)
        ));
    }

    #[test]
    fn duplicate_products_are_rejected() {
        let id = Uuid::new_v4();
        assert!(matches!(
            validate_checkout(&request(vec![item(id, 1), item(id, 2)], "wallet")).unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[test]
    fn quantity_outside_range_is_rejected() {
        for qty in [0, 100] {
            assert!(matches!(
                validate_checkout(&request(vec![item(Uuid::new_v4(), qty)], "wallet")).unwrap_err(),
                AppError::Validation(_)
            ));
        }
    }
}
