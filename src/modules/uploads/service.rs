//! Pre-signed upload issuance (docs/API_SPEC.md section 6).
//!
//! The server owns the key and the limits. The client never chooses a prefix,
//! and no upload is accepted until its type and size are allow-listed here.

use chrono::{Duration, Utc};
use uuid::Uuid;

use super::dtos::{PresignRequest, PresignResponse};
use crate::common::storage::prefix;
use crate::error::AppError;
use crate::state::AppState;

const MIB: i64 = 1024 * 1024;

struct Rule {
    extension: &'static str,
    max_bytes: i64,
}

fn rule_for(purpose: &str, content_type: &str) -> Result<Rule, AppError> {
    let allowed: &[(&str, &str)] = match purpose {
        "brief_asset" => &[
            ("image/png", "png"),
            ("image/jpeg", "jpg"),
            ("image/webp", "webp"),
            ("application/pdf", "pdf"),
        ],
        "generator_input" => &[("image/png", "png")],
        _ => {
            return Err(AppError::Unprocessable(format!(
                "unknown upload purpose '{purpose}'"
            )))
        }
    };

    let extension = allowed
        .iter()
        .find(|(mime, _)| *mime == content_type)
        .map(|(_, ext)| *ext)
        .ok_or(AppError::UnsupportedMediaType)?;

    Ok(Rule {
        extension,
        max_bytes: 10 * MIB,
    })
}

pub async fn presign(
    state: &AppState,
    user_id: Uuid,
    req: PresignRequest,
) -> Result<PresignResponse, AppError> {
    if req.filename.trim().is_empty() || req.filename.len() > 255 {
        return Err(AppError::Validation(
            "filename must be 1-255 characters".into(),
        ));
    }

    let rule = rule_for(&req.purpose, &req.content_type)?;

    if req.size_bytes <= 0 || req.size_bytes > rule.max_bytes {
        return Err(AppError::PayloadTooLarge);
    }

    let key = prefix::brief_upload(
        &user_id.to_string(),
        &format!("{}.{}", Uuid::new_v4(), rule.extension),
    );

    let upload_url = state.storage.presign_put(&key, &req.content_type).await?;

    Ok(PresignResponse {
        upload_url,
        key,
        expires_at: Utc::now() + Duration::seconds(state.cfg.signed_upload_ttl_seconds as i64),
        max_size_bytes: rule.max_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_purpose() {
        assert!(matches!(
            rule_for("avatar", "image/png"),
            Err(AppError::Unprocessable(_))
        ));
    }

    #[test]
    fn rejects_disallowed_type() {
        assert!(matches!(
            rule_for("brief_asset", "image/gif"),
            Err(AppError::UnsupportedMediaType)
        ));
        assert!(matches!(
            rule_for("generator_input", "image/jpeg"),
            Err(AppError::UnsupportedMediaType)
        ));
    }

    #[test]
    fn accepts_the_allow_list() {
        assert_eq!(
            rule_for("brief_asset", "image/png").unwrap().extension,
            "png"
        );
        assert_eq!(
            rule_for("brief_asset", "application/pdf")
                .unwrap()
                .extension,
            "pdf"
        );
        assert_eq!(
            rule_for("generator_input", "image/png").unwrap().max_bytes,
            10 * MIB
        );
    }
}
