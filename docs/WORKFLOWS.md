# System Workflows & State Machines

Two independent lifecycles share this platform: **service order fulfilment** (human) and **digital asset delivery** (automated). They never share state, only identity, wallet, and audit.

---

## 1. Service Order State Machine

```
                          ┌──────────────┐
                          │ pending_brief│
                          └──────┬───────┘
                 cancel ◄────────┤ client submits brief
                                 ▼
                          ┌──────────────┐
                          │   in_queue   │
                          └──────┬───────┘
                 cancel ◄────────┤ staff assigned + due_at set
                                 ▼
                          ┌──────────────┐
        reassign ────────►│  in_progress │◄──────────────────┐
        (no state change)└──────┬───────┘                   │
                                 │ deliverable uploaded     │ staff resumes
                                 │ + watermark ready        │
                                 ▼                          │
                          ┌──────────────┐                  │
                          │ under_review │                  │
                          └──┬────────┬──┘                  │
                             │        │                     │
              approve        │        │ request revision    │
                             ▼        ▼                     │
                     ┌───────────┐ ┌────────────────────┐   │
                     │ completed │ │ revision_requested ├───┘
                     └───────────┘ └────────────────────┘
```

`cancelled` is terminal and reachable from `pending_brief` and `in_queue` (client or admin) and from any non-completed state by admin only.

### 1.1 Transition Table

| From | Action | Actor | Guard | To | Side effects |
| --- | --- | --- | --- | --- | --- |
| `pending_brief` | submit brief | client (owner) | payload matches `brief_template` | `in_queue` | insert `order_briefs` |
| `pending_brief` | cancel | client/admin | none | `cancelled` | wallet refund + ledger row |
| `in_queue` | assign | admin/staff | assignee role = `staff`, `due_at` future | `in_progress` | set `assigned_staff_id`, `due_at` |
| `in_queue` | cancel | client/admin | none | `cancelled` | wallet refund + ledger row |
| `in_progress` | reassign | admin | assignee role = `staff` | `in_progress` | overwrite `assigned_staff_id` |
| `in_progress` | upload deliverable | assigned staff/admin | asset uploaded, version = max+1 | `under_review`* | insert `order_deliverables`, enqueue watermark |
| `revision_requested` | upload deliverable | assigned staff/admin | version = max+1 | `under_review`* | insert new deliverable row |
| `under_review` | request revision | client (owner) | `revisions_used < revision_limit` | `revision_requested` | `revisions_used += 1`, insert `order_revisions` |
| `under_review` | approve | client (owner) | none | `completed` | `is_approved = true`, `approved_at`, unlock final asset |
| any non-terminal | cancel | admin | none | `cancelled` | refund per policy, revoke access |

\* The transition to `under_review` happens **only after** the watermarked preview is committed. The deliverable row is inserted first with the order still in `in_progress`; a worker callback performs the transition. This prevents a client from seeing a `under_review` order with no preview.

### 1.2 Transition Invariants

- An order cannot move out of `pending_brief` until an `order_briefs` record exists.
- An order cannot transition to `under_review` without at least one linked `order_deliverables` row **that has a ready watermarked preview**.
- `final_asset_path` cannot be requested or exposed via a signed URL while the status is `under_review` or `revision_requested`.
- `revisions_used` never exceeds `revision_limit`. Once the limit is reached, the only client actions are `approve` or support escalation.
- Every transition writes an `updated_at` bump and an audit entry (actor, from, to, reason).
- Illegal transitions return `409 invalid_state_transition` with the current status — never a silent no-op.

### 1.3 Revision Exhaustion

When `revisions_used == revision_limit`, the client still sees the proof. `POST /revision` returns `409 revision_limit_reached`. The client must either approve or open a support conversation. Admin can raise `revision_limit` explicitly, which is logged.

---

## 2. Digital Asset Delivery Workflow

### 2.1 Grant

1. User completes checkout, or an admin grants a licence.
2. Inside one transaction:
   - `orders.status` → `paid` (or an admin-grant audit row is written)
   - one `user_entitlements` row per purchased product, with `order_ref = order_number`
   - wallet debit + `wallet_transactions` row when paid by wallet
3. The transaction commits. Any failure rolls back all three.
4. An idempotency key protects against duplicate grants on client retry.

### 2.2 Download

1. User opens **My Assets** in the SPA or Tauri app (`GET /digital/my-vault`).
2. User clicks **Download** (`POST /digital/download/{entitlement_id}`).
3. Backend, in order:
   - verifies entitlement ownership (`user_id` matches the JWT subject)
   - atomically increments `download_count` and validates expiry/limit in one `UPDATE ... RETURNING`
   - resolves the current `product_files` row for the entitlement version
   - signs an S3/R2 URL with 900 s TTL and `Content-Disposition: attachment`
   - enqueues an audit log write (IP, user agent)
4. Response returns only `download_url`, `expires_at`, `filename`.
5. Client downloads directly from storage. The API is never in the byte path.

### 2.3 Denial Paths

| Condition | HTTP | `code` |
| --- | --- | --- |
| Entitlement belongs to another user | 403 | `forbidden` |
| `expires_at < NOW()` | 410 | `entitlement_expired` |
| `max_downloads != -1 AND download_count >= max_downloads` | 410 | `downloads_exhausted` |
| Product or file deleted | 404 | `not_found` |

A denied request must **not** increment `download_count`.

### 2.4 New Version Release

When a new `product_files` version is published, existing entitlements remain valid and resolve to the newest current version at download time. The vault response shows the version that will actually be served.

---

## 3. Background Worker Workflows

### 3.1 Watermark Pipeline (service proofs)

```
raw upload ──► job: watermark ──► write previews/ ──► update deliverable ──► transition to under_review
                    │
                    └── on failure (max 3 attempts) ──► park order in in_progress, alert staff
```

- The watermark text is `PROOFS ONLY - [ORDER_NUMBER]`, semi-transparent, placed so it cannot be cropped out (tiled or corner-anchored per media type).
- Files live under `raw/` until processed; the original is never moved to `previews/`.
- A failed job must never make the unwatermarked original reachable to the client.
- Video: transcode to a review stream, watermark burned in, never overlay-only CSS that can be hidden.

### 3.2 Signed URL Issuance

- Signing happens in the API process for downloads (low latency) and in workers for batch/export jobs.
- TTL is always 900 s. No long-lived URLs are ever stored or emailed.
- Signed URLs are never cached, logged in full, or included in list responses.

### 3.3 AI Copy Streaming

- Streaming state is held in Redis, keyed by request id, with a TTL longer than the max generation time.
- The client reconnects using the same id and resumes from the last token offset.
- Output is persisted only after the stream completes successfully.

---

## 4. SLA & Queue Rules

- `in_queue` → `in_progress` should happen within one business day.
- `due_at` is advisory for staff, hard for admin dashboards; overdue orders surface first on the board.
- `under_review` has no SLA on the client, but reminders fire at 3 and 7 days of inactivity.
- Board columns map 1:1 to `service_status`; the board query uses `idx_service_orders_board`.
