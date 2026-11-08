# E-commerce Digital Asset & AI Generator Pipeline

Implements `POST /api/v1/digital/generate` (see `API_SPEC.md` §7). Every job is
a row in `generation_jobs` (`0005_generation_jobs.sql`); credits are handled by
`docs/BILLING_WALLET_SPEC.md`.

---

## 1. Tools

| Tool | Input | Output | Credits | Latency target |
| --- | --- | --- | ---: | --- |
| `promo_banner` | transparent product PNG, discount %, brand colour, aspect (1:1, 9:16) | PNG / WebP | 2 | p95 ≤ 8 s |
| `ad_copy` | product features, tone, platform (facebook / tiktok) | streamed text | 1 | first token ≤ 1.5 s |
| `packing_slip` | order id, brand block, QR payload | print-ready PDF (A4 / A6) | 1 | p95 ≤ 3 s |

Credit costs live in config (`GENERATION_COST_PROMO_BANNER`, …), not in code
branches. Changing a price is a config change, not a deploy of new logic.

---

## 2. Request → Job Lifecycle

```
POST /digital/generate
  │
  ├─ 1. validate params against the tool schema        → 422 validation_error
  ├─ 2. idempotency check (user_id, idempotency_key)   → 200 replay existing job
  ├─ 3. transaction:
  │       debit ai_credits (atomic UPDATE ... RETURNING)
  │       insert ai_credit_transactions (type = 'spend')
  │       insert generation_jobs (status = 'queued')
  │     → 402 insufficient_credits on zero rows
  ├─ 4. enqueue worker job (Redis) OR stream inline for ad_copy
  └─ 5. respond 202 with job_id + stream_url
```

```json
{
  "data": {
    "job_id": "uuid",
    "status": "queued",
    "credits_charged": 2,
    "credits_remaining": 18,
    "stream_url": "/api/v1/digital/generate/{job_id}/stream"
  }
}
```

### 2.1 Guarantees

- The credit debit and the job insert happen in **one** transaction. A failed
  enqueue rolls both back; the user is never charged for a job that does not
  exist.
- If the enqueue succeeds but the worker never runs, the job stays `queued`;
  the reaper moves it to `failed` after 10 minutes and refunds credits.
- Retrying with the same `idempotency_key` returns the original job and does
  not charge again.

---

## 3. Worker Execution

Queue names are defined in `docs/WORKER_JOBS.md` (`ai.render`, `ai.stream`).

```
queued → running → succeeded
                 └→ failed  → refund credits (same transaction as status write)
running → cancelled (client) → refund credits
```

Render path (`promo_banner`, `packing_slip`):

1. Fetch inputs from `user-uploads/{user_id}/briefs/` (or job params).
2. Render with Rust `image` / `resvg` for rasters, `printpdf` for documents.
3. Write output to `user-uploads/{user_id}/generated/{job_id}.{png|webp|pdf}`.
4. Update `generation_jobs` with `output_path`, `output_mime`, `status`.
5. Create a `user_entitlements` row when the output is a sellable product;
   generated one-offs are reachable only through the job endpoint.
6. Publish a completion event for the SSE stream and push a notification.

Stream path (`ad_copy`):

1. Call the LLM with a streaming client.
2. Persist tokens to Redis under `gen:{job_id}` with a 1 h TTL so a reconnect
   can resume from an offset.
3. On completion, store the final text in `generation_jobs.output_path`
   (as a storage object) or in `params`-adjacent JSON — never in a log.
4. On client disconnect, keep generating; the job is not cancelled implicitly.

---

## 4. SSE Contract

`GET /digital/generate/{job_id}/stream`

```
event: meta        data: {"job_id":"…","tool":"ad_copy","credits_charged":1}
event: token       data: {"i":0,"t":"Your "}
event: token       data: {"i":1,"t":"product "}
event: done        data: {"output_url":"…","expires_at":"…"}
event: error       data: {"code":"generation_failed","message":"…"}
```

Rules:

- `Content-Type: text/event-stream`, `Cache-Control: no-store`.
- Heartbeat comment (`: ping`) every 15 s so proxies do not idle-close.
- Reconnect with `Last-Event-ID`; the server replays from the stored offset.
- The terminal event is exactly one of `done` or `error`.
- The stream never carries a storage key — only the signed `output_url`.
- Auth: the stream requires the same JWT. EventSource cannot set headers, so
  use a short-lived query token (`?ticket=…`) issued by `POST /generate`.

---

## 5. Storage Layout

```
user-uploads/
└── {user_id}/
    ├── briefs/       ← client-supplied brand assets
    └── generated/    ← AI outputs, per-job, private
```

Permissions, TTL, and signing rules are in `STORAGE_SECURITY.md` §2. Outputs are
private to their owner; `generated/` is never browsable.

---

## 6. Credit Failure Handling

| Case | Action |
| --- | --- |
| Insufficient credits at submit | `402 insufficient_credits`, no job row |
| Enqueue failure | Transaction rolls back, credits untouched |
| Worker crash mid-render | Reaper marks `failed`, writes `refund` ledger row |
| Partial output written | Delete the object, then refund |
| Client cancels before start | `cancelled` + full refund |
| Client cancels mid-render | Best effort stop, refund pro-rata per policy |

Refunds are `ai_credit_transactions.txn_type = 'refund'` with
`reference_type = 'generation_job'` and `reference_id = job.id`.

---

## 7. Moderation & Abuse

- Uploads are validated for type and size before signing (PNG ≤ 10 MB).
- Rendering runs with a hard timeout (30 s) and a memory cap; a job that
  exceeds it is failed and refunded.
- Prompt text is length-capped (2,000 chars) and stripped of control characters.
- Per-user concurrency limit: 2 active jobs. Beyond that → `429 rate_limited`.
- Outputs older than 30 days are pruned unless attached to a purchased product.

---

## 8. Observability

Per job emit: `job_id`, `tool`, `status`, `duration_ms`, `credits_charged`,
`queue_wait_ms`, `request_id`. Never emit prompt text, generated copy, or
signed URLs.
