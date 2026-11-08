# REST API Specifications

Base path: `/api/v1`

## 0. Conventions

### Authentication & Context

All authenticated requests must pass:

```
Authorization: Bearer <JWT_TOKEN>
```

The token carries `sub` (user id) and `role`. The server re-reads `users.is_active`; a deactivated user is rejected even with a valid token.

### Response envelope

Single resource:

```json
{ "data": { }, "meta": null }
```

List resource:

```json
{
  "data": [],
  "meta": { "page": 1, "per_page": 20, "total": 137 }
}
```

### Error envelope

```json
{
  "error": {
    "code": "invalid_state_transition",
    "message": "Order is in 'under_review' and cannot be assigned.",
    "details": { "current_status": "under_review" }
  }
}
```

| HTTP | `code` | When |
| --- | --- | --- |
| 400 | `validation_error` | Malformed body, missing required field |
| 401 | `unauthorized` | Missing/expired/invalid token |
| 402 | `insufficient_funds` | Wallet balance below the order total |
| 402 | `insufficient_credits` | AI credits below the job cost |
| 403 | `forbidden` | Role or ownership check failed |
| 404 | `not_found` | Resource does not exist or is not visible to caller |
| 409 | `invalid_state_transition` | Action illegal for current status |
| 409 | `revision_limit_reached` | `revisions_used >= revision_limit` |
| 409 | `email_taken` | Register with existing email |
| 410 | `entitlement_expired` | Entitlement exists but `expires_at` passed |
| 410 | `downloads_exhausted` | `download_count >= max_downloads` |
| 413 | `payload_too_large` | Upload exceeds the allowed size |
| 415 | `unsupported_media_type` | Upload type is not allow-listed |
| 422 | `brief_incomplete` | Brief payload fails the service template |
| 429 | `rate_limited` | Throttled |
| 502 | `generation_failed` | Renderer/LLM failed; credits refunded |
| 500 | `internal_error` | Unexpected failure; carries `request_id` |

All list endpoints accept `?page=1&per_page=20` (`per_page` max 100).

---

## 1. Authentication

### `POST /auth/register`

Public. Creates a `client`.

```json
{ "email": "a@b.com", "password": "min-12-chars", "full_name": "A B" }
```

`201 Created`

```json
{
  "data": {
    "id": "uuid",
    "email": "a@b.com",
    "full_name": "A B",
    "role": "client",
    "wallet_balance": 0.00
  },
  "meta": null
}
```

Errors: `409 email_taken`, `400 validation_error`.

### `POST /auth/login`

Public.

```json
{ "email": "a@b.com", "password": "..." }
```

`200 OK`

```json
{
  "data": {
    "token": "eyJ...",
    "expires_in": 3600,
    "user": { "id": "uuid", "email": "a@b.com", "role": "client" }
  },
  "meta": null
}
```

Errors: `401 unauthorized` (wrong credentials — same response as unknown email, no user enumeration).

### `GET /auth/me`

Authenticated.

```json
{
  "data": {
    "id": "uuid",
    "email": "a@b.com",
    "full_name": "A B",
    "role": "client",
    "wallet_balance": 42.50,
    "is_active": true,
    "created_at": "2026-01-01T00:00:00Z"
  },
  "meta": null
}
```

---

## 2. Digital Products

### `GET /digital/products`

Public. Only `is_active` rows.

```json
{
  "data": [
    {
      "id": "uuid",
      "title": "Notion Template Pack",
      "slug": "notion-template-pack",
      "product_type": "premade_file",
      "price": 19.00,
      "thumbnail_url": "https://...",
      "current_version": "1.2.0",
      "owned": false
    }
  ],
  "meta": { "page": 1, "per_page": 20, "total": 12 }
}
```

`owned` is filled when the caller is authenticated, otherwise `false`.

### `GET /digital/products/{slug}`

Public. Full description plus file metadata (size, mime, version). **Never** includes `storage_path` or any URL that grants access.

### `GET /digital/my-vault`

Authenticated (`client`, `staff`, `admin` — returns own rows).

```json
{
  "data": [
    {
      "entitlement_id": "uuid",
      "product": { "id": "uuid", "title": "...", "slug": "..." },
      "version": "1.2.0",
      "download_count": 1,
      "max_downloads": -1,
      "expires_at": null,
      "created_at": "2026-01-01T00:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 20, "total": 3 }
}
```

### `POST /digital/download/{entitlement_id}`

Authenticated. Caller must own the entitlement.

Validates ownership → active status → expiry → max downloads, atomically increments `download_count`, signs a 15-minute S3/R2 URL, logs IP and user agent asynchronously.

`200 OK`

```json
{
  "data": {
    "download_url": "https://storage.example.com/protected-products/...?X-Amz-Signature=...",
    "expires_at": "2026-01-01T00:15:00Z",
    "filename": "notion-template-pack-1.2.0.zip"
  },
  "meta": null
}
```

Errors: `403 forbidden` (not owner), `410 entitlement_expired`, `410 downloads_exhausted`, `404 not_found`.

Rules:
- The response must never contain `storage_path`.
- The URL is `Content-Disposition: attachment` and TTL 900 s.
- The audit row is written even if the client disconnects after receiving the URL.

---

## 3. Checkout (Digital)

### `POST /digital/checkout`

Authenticated. Creates an `orders` row.

```json
{
  "items": [{ "product_id": "uuid", "quantity": 1 }],
  "payment_provider": "wallet",
  "idempotency_key": "client-generated-uuid"
}
```

`201 Created` — order `pending`, or immediately `paid` when paid by wallet.

```json
{
  "data": {
    "order_id": "uuid",
    "order_number": "DIG-20260101-0001",
    "status": "paid",
    "total_amount": 19.00,
    "entitlements_created": 1
  },
  "meta": null
}
```

Errors: `402 insufficient_funds` (wallet), `409 invalid_state_transition` (replaying a completed idempotency key with a different body), `422` for inactive products.

Idempotency: the same `idempotency_key` replays the original response and must not create a second entitlement.

---

## 4. Service Orders

### `POST /services/orders`

Authenticated (`client`). Creates an order with status `pending_brief` and debits the wallet (or creates a pending payment).

```json
{ "service_id": "uuid" }
```

`201 Created`

```json
{
  "data": {
    "id": "uuid",
    "order_number": "SVC-20260101-0007",
    "status": "pending_brief",
    "price_charged": 250.00,
    "revision_limit": 2,
    "revisions_used": 0,
    "due_at": null,
    "brief_required": true
  },
  "meta": null
}
```

Errors: `402 insufficient_funds`, `422` for inactive service.

### `POST /services/orders/{order_id}/brief`

Authenticated (`client`, owner only). Writes `order_briefs`, transitions `pending_brief → in_queue`.

```json
{
  "payload": {
    "brand_name": "Acme",
    "style": "minimal",
    "references": ["user-uploads/uuid/briefs/ref.png"]
  },
  "uploaded_asset_keys": ["user-uploads/uuid/briefs/ref.png"]
}
```

`200 OK` → `{ "data": { "status": "in_queue", "submitted_at": "..." } }`

Errors: `409 invalid_state_transition` (already submitted), `422 brief_incomplete` with a list of missing keys from `brief_template`.

### `PATCH /services/orders/{order_id}/assign`

`admin` | `staff` only.

```json
{ "assigned_staff_id": "uuid", "due_at": "2026-01-15T18:00:00Z" }
```

Allowed from `in_queue` (→ `in_progress`) and from `in_progress` (reassignment, status unchanged).

`200 OK` → `{ "data": { "status": "in_progress", "assigned_staff_id": "uuid", "due_at": "..." } }`

Errors: `409 invalid_state_transition`, `422` if `due_at` is in the past or the user is not `staff`.

### `POST /services/orders/{order_id}/deliverable`

`staff` (assigned) or `admin`. Allowed from `in_progress` and `revision_requested`.

```json
{
  "final_asset_key": "service-deliverables/uuid/final/v1.zip",
  "preview_asset_key": "service-deliverables/uuid/raw/v1.png",
  "notes": "First pass, brand palette applied."
}
```

Server enqueues watermarking for `preview_asset_key`. The order transitions to `under_review` **only after** the watermarked preview exists; until then it stays in `in_progress` and the client sees nothing.

`202 Accepted`

```json
{ "data": { "deliverable_id": "uuid", "version": 1, "status": "in_progress", "preview_status": "processing" } }
```

Errors: `409 invalid_state_transition`, `403 forbidden` (not the assigned staff).

### `POST /services/orders/{order_id}/revision`

`client` (owner only). Allowed from `under_review`.

```json
{ "deliverable_id": "uuid", "feedback": "Please make the logo larger." }
```

Increments `revisions_used`, writes `order_revisions`, transitions to `revision_requested`.

`200 OK` → `{ "data": { "status": "revision_requested", "revisions_used": 1, "revision_limit": 2 } }`

Errors: `409 revision_limit_reached` (client must then approve or contact support), `409 invalid_state_transition`.

### `POST /services/orders/{order_id}/approve`

`client` (owner only). Allowed from `under_review`.

Marks the deliverable `is_approved = true`, sets `approved_at`, transitions to `completed`, and unlocks the unwatermarked final asset for this order.

`200 OK`

```json
{
  "data": {
    "status": "completed",
    "approved_deliverable_id": "uuid",
    "download_available": true
  },
  "meta": null
}
```

### `GET /services/orders/{order_id}/deliverable/{deliverable_id}/download`

Client-owner, and only when `service_orders.status = 'completed'` **and** the deliverable is approved. Returns a short-lived signed URL for the file under `.../final/`.

Errors: `403 forbidden` while status is `under_review` or `revision_requested`, `410` if the asset is missing from storage.

### `POST /services/orders/{order_id}/cancel`

`admin` only, or `client` while status is `pending_brief` or `in_queue`.

```json
{ "reason": "No longer needed" }
```

Sets `cancelled`, writes `cancelled_reason`, refunds to the wallet ledger in the same transaction.

`200 OK` → `{ "data": { "status": "cancelled", "refunded": 250.00 } }`

### `GET /services/orders`

Authenticated. Role-scoped automatically:

| Role | Returns |
| --- | --- |
| `client` | own orders |
| `staff` | orders assigned to them |
| `admin` | all orders |

Query params: `?status=in_progress&assignee=me&page=1&per_page=20`.

### `GET /services/orders/{order_id}`

Authenticated, scoped as above. Includes brief, deliverables (previews only), revision history, and allowed next actions:

```json
{
  "data": {
    "id": "uuid",
    "order_number": "SVC-...",
    "status": "under_review",
    "allowed_actions": ["revision", "approve"],
    "deliverables": [
      { "id": "uuid", "version": 1, "preview_url": "signed...", "notes": "...", "is_approved": false }
    ],
    "revisions_used": 0,
    "revision_limit": 2
  },
  "meta": null
}
```

`allowed_actions` is the machine-readable contract the frontend uses to enable buttons — do not recompute state rules in the client.

---

## 5. Rate Limits

| Scope | Limit |
| --- | --- |
| Login | 10 / minute / IP |
| Register | 5 / hour / IP |
| Download | 30 / hour / user |
| Everything else | 300 / minute / user |

---

## 6. Uploads (Pre-signed PUT)

### `POST /uploads/presign`

Authenticated. Issues a short-lived upload URL for client brief assets.

```json
{
  "purpose": "brief_asset",
  "filename": "logo.png",
  "content_type": "image/png",
  "size_bytes": 482113
}
```

`200 OK`

```json
{
  "data": {
    "upload_url": "https://storage.example.com/user-uploads/...?X-Amz-Signature=...",
    "key": "user-uploads/{user_id}/briefs/{uuid}.png",
    "expires_at": "2026-01-01T00:05:00Z",
    "max_size_bytes": 10485760
  },
  "meta": null
}
```

Rules:

- Allow-list by `purpose`: `brief_asset` → png/jpg/webp/pdf, ≤ 10 MB;
  `generator_input` → png only, ≤ 10 MB.
- The key is generated server-side under the caller's own prefix. A client
  cannot request another user's prefix.
- TTL is 300 s.
- Errors: `413 payload_too_large`, `415 unsupported_media_type`.

The client then `PUT`s the bytes directly to `upload_url`. The server verifies
object existence and size before accepting the key in a brief.

---

## 7. AI Generator

Full pipeline: `docs/AI_GENERATOR_PIPELINE.md`.

### `POST /digital/generate`

Authenticated (`client`, `admin`).

```json
{
  "tool": "promo_banner",
  "idempotency_key": "client-uuid",
  "params": {
    "product_image_key": "user-uploads/{user_id}/briefs/{uuid}.png",
    "discount_percent": 20,
    "brand_color": "#6366F1",
    "aspect": "1:1"
  }
}
```

`202 Accepted`

```json
{
  "data": {
    "job_id": "uuid",
    "status": "queued",
    "credits_charged": 2,
    "credits_remaining": 18,
    "stream_url": "/api/v1/digital/generate/{job_id}/stream"
  },
  "meta": null
}
```

Errors: `402 insufficient_credits`, `422 validation_error`,
`429 rate_limited` (more than 2 active jobs).

Replaying the same `idempotency_key` returns the original job with `200` and
charges nothing.

### `GET /digital/generate/{job_id}`

Owner or admin.

```json
{
  "data": {
    "job_id": "uuid",
    "tool": "promo_banner",
    "status": "succeeded",
    "output_url": "https://storage.example.com/...?X-Amz-Signature=...",
    "output_mime": "image/png",
    "credits_charged": 2,
    "created_at": "2026-01-01T00:00:00Z"
  },
  "meta": null
}
```

`output_url` is a 900 s signed URL. `output_path` is never returned.

### `GET /digital/generate/{job_id}/stream`

Server-Sent Events. Events: `meta`, `token`, `done`, `error`.
See `AI_GENERATOR_PIPELINE.md` §4 for the wire format.

Authentication uses a short-lived `?ticket=` issued alongside `stream_url`,
because `EventSource` cannot send headers.

---

## 8. Payment Webhooks

### `POST /webhooks/payment/{provider}`

Public endpoint, authenticated by signature only.

`provider` ∈ `bkash` | `sslcommerz` | `stripe`.

- Verify the signature over the **raw body**.
- Idempotent on `(provider, event_id)`; a duplicate returns `200` with no
  side effect.
- Applies the wallet credit and marks the webhook processed in one transaction.

Responses:

| Case | Status |
| --- | --- |
| Accepted and processed | `200` |
| Already processed | `200` |
| Invalid signature | `400` |
| Unknown provider | `404` |
| Internal failure (gateway should retry) | `500` |

Never returns wallet details.

---

## 9. Finance

### `GET /finance/wallet`

```json
{
  "data": {
    "currency": "USD",
    "wallet_balance": "42.50",
    "ai_credits": 18,
    "pending_topups": 0
  },
  "meta": null
}
```

Amounts are decimal strings, never floats.

### `POST /finance/topup`

```json
{ "amount": "50.00", "currency": "USD", "provider": "stripe" }
```

`201 Created`

```json
{
  "data": {
    "topup_id": "uuid",
    "status": "pending",
    "redirect_url": "https://checkout.stripe.com/...",
    "expires_at": "2026-01-01T00:15:00Z"
  },
  "meta": null
}
```

For `provider: "wallet"` this is invalid — use the internal credit path.

### `GET /finance/invoices`

Paginated. `GET /finance/invoices/{id}` returns the invoice with a signed
`pdf_url` (900 s).

```json
{
  "data": [
    {
      "id": "uuid",
      "invoice_number": "INV-202601-000001",
      "currency": "USD",
      "subtotal": "19.00",
      "tax_amount": "0.00",
      "total": "19.00",
      "status": "paid",
      "issued_at": "2026-01-01T00:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 20, "total": 4 }
}
```

### `GET /finance/transactions`

Paginated wallet ledger for the caller: `txn_type`, `amount`, `balance_after`,
`reference_type`, `created_at`. Never exposes `gateway_ref`.

---

## 10. Notifications

### `GET /notifications`

```json
{
  "data": [
    {
      "id": "uuid",
      "kind": "order_status",
      "title": "Deliverable ready for review",
      "body": "SVC-20260101-0007",
      "reference_type": "service_order",
      "reference_id": "uuid",
      "read_at": null,
      "created_at": "2026-01-01T00:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 20, "total": 3, "unread": 2 }
}
```

### `PATCH /notifications/read`

```json
{ "ids": ["uuid"], "all": false }
```

Returns `{ "data": { "updated": 1 } }`.

---

## 11. Settings & Team

Single workspace in v1; there is no workspace id in any path.

### `GET /settings/profile` · `PATCH /settings/profile`

Fields: `name`, `default_currency`, `locale`, `timezone`, `logo_key`
(uploaded via §6, signed on read).

### `GET /settings/team`

```json
{
  "data": [
    { "id": "uuid", "email": "a@b.com", "full_name": "A B", "role": "staff", "is_active": true, "last_seen_at": "..." }
  ],
  "meta": { "page": 1, "per_page": 20, "total": 3 }
}
```

### `POST /settings/team/invite`

Admin only. `{ "email": "...", "role": "staff" }`. Returns `201` with the
invite id; `409 email_taken` when the user already exists.

### `PATCH /settings/team/{user_id}`

Admin only. `{ "role": "staff" | "admin", "is_active": true }`.
An admin cannot demote or deactivate themselves — `409 invalid_state_transition`.

### `GET /settings/api-keys` · `POST /settings/api-keys`

The secret is returned **once**, on creation, and never again:

```json
{ "data": { "id": "uuid", "name": "CI", "prefix": "erp_live_ab12", "secret": "erp_live_ab12_…", "created_at": "..." } }
```

### `DELETE /settings/api-keys/{id}`

Revokes immediately. Existing keys are listed with `last_used_at`.

---

## 12. Rate Limits (extended)

| Scope | Limit |
| --- | --- |
| `POST /digital/generate` | 10 / hour / user, 2 concurrent jobs |
| Uploads | 50 / hour / user |
| Webhooks | 600 / minute / provider (gateway bursts) |
| Team invites | 20 / hour / workspace |
