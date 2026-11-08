# Phased Delivery Plan

Derived from the actual repository state, not from ambition. A phase is done
only when its **exit criteria** are true and CI is green. Do not start a phase
before the previous one's exit criteria hold, except where marked parallel.

Status legend: `[x]` done · `[ ]` not started · `[~]` specified/stubbed only.

---

## P0 — Specification & Platform Skeleton `[x]`

**Goal:** every contract exists before behaviour; the stack boots.

| Deliverable | State |
| --- | --- |
| `docs/` specification set (14 files) | done |
| Migrations `0001`–`0005` | done |
| `AppError` → HTTP envelope, auth extractors | done |
| Register / login / `me` | done |
| Private storage with pre-signed URLs | done |
| `/healthz`, Dockerfile, compose stack, CI | done |
| `AGENTS.md` + anti-bloat rules | done |

**Exit criteria**

- [x] `docker compose up -d` yields Postgres, Redis, MinIO, private bucket
- [x] `sqlx migrate run` applies 0001–0005 from an empty database
- [x] CI runs fmt, clippy, migrations, tests
- [ ] First real `cargo test` run on a machine with a toolchain

> Local note: this workspace has no Rust toolchain, so the last box stays open
> until CI or a dev machine runs it.

---

## P1 — Digital Commerce Core

**Goal:** a client can buy a digital product and download it once, safely.

| Deliverable | Notes |
| --- | --- |
| `GET /digital/products`, `GET /digital/products/{id}` | published only, no storage keys |
| `POST /digital/checkout` | wallet debit + order + items + payment in one tx |
| `GET /digital/entitlements` (vault) | joins `user_entitlements` |
| `POST /digital/download/{entitlement_id}` | `UPDATE ... RETURNING` counter, then `presign_get` |
| `audit_download_logs` write | same tx as the counter |

**Exit criteria**

- [ ] Checkout with insufficient balance returns `402` and writes nothing
- [ ] Concurrent double-checkout cannot overspend (test with two connections)
- [ ] Download beyond `max_downloads` returns `410` and does not increment
- [ ] No DTO contains `storage_path`; test asserts it
- [ ] Integration tests cover success + 4xx for every endpoint

**Docs:** `API_SPEC.md` §2, `WORKFLOWS.md` §3, `DATABASE.md` §3.

---

## P2 — Agency Service Pipeline

**Goal:** brief → queue → progress → proof → revision → delivery.

| Deliverable | Notes |
| --- | --- |
| `POST /services/orders`, `GET /services/orders` | filters: status, assignee, service |
| `POST /services/orders/{id}/brief` | validate against `agency_services.brief_template` |
| `PATCH /services/orders/{id}/status` | `allowed_actions` + `SELECT ... FOR UPDATE` |
| `POST /services/orders/{id}/deliverables` | pre-signed upload, watermark job enqueued |
| `POST /services/orders/{id}/revisions` | budget check inside the lock |
| `POST /services/orders/{id}/approve` | moves to `completed`, unlocks `final/` |
| `allowed_actions` in every order DTO | single source of UI truth |

**Exit criteria**

- [ ] Every legal transition has a test; every illegal one returns `409`
- [ ] Revision limit cannot be exceeded under concurrent requests
- [ ] Client cannot read `raw/`; preview signed only in `under_review` /
      `revision_requested`; `final/` only after `completed`
- [ ] Watermark worker is idempotent on re-run

**Docs:** `WORKFLOWS.md` §4, `UI_USER_FLOWS.md` §4, `WORKER_JOBS.md` §2.1.

---

## P3 — Billing & Wallet

**Goal:** money in, money out, provable.

| Deliverable | Notes |
| --- | --- |
| `GET /finance/wallet`, `/finance/transactions` | decimal strings, never `gateway_ref` |
| `POST /finance/topup` | intent row + gateway redirect |
| `POST /webhooks/payment/{provider}` | raw-body signature, idempotent |
| `GET /finance/invoices`, `/{id}` | signed PDF, 900 s |
| `billing.reconcile` cron | freezes on drift, never auto-fixes |

**Exit criteria**

- [ ] Replayed webhook changes no balance (`200`, no ledger row)
- [ ] Duplicate `(gateway, gateway_ref)` is rejected by the DB, not app code
- [ ] Signature failure returns `400` and writes `status = 'rejected'`
- [ ] Ledger insert + cached balance move and roll back together (test)
- [ ] Reconciliation on seeded drift reports the user and does not repair it

**Docs:** `BILLING_WALLET_SPEC.md` §2–8, `DATABASE.md` §9.1–9.2.

**Parallel:** can run beside P2 — different modules, no shared tables.

---

## P4 — AI Generator

**Goal:** credits buy output, and a failure always refunds.

| Deliverable | Notes |
| --- | --- |
| `POST /digital/generate` | credit debit + job insert in one tx |
| `GET /digital/generate/{job_id}` | signed `output_url` only |
| `GET /digital/generate/{job_id}/stream` | SSE, ticket auth, resume |
| `ai.render` worker | render, store under `generated/` |
| Reaper | `queued` > 10 min → `failed` + refund |

**Exit criteria**

- [ ] Same `idempotency_key` returns the original job and charges once
- [ ] Insufficient credits → `402`, no job row
- [ ] Worker crash path produces exactly one `refund` row
- [ ] Stream emits exactly one terminal event; no key in any frame
- [ ] 3rd concurrent job → `429`

**Docs:** `AI_GENERATOR_PIPELINE.md` §2–6.

---

## P5 — Notifications, Settings, Team

**Goal:** the shell around the product.

- `GET /notifications`, `PATCH /notifications/read`
- `GET/PATCH /settings/profile`
- `GET /settings/team`, invite, role change (no self-demotion)
- API keys: secret shown once, `DELETE` revokes immediately

**Exit criteria:** unread count matches the partial index; revoked key fails
auth on the next request; role change is audited.

---

## P6 — Frontend SPA

**Goal:** all 13 sidebar screens, Bengali-first, dark and light.

Order inside the phase (each builds on the previous):

1. Tokens + theme + Anek Bangla wiring (`styles/main.css`, `@theme`)
2. `api/client.ts` + `auth.store` + router guards + `AppLayout`
3. Auth screens → dashboard → catalogue → vault
4. Pipeline: kanban, table, order hub, brief form
5. Generator studio (SSE)
6. Billing, top-up, invoices
7. Settings, team, API keys

**Exit criteria**

- [ ] Every screen has loading, empty, error, disabled states
- [ ] No hex colour in any component; no `tailwind.config.js`
- [ ] Buttons render from `allowed_actions`, never from a status compare
- [ ] `bn-BD` layout passes at 360 px, 768 px, 1440 px
- [ ] No signed URL enters a store or `localStorage`
- [ ] `npm run build` and `npm run lint` are clean

**Docs:** `FRONTEND_ARCHITECTURE.md`, `UI_SCREEN_SPECS.md`, `I18N.md`.

---

## P7 — Tauri Shell

**Goal:** a desktop/mobile build that downloads properly and opens deep links.

- Capabilities allow-list exactly as specified
- Native download → save dialog → progress → "open folder"
- `erp://` cold-start routing
- Keychain token storage (not `localStorage`)
- Read-only offline cache with visible "cached" marking

**Exit criteria:** `TAURI_NATIVE_SPEC.md` §9 checklist passes on one desktop
target and one mobile target.

---

## P8 — Hardening & Launch

- Rate limits from `API_SPEC.md` §12 enforced and tested
- Metrics: queue depth, oldest job age, payment failure rate, p95 latency
- `pg_stat_statements` review; board endpoints ≤ 50 ms p95
- Backup + point-in-time recovery rehearsal
- Secret rotation runbook; no secret in git history (verified)
- Load test: 50 concurrent checkout + download

**Exit criteria:** restore from backup verified; alerting fires on a synthetic
ledger drift; no open `P0` bug.

---

## Cross-Phase Rules

1. **One topic per commit**; a phase never mixes a refactor with a feature.
2. A new endpoint is done only when it appears in `API_SPEC.md`, has an
   integration test, and returns the documented error codes.
3. A new table is done only when it appears in `DATABASE.md` and a migration.
4. Never weaken a security guard to make a test pass.
5. If reality and these phases disagree, update this file in the same PR.
