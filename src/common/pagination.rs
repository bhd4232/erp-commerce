//! Shared list pagination.

use crate::common::response::PageMeta;

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
