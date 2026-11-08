//! The response envelope from docs/API_SPEC.md: `{ "data": ..., "meta": ... }`.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub data: T,
    pub meta: Option<PageMeta>,
}

#[derive(Debug, Serialize)]
pub struct PageMeta {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

impl<T> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }

    pub fn with_meta(data: T, meta: PageMeta) -> Self {
        Self {
            data,
            meta: Some(meta),
        }
    }
}
