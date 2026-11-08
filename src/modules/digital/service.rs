//! Digital product read paths: catalogue, vault and downloads.
//!
//! Money-moving paths (checkout, refund) live in `commerce.rs`.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::dtos::{
    DownloadResponse, ProductCard, ProductDetail, PublicFileInfo, VaultItem, VaultProductRef,
};
use super::models::DigitalProductType;
use crate::common::response::PageMeta;
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

    pub fn meta(&self, total: i64) -> PageMeta {
        PageMeta {
            page: self.page,
            per_page: self.per_page,
            total,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ProductRow {
    id: Uuid,
    title: String,
    slug: String,
    product_type: DigitalProductType,
    price: Decimal,
    thumbnail_url: Option<String>,
    current_version: Option<String>,
    owned: bool,
}

#[derive(sqlx::FromRow)]
struct ProductDetailRow {
    id: Uuid,
    title: String,
    slug: String,
    product_type: DigitalProductType,
    price: Decimal,
    description: Option<String>,
    thumbnail_url: Option<String>,
    version: Option<String>,
    file_size_bytes: Option<i64>,
    mime_type: Option<String>,
    owned: bool,
}

#[derive(sqlx::FromRow)]
struct VaultRow {
    entitlement_id: Uuid,
    product_id: Uuid,
    title: String,
    slug: String,
    version: Option<String>,
    download_count: i32,
    max_downloads: i32,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

pub async fn list_products(
    state: &AppState,
    caller: Option<Uuid>,
    page: &Page,
) -> Result<(Vec<ProductCard>, i64), AppError> {
    let rows = sqlx::query_as::<_, ProductRow>(
        r#"
        SELECT p.id, p.title, p.slug, p.product_type, p.price, p.thumbnail_url,
               f.version AS current_version,
               (e.id IS NOT NULL) AS owned
          FROM digital_products p
          LEFT JOIN product_files f
                 ON f.product_id = p.id AND f.is_current
          LEFT JOIN user_entitlements e
                 ON e.product_id = p.id AND e.user_id = $1
         WHERE p.is_active
         ORDER BY p.created_at DESC
         LIMIT $2 OFFSET $3
        "#,
    )
    .bind(caller)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM digital_products WHERE is_active")
        .fetch_one(&state.db)
        .await?;

    let items = rows
        .into_iter()
        .map(|r| ProductCard {
            id: r.id,
            title: r.title,
            slug: r.slug,
            product_type: r.product_type,
            price: r.price,
            thumbnail_url: r.thumbnail_url,
            current_version: r.current_version,
            owned: r.owned,
        })
        .collect();

    Ok((items, total))
}

pub async fn get_product(
    state: &AppState,
    slug: &str,
    caller: Option<Uuid>,
) -> Result<ProductDetail, AppError> {
    let row = sqlx::query_as::<_, ProductDetailRow>(
        r#"
        SELECT p.id, p.title, p.slug, p.product_type, p.price, p.description,
               p.thumbnail_url,
               f.version, f.file_size_bytes, f.mime_type,
               (e.id IS NOT NULL) AS owned
          FROM digital_products p
          LEFT JOIN product_files f
                 ON f.product_id = p.id AND f.is_current
          LEFT JOIN user_entitlements e
                 ON e.product_id = p.id AND e.user_id = $2
         WHERE p.slug = $1 AND p.is_active
        "#,
    )
    .bind(slug)
    .bind(caller)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let file = match (row.version, row.file_size_bytes, row.mime_type) {
        (Some(version), Some(file_size_bytes), Some(mime_type)) => Some(PublicFileInfo {
            version,
            file_size_bytes,
            mime_type,
        }),
        _ => None,
    };

    Ok(ProductDetail {
        id: row.id,
        title: row.title,
        slug: row.slug,
        product_type: row.product_type,
        price: row.price,
        description: row.description,
        thumbnail_url: row.thumbnail_url,
        file,
        owned: row.owned,
    })
}

pub async fn my_vault(
    state: &AppState,
    user_id: Uuid,
    page: &Page,
) -> Result<(Vec<VaultItem>, i64), AppError> {
    let rows = sqlx::query_as::<_, VaultRow>(
        r#"
        SELECT e.id AS entitlement_id, p.id AS product_id, p.title, p.slug,
               f.version, e.download_count, e.max_downloads, e.expires_at,
               e.created_at
          FROM user_entitlements e
          JOIN digital_products p ON p.id = e.product_id
          LEFT JOIN product_files f
                 ON f.product_id = p.id AND f.is_current
         WHERE e.user_id = $1
           AND (e.expires_at IS NULL OR e.expires_at > NOW())
         ORDER BY e.created_at DESC
         LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM user_entitlements
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| VaultItem {
            entitlement_id: r.entitlement_id,
            product: VaultProductRef {
                id: r.product_id,
                title: r.title,
                slug: r.slug,
            },
            version: r.version,
            download_count: r.download_count,
            max_downloads: r.max_downloads,
            expires_at: r.expires_at,
            created_at: r.created_at,
        })
        .collect();

    Ok((items, total))
}

/// Grant a download (docs/WORKFLOWS.md 2.2).
///
/// Ownership, expiry and the download limit are validated inside the same
/// `UPDATE ... RETURNING` that increments the counter, so two concurrent
/// requests cannot both take the last allowed download.
pub async fn download(
    state: &AppState,
    user_id: Uuid,
    entitlement_id: Uuid,
    ip: &str,
    user_agent: Option<&str>,
) -> Result<DownloadResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let updated = sqlx::query_as::<_, (i32, i32, Uuid)>(
        r#"
        UPDATE user_entitlements
           SET download_count = download_count + 1
         WHERE id = $1
           AND user_id = $2
           AND (expires_at IS NULL OR expires_at > NOW())
           AND (max_downloads = -1 OR download_count < max_downloads)
        RETURNING download_count, max_downloads, product_id
        "#,
    )
    .bind(entitlement_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((_, _, product_id)) = updated else {
        tx.rollback().await?;
        return Err(classify_download_failure(state, user_id, entitlement_id).await);
    };

    let file = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT version, storage_path, mime_type
          FROM product_files
         WHERE product_id = $1 AND is_current
         LIMIT 1
        "#,
    )
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query(
        "INSERT INTO audit_download_logs (entitlement_id, ip_address, user_agent)
         VALUES ($1, $2, $3)",
    )
    .bind(entitlement_id)
    .bind(ip)
    .bind(user_agent)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let filename = file
        .1
        .rsplit('/')
        .next()
        .unwrap_or("download")
        .to_string();
    let download_url = state.storage.presign_get(&file.1, &filename, &file.2).await?;

    Ok(DownloadResponse {
        download_url,
        expires_at: Utc::now() + Duration::seconds(state.cfg.signed_url_ttl_seconds as i64),
        filename,
    })
}

async fn classify_download_failure(
    state: &AppState,
    user_id: Uuid,
    entitlement_id: Uuid,
) -> AppError {
    let row = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>, i32, i32)>(
        "SELECT user_id, expires_at, download_count, max_downloads
           FROM user_entitlements WHERE id = $1",
    )
    .bind(entitlement_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(None) => AppError::NotFound,
        Ok(Some((owner, _, _, _))) if owner != user_id => AppError::Forbidden,
        Ok(Some((_, Some(expires), _, _))) if expires <= Utc::now() => {
            AppError::EntitlementExpired
        }
        // Either the limit was already reached, or another request consumed
        // the last slot between our UPDATE and this read.
        Ok(Some(_)) => AppError::DownloadsExhausted,
        Err(e) => AppError::Db(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_clamps_bad_pagination() {
        let page = Page::new(Some(0), Some(500));
        assert_eq!(page.page, 1);
        assert_eq!(page.per_page, 100);
        assert_eq!(page.offset(), 0);
    }
}
