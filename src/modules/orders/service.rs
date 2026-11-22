use super::dtos::OrderSummary;
use super::models::OrderRow;
use crate::common::pagination::Page;
use crate::common::tenant::TenantContext;
use crate::error::AppError;
use crate::state::AppState;

pub async fn list_orders(
    state: &AppState,
    ctx: &TenantContext,
    page: &Page,
    status: Option<&str>,
) -> Result<(Vec<OrderSummary>, i64), AppError> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
          FROM external_orders
         WHERE tenant_id = $1
           AND ($2::text IS NULL OR status = $2)
        "#,
    )
    .bind(ctx.tenant_id)
    .bind(status)
    .fetch_one(&state.db)
    .await?;

    let rows: Vec<OrderRow> = sqlx::query_as(
        r#"
        SELECT o.id, o.order_number, o.external_id, o.status,
               o.total_amount, o.cod_amount, o.currency, o.placed_at,
               c.name AS customer_name, c.phone_normalized AS customer_phone
          FROM external_orders o
          LEFT JOIN customers c ON c.id = o.customer_id
         WHERE o.tenant_id = $1
           AND ($2::text IS NULL OR o.status = $2)
         ORDER BY o.placed_at DESC
         LIMIT $3 OFFSET $4
        "#,
    )
    .bind(ctx.tenant_id)
    .bind(status)
    .bind(page.per_page)
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let orders = rows
        .into_iter()
        .map(|row| OrderSummary {
            id: row.id,
            order_number: row.order_number,
            external_id: row.external_id,
            status: row.status,
            total_amount: format!("{:.2}", row.total_amount),
            cod_amount: format!("{:.2}", row.cod_amount),
            currency: row.currency,
            placed_at: row.placed_at,
            customer_name: row.customer_name,
            customer_phone: row.customer_phone,
        })
        .collect();

    Ok((orders, total))
}
