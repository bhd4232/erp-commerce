# Rule: Connectors, webhooks and order ingest

**Tier:** 1 for secrets/signature/idempotency; 2 for order upsert and tenant
scope.

- Connector secrets are encrypted at rest with `pgp_sym_encrypt` using
  `CONNECTOR_ENCRYPTION_KEY`. The plaintext is returned only once at creation;
  no list/detail DTO contains it.
- The WooCommerce signature is `base64(HMAC-SHA256(raw_body, secret))`. Verify
  the raw body **before** parsing or writing an order. A bad signature writes a
  `rejected` webhook event and no order.
- The idempotency key is `(connector_id, event_id)`. Use
  `X-WC-Webhook-Delivery-ID`; if absent, hash the raw body. A replay returns
  `duplicate` and changes no order.
- An order is unique on `(tenant_id, connector_id, external_id)`. A webhook
  update upserts; it never creates a second row.
- Normalize phone to digits; accept `880...`, `1...` and `01...`. Identify a
  customer by phone first, then email. Lowercase email.
- A CSV import is one transaction. One invalid row rolls the whole batch back.
- Every connector/webhook/order query filters `tenant_id`; a foreign connector
  returns `404`.
- REST credentials are optional and must be supplied as a complete set; they are
  encrypted with the same key as the webhook secret and never returned after
  creation.
- A REST sync is bounded (10 pages) and upserts each page in one transaction;
  the base URL is site-controlled, so never log the URL with credentials.
- Never log the raw body, phone, address, signature or secret. The webhook event
  row stores the payload for retry/debugging; it is not a DTO.
- A new webhook topic that changes money or status needs an explicit test.
