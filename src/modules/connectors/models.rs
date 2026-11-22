use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct ConnectorRow {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
