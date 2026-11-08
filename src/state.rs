use std::sync::Arc;

use sqlx::PgPool;

use crate::common::storage::Storage;
use crate::config::Config;

/// Shared, cheaply-cloneable application state.
///
/// Only infrastructure handles live here. Business state belongs in the
/// database and is never cached in this struct.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub storage: Storage,
}
