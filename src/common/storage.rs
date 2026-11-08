//! Private object storage (S3 / Cloudflare R2).
//!
//! The bucket is never public. The only way bytes leave it is a short-lived
//! SigV4 pre-signed URL generated here.

use std::time::Duration;

use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;

use crate::config::Config;
use crate::error::AppError;

/// Prefix layout. Mirrors docs/STORAGE_SECURITY.md section 1.
pub mod prefix {
    pub const PRODUCTS: &str = "protected-products";
    pub const DELIVERABLES: &str = "service-deliverables";
    pub const UPLOADS: &str = "user-uploads";

    pub fn product_asset(product_id: &str, version: &str, filename: &str) -> String {
        format!("{PRODUCTS}/{product_id}/{version}/{filename}")
    }

    pub fn deliverable_raw(order_id: &str, filename: &str) -> String {
        format!("{DELIVERABLES}/{order_id}/raw/{filename}")
    }

    pub fn deliverable_preview(order_id: &str, filename: &str) -> String {
        format!("{DELIVERABLES}/{order_id}/previews/{filename}")
    }

    pub fn deliverable_final(order_id: &str, filename: &str) -> String {
        format!("{DELIVERABLES}/{order_id}/final/{filename}")
    }

    pub fn brief_upload(user_id: &str, filename: &str) -> String {
        format!("{UPLOADS}/{user_id}/briefs/{filename}")
    }
}

#[derive(Clone)]
pub struct Storage {
    client: Client,
    bucket: String,
    download_ttl: Duration,
    upload_ttl: Duration,
}

impl Storage {
    pub async fn new(cfg: &Config) -> Self {
        let credentials = Credentials::new(
            cfg.s3_access_key.clone(),
            cfg.s3_secret_key.clone(),
            None,
            None,
            "static-env",
        );

        let shared = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(cfg.s3_region.clone()))
            .endpoint_url(cfg.s3_endpoint.clone())
            .credentials_provider(credentials)
            .load()
            .await;

        let s3_config = aws_sdk_s3::config::Builder::from(&shared)
            .force_path_style(true)
            .build();

        Self {
            client: Client::from_conf(s3_config),
            bucket: cfg.s3_bucket.clone(),
            download_ttl: Duration::from_secs(cfg.signed_url_ttl_seconds),
            upload_ttl: Duration::from_secs(cfg.signed_upload_ttl_seconds),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Pre-sign a GET for an existing object.
    ///
    /// TTL is fixed by config and clamped to 900 s in `Config::from_env`.
    /// The returned URL is the only thing the client receives — never the key.
    pub async fn presign_get(
        &self,
        key: &str,
        filename: &str,
        mime_type: &str,
    ) -> Result<String, AppError> {
        let signing = PresigningConfig::expires_in(self.download_ttl)
            .map_err(|e| AppError::Internal(format!("invalid signing config: {e}")))?;

        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .response_content_disposition(format!("attachment; filename=\"{}\"", sanitize_filename(filename)))
            .response_content_type(mime_type)
            .presigned(signing)
            .await
            .map_err(|e| AppError::Internal(format!("presign failed: {e}")))?;

        Ok(request.uri().to_string())
    }

    /// Pre-sign a PUT for a client brief upload.
    ///
    /// Callers must validate purpose, extension, MIME and size *before*
    /// calling this, and must verify the object after upload.
    pub async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<String, AppError> {
        let signing = PresigningConfig::expires_in(self.upload_ttl)
            .map_err(|e| AppError::Internal(format!("invalid signing config: {e}")))?;

        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .presigned(signing)
            .await
            .map_err(|e| AppError::Internal(format!("presign failed: {e}")))?;

        Ok(request.uri().to_string())
    }

    /// Download an object into memory. Used by workers, never by handlers.
    pub async fn get_bytes(&self, key: &str) -> Result<Vec<u8>, AppError> {
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("storage read failed: {e}")))?;

        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| AppError::Internal(format!("storage read failed: {e}")))?;

        Ok(bytes.into_bytes().to_vec())
    }
}

/// Strip path separators and quotes so `Content-Disposition` cannot be broken.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '"' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}
