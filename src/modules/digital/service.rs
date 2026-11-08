//! Digital product business rules.
//!
//! Every function here is the authoritative implementation of a rule in
//! docs/WORKFLOWS.md section 2 and docs/STORAGE_SECURITY.md.

use uuid::Uuid;

use super::dtos::{
    CheckoutRequest, CheckoutResponse, DownloadResponse, ProductCard, ProductDetail, VaultItem,
};
use crate::error::AppError;
use crate::state::AppState;

pub struct Page {
    pub page: i64,
    pub per_page: i64,
}

impl Page {
    pub fn new(page: Option<i64>, per_page: Option<i64>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1),
            per_page: per_page.unwrap_or(20).clamp(1, 100),
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }
}

pub async fn list_products(
    _state: &AppState,
    _caller: Option<Uuid>,
    _page: &Page,
) -> Result<Vec<ProductCard>, AppError> {
    // TODO: SELECT active products, LEFT JOIN product_files ON is_current,
    //       LEFT JOIN user_entitlements for `owned` when caller is Some.
    Err(AppError::NotImplemented("digital::list_products"))
}

pub async fn get_product(
    _state: &AppState,
    _slug: &str,
    _caller: Option<Uuid>,
) -> Result<ProductDetail, AppError> {
    // TODO: WHERE slug = $1 AND is_active; return 404 when absent.
    Err(AppError::NotImplemented("digital::get_product"))
}

pub async fn my_vault(
    _state: &AppState,
    _user_id: Uuid,
    _page: &Page,
) -> Result<Vec<VaultItem>, AppError> {
    // TODO: JOIN user_entitlements -> digital_products -> current product_files.
    Err(AppError::NotImplemented("digital::my_vault"))
}

/// Grant a download.
///
/// Must follow this exact order (docs/WORKFLOWS.md 2.2):
///   1. ownership check
///   2. atomic `UPDATE ... RETURNING` on download_count / expiry / limit
///   3. resolve the current product_files row
///   4. presign a 900 s URL
///   5. enqueue the audit log write
pub async fn download(
    _state: &AppState,
    _user_id: Uuid,
    _entitlement_id: Uuid,
    _ip: &str,
    _user_agent: Option<&str>,
) -> Result<DownloadResponse, AppError> {
    // The counter must be incremented and validated in one statement:
    //
    //   UPDATE user_entitlements
    //      SET download_count = download_count + 1
    //    WHERE id = $1
    //      AND user_id = $2
    //      AND (expires_at IS NULL OR expires_at > NOW())
    //      AND (max_downloads = -1 OR download_count < max_downloads)
    //   RETURNING download_count, max_downloads;
    //
    // Zero rows => distinguish expired (410) from exhausted (410) from
    // not-owned (403) with a follow-up read, never by trusting the client.
    Err(AppError::NotImplemented("digital::download"))
}

pub async fn checkout(
    _state: &AppState,
    _user_id: Uuid,
    _req: CheckoutRequest,
) -> Result<CheckoutResponse, AppError> {
    // TODO, all inside one transaction:
    //   - charge the wallet with `UPDATE users SET wallet_balance = wallet_balance - $1
    //     WHERE id = $2 AND wallet_balance >= $1 RETURNING wallet_balance`
    //   - insert orders + order_items + payments
    //   - insert one user_entitlements row per product (order_ref = order_number)
    //   - insert wallet_transactions
    // Replaying an idempotency_key returns the original response.
    Err(AppError::NotImplemented("digital::checkout"))
}
