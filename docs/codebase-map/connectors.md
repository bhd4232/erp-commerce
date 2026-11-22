# Module: Connectors

**Directory:** `src/modules/connectors/`  
**Tables:** `connectors`, `connector_webhook_events`, `customers`,
`external_orders`, `order_events`  
**Endpoints:** `GET/POST /connectors`, `POST /connectors/{id}/import`,
`POST /connectors/{id}/test`, `POST /connectors/{id}/sync`,
`GET /connectors/{id}/health`, `POST /webhooks/woocommerce/{connector_id}`

## Files

- `models.rs` — connector list row.
- `dtos.rs` — connector summaries, created-secret response, webhook/import
  results, canonical CSV row.
- `service.rs` — connector creation, secret encryption/decryption, WooCommerce
  signature verification, REST test/sync/health, idempotent webhook ingest, CSV
  import, customer/order upsert.
- `handlers.rs` — HTTP only.

## Invariants

- `ctx.require_manager()` is required to create a connector or import CSV.
- Connector secrets are encrypted at rest with `pgp_sym_encrypt`; only the
  creation response returns the plaintext, once.
- WooCommerce signature is HMAC-SHA256(base64) over the **raw** body; verify
  before parsing.
- `X-WC-Webhook-Delivery-ID` (or body hash) is unique per connector; a replay
  changes no order.
- Evidence order: `(tenant_id, connector_id, external_id)` unique; upsert.
- A rejected signature writes a `rejected` webhook event and no order.
- A CSV import is one transaction; one invalid row rolls the batch back.
- Every query filters `tenant_id`; a foreign connector is `404`.

## Tests

- `tests/connector_flow.rs` — signed webhook, replay, bad signature, CSV import,
  cross-tenant isolation, staff guard.
