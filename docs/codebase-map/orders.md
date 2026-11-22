# Module: Orders

**Directory:** `src/modules/orders/`  
**Tables:** `external_orders` (read), `customers` (read)  
**Endpoints:** `GET /orders`

## Files

- `dtos.rs` — tenant-scoped order summary; money is a decimal string.
- `service.rs` — filtered/paginated read query.
- `handlers.rs` — HTTP only.

## Invariants

- Every query filters `tenant_id`; a foreign order is invisible.
- `status` is an optional exact filter.
- Money is formatted as two-decimal strings; never a float.
- Customer name/phone are optional because some orders lack customer identity.

## Tests

- `tests/connector_flow.rs` — webhook/CSV-created orders are visible to the
  owning tenant and invisible to another tenant.
