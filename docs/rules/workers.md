# Rule: Workers and queues

**Tier:** 1 for money/idempotency; 2 for queue-contract tests.

- The worker shares the service layer with the API; a job is a pointer to a
  database row, never a copy of its state.
- Queues: `media.watermark`, `ai.render`, `ai.stream`, `mail.send`,
  `notify.push`, `storage.sweep`, `billing.reconcile`, plus the COD queues
  (`connector.sync`, `webhook.process`, `confirmation.send`, `automation.run`,
  `courier.reconcile`).
- Payloads carry ids only. A worker re-reads the row so retries act on current
  state.
- Every handler starts with an idempotency check. A retry must be safe.
- Retry with exponential backoff; after the maximum, move to `*.dead` with the
  last error. Dead jobs are visible and replayable.
- A failed watermark job must never expose the raw original.
- A money/credit job writes the state change and the ledger row in one
  transaction; terminal failure writes exactly one refund.
- Only one instance runs a cron job; use a Redis lock. A missed window is
  skipped, not back-filled.
- Graceful shutdown: stop accepting, finish up to 30s, push unfinished jobs
  back unchanged, release locks.
- Never log payload contents, prompts, file contents or signed URLs.
