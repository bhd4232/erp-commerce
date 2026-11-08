use std::env;

/// Parsed, validated runtime configuration.
///
/// Required values fail fast at startup; optional values fall back to the
/// documented defaults. Nothing in the application reads `std::env` directly.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,

    pub database_url: String,
    pub database_max_connections: u32,
    pub redis_url: String,

    pub jwt_secret: String,
    pub jwt_ttl_seconds: i64,

    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,

    /// Signed download URL lifetime. Always clamped to at most 900 s.
    pub signed_url_ttl_seconds: u64,
    /// Signed upload (PUT) URL lifetime.
    pub signed_upload_ttl_seconds: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
    #[error("invalid value for {0}: {1}")]
    Invalid(&'static str, String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "JWT_SECRET",
                "must be at least 32 characters".to_string(),
            ));
        }

        Ok(Self {
            host: optional("APP_HOST").unwrap_or_else(|| "0.0.0.0".to_string()),
            port: parse_optional("APP_PORT", 8080u16)?,

            database_url: required("DATABASE_URL")?,
            database_max_connections: parse_optional("DATABASE_MAX_CONNECTIONS", 10u32)?,
            redis_url: required("REDIS_URL")?,

            jwt_secret,
            jwt_ttl_seconds: parse_optional("JWT_TTL_SECONDS", 3600i64)?,

            s3_endpoint: required("S3_ENDPOINT")?,
            s3_region: optional("S3_REGION").unwrap_or_else(|| "auto".to_string()),
            s3_bucket: required("S3_BUCKET")?,
            s3_access_key: required("S3_ACCESS_KEY")?,
            s3_secret_key: required("S3_SECRET_KEY")?,

            // Hard ceiling: the spec forbids URLs longer than 15 minutes.
            signed_url_ttl_seconds: parse_optional("SIGNED_URL_TTL_SECONDS", 900u64)?.min(900),
            signed_upload_ttl_seconds: parse_optional("SIGNED_UPLOAD_TTL_SECONDS", 300u64)?,
        })
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(v) if !v.trim().is_empty() => Ok(v),
        _ => Err(ConfigError::Missing(key)),
    }
}

fn optional(key: &'static str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn parse_optional<T>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match optional(key) {
        None => Ok(default),
        Some(raw) => raw
            .parse::<T>()
            .map_err(|e| ConfigError::Invalid(key, format!("{e} (got {raw:?})"))),
    }
}
