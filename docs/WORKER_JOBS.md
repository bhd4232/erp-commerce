# Background Worker & Queue Specification

Workers share the service layer with the API. A job is a pointer to a database
row, never a copy of its state.

---

## 1. Queues (Redis)

| Queue | Purpose | Concurrency | Max attempts |
| --- | --- | ---: | ---: |
| `media.watermark` | burn proof watermark, build preview | 2 | 3 |
| `ai.render` | banner / packing slip rendering | 2 | 2 |
| `ai.stream` | ad-copy streaming | 4 | 1 (no retry) |
| `mail.send` | transactional email | 4 | 5 |
| `notify.push` | system / mobile notifications | 4 | 3 |
| `storage.sweep` | orphan objects, expired outputs | 1 | 1 (cron) |
| `billing.reconcile` | ledger vs cached balance | 1 | 1 (cron) |

Job envelope:

```json
{
  "id": "uuid",
  "queue": "media.watermark",
  "attempt": 1,
  "enqueued_at": "2026-11-08T10:00:00Z",
  "payload": { "deliverable_id": "uuid" }
}
```

Payloads carry **ids only**. A worker re-reads the row, so a retry always acts
on current state.

---

## 2. Job Contracts

### 2.1 `media.watermark`

```json
{ "deliverable_id": "uuid" }
```

1. Load the deliverable; if `watermarked_preview_path` is already set, exit
   (idempotent).
2. Read `service-deliverables/{order_id}/raw/{file}`.
3. Burn `PROOFS ONLY - {order_number}` (tiled / corner-anchored per media).
4. Write `.../previews/{file}`.
5. In one transaction: update the deliverable, and if the order is
   `in_progress` or `revision_requested`, move it to `under_review`.
6. Emit a notification to the client.

A failure after 3 attempts leaves the order in `in_progress` and alerts staff.
The unwatermarked file is never exposed.

### 2.2 `ai.render`

```json
{ "job_id": "uuid" }
```

Credit handling is owned by the job row (see `AI_GENERATOR_PIPELINE.md` §3).
On terminal failure the worker writes the status and the refund in one
transaction.

### 2.3 `storage.sweep` (cron, hourly)

- Objects older than 24 h with no DB row → delete.
- Generated outputs older than 30 days with no entitlement → delete.
- Never touch `protected-products/` or `final/`.

### 2.4 `billing.reconcile` (cron, daily)

Implements `BILLING_WALLET_SPEC.md` §8. Divergence freezes the wallet and
alerts; it never auto-corrects.

---

## 3. Retry & Backoff

| Attempt | Delay |
| ---: | --- |
| 1 | immediate |
| 2 | 30 s |
| 3 | 5 min |
| 4+ | dead-letter |

- Backoff is exponential with jitter; the table is the minimum.
- A retry must be safe: every handler starts with an idempotency check.
- After the last attempt the job moves to `*.dead` with the last error. Dead
  jobs are visible in the admin dashboard and replayable manually.

---

## 4. Leader Election & Cron

- Only one instance runs cron jobs; use a Redis lock
  (`SET cron:{name} <instance> NX PX 60000`) refreshed while running.
- A missed cron window is skipped, not back-filled.

---

## 5. Graceful Shutdown

1. Stop accepting new jobs.
2. Finish in-flight jobs for up to 30 s.
3. Push unfinished jobs back to the queue unchanged.
4. Release locks, then exit.

SIGTERM handling is required; SIGKILL must not corrupt a job.

---

## 6. Observability

Per job: `job_id`, `queue`, `attempt`, `duration_ms`, `outcome`, `request_id`.
Never log payload contents, file contents, prompts, or signed URLs.

Metrics: queue depth, oldest job age, failure rate per queue, dead-letter count.
Alert when `oldest_job_age > 5 min` for interactive queues (`media.watermark`,
`ai.render`).
