use serde::{Deserialize, Serialize};

/// Maps to the PostgreSQL `digital_product_type` enum.
///
/// Row structs are local to `service.rs`: they exist to decode one query and
/// must not be reused as wire types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "digital_product_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DigitalProductType {
    PremadeFile,
    LicenseKey,
    DynamicTool,
}
