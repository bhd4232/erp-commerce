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
| 403 | `forbidden` | Role or ownership check failed |
| 404 | `not_found` | Resource does not exist or is not visible to caller |
| 409 | `invalid_state_transition` | Action illegal for current status |
| 409 | `revision_limit_reached` | `revisions_used >= revision_limit` |
| 409 | `email_taken` | Register with existing email |
| 410 | `entitlement_expired` | Entitlement exists but `expires_at` passed |
| 410 | `downloads_exhausted` | `download_count >= max_downloads` |
| 422 | `brief_incomplete` | Brief payload fails the service template |
| 429 | `rate_limited` | Throttled |
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
