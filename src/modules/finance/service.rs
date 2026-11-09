//! Wallet reads, top-up intents and invoices (docs/BILLING_WALLET_SPEC.md).

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::dtos::{
    InvoiceDetail, InvoiceSummary, TopupRequest, TopupView, TransactionView, WalletView,
};
use crate::common::pagination::Page;
use crate::error::AppError;
use crate::state::AppState;

const MIN_TOPUP: &str = "1.00";
const MAX_TOPUP: &str = "10000.00";
const TOPUP_TTL_MINUTES: i64 = 30;
const PROVIDERS: [&str; 3] = ["stripe", "bkash", "sslcommerz"];

fn decimal(raw: &str) -> Decimal {
    raw.parse().expect("constant decimal")
}

pub async fn wallet(state: &AppState, user_id: Uuid) -> Result<WalletView, AppError> {
    let (balance, credits): (Decimal, i32) =
        sqlx::query_as("SELECT wallet_balance, ai_credits FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM topup_intents WHERE user_id = $1 AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(WalletView {
        currency: "USD".into(),
        wallet_balance: balance,
        ai_credits: credits,
        pending_topups: pending,
    })
}

pub async fn transactions(
    state: &AppState,
    user_id: Uuid,
    page: &Page,
) -> Result<(Vec<TransactionView>, i64), AppError> {
    let rows = sqlx::query_as::<_, TransactionRow>(
        r#"
        SELECT id, txn_type::text, amount, balance_after, reference_type, note, created_at
          FROM wallet_transactions
         WHERE user_id = $1
         ORDER BY created_at DESC
         LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM wallet_transactions WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total))
}

#[derive(sqlx::FromRow)]
struct TransactionRow {
    id: Uuid,
    txn_type: String,
    amount: Decimal,
    balance_after: Decimal,
    reference_type: Option<String>,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<TransactionRow> for TransactionView {
    fn from(row: TransactionRow) -> Self {
        Self {
            id: row.id,
            txn_type: row.txn_type,
            amount: row.amount,
            balance_after: row.balance_after,
            reference_type: row.reference_type,
            note: row.note,
            created_at: row.created_at,
        }
    }
}

pub async fn topup(
    state: &AppState,
    user_id: Uuid,
    req: TopupRequest,
) -> Result<TopupView, AppError> {
    if req.amount < decimal(MIN_TOPUP) || req.amount > decimal(MAX_TOPUP) {
        return Err(AppError::Unprocessable(format!(
            "amount must be between {MIN_TOPUP} and {MAX_TOPUP}"
        )));
    }
    if req.amount.round_dp(2) != req.amount {
        return Err(AppError::Validation(
            "amount supports at most two decimal places".into(),
        ));
    }
    if !PROVIDERS.contains(&req.provider.as_str()) {
        return Err(AppError::Unprocessable(format!(
            "unknown payment provider '{}'",
            req.provider
        )));
    }

    let currency = req.currency.unwrap_or_else(|| "USD".into()).to_uppercase();
    if currency.len() != 3 {
        return Err(AppError::Validation("currency must be ISO 4217".into()));
    }

    let expires_at = Utc::now() + Duration::minutes(TOPUP_TTL_MINUTES);

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO topup_intents (user_id, amount, currency, provider, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(req.amount)
    .bind(&currency)
    .bind(&req.provider)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    let redirect_url = state
        .cfg
        .payment_redirect_base
        .as_ref()
        .map(|base| format!("{}/{id}", base.trim_end_matches('/')));

    Ok(TopupView {
        topup_id: id,
        status: "pending".into(),
        amount: req.amount,
        currency,
        redirect_url,
        expires_at,
    })
}

#[derive(sqlx::FromRow)]
struct InvoiceRow {
    id: Uuid,
    invoice_number: String,
    order_id: Option<Uuid>,
    currency: String,
    subtotal: Decimal,
    tax_amount: Decimal,
    total: Decimal,
    status: String,
    issued_at: DateTime<Utc>,
    paid_at: Option<DateTime<Utc>>,
    pdf_path: Option<String>,
}

impl From<InvoiceRow> for InvoiceSummary {
    fn from(row: InvoiceRow) -> Self {
        Self {
            id: row.id,
            invoice_number: row.invoice_number,
            currency: row.currency,
            subtotal: row.subtotal,
            tax_amount: row.tax_amount,
            total: row.total,
            status: row.status,
            issued_at: row.issued_at,
            paid_at: row.paid_at,
        }
    }
}

pub async fn invoices(
    state: &AppState,
    user_id: Uuid,
    page: &Page,
) -> Result<(Vec<InvoiceSummary>, i64), AppError> {
    let rows = sqlx::query_as::<_, InvoiceRow>(
        r#"
        SELECT id, invoice_number, order_id, currency, subtotal, tax_amount,
               total, status::text, issued_at, paid_at, pdf_path
          FROM invoices
         WHERE user_id = $1
         ORDER BY issued_at DESC
         LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total))
}

pub async fn invoice(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    invoice_id: Uuid,
) -> Result<InvoiceDetail, AppError> {
    let row = sqlx::query_as::<_, InvoiceRow>(
        r#"
        SELECT id, invoice_number, order_id, currency, subtotal, tax_amount,
               total, status::text, issued_at, paid_at, pdf_path
          FROM invoices
         WHERE id = $1 AND ($2::boolean OR user_id = $3)
        "#,
    )
    .bind(invoice_id)
    .bind(is_admin)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let order_id = row.order_id;

    let pdf_url = match row.pdf_path.as_deref() {
        Some(path) => {
            let filename = path.rsplit('/').next().unwrap_or("invoice.pdf");
            Some(
                state
                    .storage
                    .presign_get(path, filename, "application/pdf")
                    .await?,
            )
        }
        None => None,
    };

    Ok(InvoiceDetail {
        summary: row.into(),
        order_id,
        pdf_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topup_bounds_are_sane() {
        assert!(decimal(MIN_TOPUP) < decimal(MAX_TOPUP));
        assert_eq!(decimal(MAX_TOPUP).round_dp(2), decimal(MAX_TOPUP));
    }

    #[test]
    fn providers_are_the_documented_three() {
        assert_eq!(PROVIDERS.len(), 3);
        assert!(PROVIDERS.contains(&"stripe"));
        assert!(!PROVIDERS.contains(&"wallet"));
    }
}
