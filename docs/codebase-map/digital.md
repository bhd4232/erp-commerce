# Module: Digital Commerce

**Directory:** `src/modules/digital/`  
**Tables:** `digital_products`, `product_files`, `user_entitlements`,
`orders`, `order_items`, `payments`, `audit_download_logs`  
**Endpoints:** catalogue, product detail, vault, checkout, download, refund

## Files

- `models.rs` — product/file/order/entitlement rows.
- `dtos.rs` — public catalogue, checkout, vault, download, refund types.
- `commerce.rs` — pricing/cart validation and pure rules.
- `service.rs` — catalogue, checkout transaction, vault, atomic download,
  refund transaction.
- `handlers.rs` — HTTP only.

## Invariants

- Checkout is one transaction: wallet debit + ledger row + order + items +
  payment + entitlements.
- An idempotency key charges once.
- Download limit is enforced by `UPDATE ... RETURNING`, never read-then-write.
- A denied download does not increment the counter.
- Refund writes one `refund` row and revokes every entitlement in the same
  transaction; repeat refund is `409`.
- No DTO contains `storage_path`; only signed URLs leave the API.

## Tests

- `tests/digital_flow.rs` — six Postgres-backed cases including overspend
  concurrency and download limit.
