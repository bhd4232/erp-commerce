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

- [ ] `docker compose up -d` yields Postgres, Redis, MinIO, private bucket
      — the image builds in CI, but the compose stack itself is unverified
      here (no container runtime). Run it once on a Docker host.
- [x] Migrations `0001`–`0006` apply cleanly, in order, each inside a single
      transaction (local PostgreSQL 16; `sqlx migrate run` in CI)
- [x] CI runs fmt, clippy, migrations, tests
- [x] First real `cargo test` run (Rust 1.99.1): 5 unit + 6 integration tests,
      clippy `-D warnings`, `cargo fmt --check` all clean
- [x] First green CI run (2026-11-09): backend 4m10s, Docker image 4m51s

> **Toolchain gate:** this workspace has no Rust toolchain, no container runtime,
> and `curl` aborts under the current emulation. Code written here is unverified.
> P0 exits on the **first green CI run**; an unavailable local toolchain is a
> risk to manage, not a reason to delay. Keep changes small and reviewable so a
> compile failure is cheap to localise.

---

## P1 — Digital Commerce Core

**Goal:** a client can buy a digital product and download it once, safely.

| Deliverable | Notes |
| --- | --- |
| `GET /digital/products`, `GET /digital/products/{id}` | published only, no storage keys |
| `POST /digital/checkout` | wallet debit + order + items + payment in one tx |
| `GET /digital/my-vault` | joins `user_entitlements`; expired grants excluded |
| `POST /digital/download/{entitlement_id}` | `UPDATE ... RETURNING` counter, then `presign_get` |
| `audit_download_logs` write | same tx as the counter |
| `POST /digital/orders/{id}/refund` | `refund` ledger row + entitlement revoke |
| `scripts/seed.sql` | seed-only catalogue **and** `seed_wallet()` fixture |

**Scope decision (v1):** product creation is **seed-only**. There is no admin
product CRUD; a new product is a `scripts/seed.sql` change. This avoids building
an admin surface before the catalogue itself is proven. Revisit in P8.

**Wallet funding:** P1 must be testable before P3 exists. So `scripts/seed.sql`
ships a `seed_wallet(user_id, amount)` helper that, in one transaction, updates
`users.wallet_balance` **and** inserts the matching
`wallet_transactions(txn_type = 'credit', reference_type = 'manual')` row.
Integration tests call it directly. No top-up API and no admin credit endpoint
is added in P1 — the only sanctioned funding path is the ledger-writing fixture.

**Exit criteria**

- [x] Checkout with insufficient balance returns `402` and writes nothing
- [x] Concurrent double-checkout cannot overspend (`tokio::join!`, one winner,
      loser's order row rolled back)
- [x] Download beyond `max_downloads` returns `410` and does not increment
- [x] No DTO contains `storage_path`; asserted in the download test
- [x] `seed_wallet` leaves `wallet_balance` equal to the derived ledger balance
- [x] Refund writes one `refund` row and expires every entitlement of the order
      in the same transaction; second refund returns `409`
- [x] Integration tests cover the success and 4xx paths
      (`tests/digital_flow.rs`, 6 cases against real PostgreSQL)

**Current state:** the backend for every bullet above is implemented and was
exercised against a real PostgreSQL 16 instance: checkout `201`, idempotent
replay without a second debit, vault listing, download `200` with a signed URL,
second download `410`, refund `200` / repeat `409` / after-download `409` /
non-admin `403`, zero balance `402`, and a ledger that reconciles exactly.
Pinned by `tests/digital_flow.rs`: 6 `#[sqlx::test]` cases (fresh database
and all migrations per test) covering 402, idempotent replay, ledger/balance
reconciliation, the 410 download limit, refund rules, and concurrent
double-checkout.

**Docs:** `API_SPEC.md` §2, `WORKFLOWS.md` §3, `DATABASE.md` §3.

---

## P1.5 — Optional Vertical Slice (UI proof) `[optional]`

**Goal:** see P1 in a browser before building P2, so backend work stays honest.

Do this only if P1 is green. It is a slice, not the frontend phase: the screens
here are revisited in P6 for polish, but their contracts do not change.

In scope:

1. Tailwind v4 `@theme`, dark/light tokens, Anek Bangla, `AppLayout`
2. `api/client.ts`, `auth.store`, router guards
3. Login / register, product catalogue, product detail, vault, download button

Explicitly **out of scope:** pipeline UI, generator studio, billing screens,
settings, Tauri. Those stay in P6/P7.

**Exit criteria**

- [ ] A real wallet-funded checkout is visible end to end in a browser
- [ ] Download button returns a working pre-signed URL and increments the counter
- [ ] `bn-BD` and `en-US` both render without clipping
- [ ] No hex colour outside `styles/main.css`

> If skipped, nothing else changes: P6 still owns the whole frontend.

---

## P2 — Agency Service Pipeline

**Goal:** brief → queue → progress → proof → revision → delivery.

| Deliverable | Notes |
| --- | --- |
| `POST /services/orders`, `GET /services/orders` | filters: status, assignee, service |
| `GET /services/orders/{id}` | order workspace: order, brief, deliverables, revisions, `allowed_actions` |
| `POST /uploads/presign` | brief asset PUT; also required by P6 brief form |
| `POST /services/orders/{id}/brief` | validate against `agency_services.brief_template` |
| `PATCH /services/orders/{id}/status` | `allowed_actions` + `SELECT ... FOR UPDATE` |
| `POST /services/orders/{id}/deliverables` | pre-signed upload, watermark job enqueued |
| `POST /services/orders/{id}/revisions` | budget check inside the lock |
| `POST /services/orders/{id}/approve` | moves to `completed`, unlocks `final/` |
| `allowed_actions` in every order DTO | single source of UI truth |

**Current state:** `POST /uploads/presign`, `POST /services/orders`,
`GET /services/orders` (role-scoped, `status` + `assignee=me` filters) and
`GET /services/orders/{id}` (`allowed_actions`, brief, previews) are
implemented and covered by `tests/service_flow.rs`. Order creation debits the
wallet and writes the ledger row in one transaction. Remaining in this phase:
brief submission, assignment, deliverable upload + watermark enqueue,
revision, approve, cancel/refund, and the final download gate.

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

**Current state:** `GET /finance/wallet`, `GET /finance/transactions`,
`POST /finance/topup`, `GET /finance/invoices`, `GET /finance/invoices/{id}`
and `POST /webhooks/payment/{provider}` are implemented. Webhooks verify a
Stripe-style HMAC (or a shared secret for bKash/SSLCommerz), are idempotent on
`(provider, event_id)`, and credit the wallet + write the ledger row + invoice
in one transaction.

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
- `mail.send` queue + worker: invite and order-status email (contract in
  `WORKER_JOBS.md` §1)

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
