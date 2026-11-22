# COD Profit Recovery Platform — Product & Build Plan

**Status:** DRAFT — approval required before implementation  
**Date:** 2026-11-18  
**Repo:** `erp-commerce`  
**Scope:** first commercial product track. This plan narrows the earlier broad
agency/SaaS plan to one wedge. Existing digital-commerce and service-order
modules stay in the repo; they are not the v1 sales motion.

---

## 0. One-line definition

> A self-service COD profit-recovery platform for Bangladeshi e-commerce
> businesses: connect an existing WooCommerce/Laravel/custom store, get a free
> COD leak audit, confirm orders via WhatsApp, reconcile courier COD, and
> recover lost revenue without changing the website.

The platform owner operates the backend and exceptions; the client signs up,
connects its store, configures packs, and uses the product without manual help.

---

## 1. Why this wedge

- Bangladesh is COD-heavy: public estimates put COD at 75–90% of online orders.
- Fake orders, cancellations, returns, courier fees, and missing COD remittances
  leak cash every month.
- Most SMEs handle this in Excel, WhatsApp, and courier panels; the work is
  manual, fragmented, and hard to prove.
- Global tools (Shopify, Zapier, Gorgias, Klaviyo, Triple Whale) do not cover
  the local last mile: Pathao/Steadfast/RedX, bKash/SSLCommerz, Bangla/Banglish
  confirmation, COD reconciliation.
- The value is measurable in taka, which makes the first sale easier than
  selling another chatbot, CRM, or storefront.

---

## 2. Ideal customer profile (ICP)

| Attribute | Target |
| --- | --- |
| Order volume | 500–5,000 COD orders/month |
| Category | fashion, electronics, cosmetics, gadgets |
| Store | WooCommerce/WordPress or Laravel/custom |
| Acquisition | Facebook/Instagram/Meta ads, WhatsApp, website |
| Team | 3–20 people |
| Pain | fake/cancelled orders, returns, courier COD mismatch |
| Geography | Dhaka and Chattogram first |
| Buyer | founder or operations lead |

**Not the first target:** businesses below ~200 orders/month, businesses that
only sell through a marketplace, and clients who will not share order data.

---

## 3. Non-goals for v1

Do not build these before the COD loop is proven:

- storefront or website builder
- booking/appointment module
- full CRM suite
- generic IFTTT-style workflow builder
- AI chatbot as the primary product
- Meta Ads campaign manager
- Tauri/desktop app
- multi-level agency/reseller hierarchy
- marketplace or B2B wholesale portal

These are later modules. The automation engine is an internal capability, not
the v1 sales pitch.

---

## 4. Core user journeys

### 4.1 Self-service onboarding

1. Client signs up with business email.
2. Creates a business workspace and invites team members.
3. Connects its store:
   - WooCommerce: API key/plugin + webhook;
   - Laravel/custom: REST API key + webhook;
   - CSV fallback.
4. Imports the last 90 days of orders/customers.
5. Receives a free **COD Leak Audit**: estimated fake/cancel/return loss,
   courier mismatch, address/phone issues, recoverable amount.
6. Chooses a plan and enables the COD Confirm pack.
7. Connects a WhatsApp number and approves Bangla/English templates.
8. Goes live.

### 4.2 Daily COD loop

1. A new COD order arrives by webhook/API.
2. Risk engine scores it and explains why.
3. Confirmation workflow sends WhatsApp; falls back to a human call.
4. Agent marks confirmed, cancelled, or held.
5. The order status is pushed back to the store.
6. Courier delivery and COD status are ingested.
7. Reconciliation matches parcel, COD, fee, and return.
8. A recovery case is opened when money is missing.
9. The owner sees recovered revenue and leak reduction.

### 4.3 Team workflow

- Owner: dashboard, pilot status, team, pack settings.
- Admin/ops: confirmation queue, risk review, recovery cases.
- Staff: assigned confirmations and calls only.
- Platform admin: tenant health, connector errors, usage, exceptions.

---

## 5. Product scope

### 5.1 Onboarding

- Tenant creation and membership.
- Store connector setup with connection test.
- Historical import (90 days default).
- Free COD Leak Audit.
- Plan selection.
- WhatsApp connection.
- Team invitations.
- Onboarding checklist and activation state.

### 5.2 COD Leak Audit

The audit must quantify, not just describe:

- total COD orders
- risk distribution
- estimated fake/cancelled/returned orders
- estimated lost ad spend, courier fee, and return cost
- courier COD mismatch
- address/phone quality issues
- recoverable amount per month

The audit is the lead magnet. It must work before a paid plan is active.

### 5.3 Order ingestion and risk

- WooCommerce webhook/API.
- Laravel/custom webhook + CSV.
- Idempotent order upsert by `(tenant_id, connector_id, external_id)`.
- Customer identity by normalized phone, then email.
- Risk signals:
  - phone validity and repeat behaviour
  - address completeness and area
  - order value
  - first-time vs repeat customer
  - previous cancels/returns
  - courier/area performance
  - product category
  - time of order
- Output: score, reason codes, recommended action.

### 5.4 Confirmation workflow

- WhatsApp Cloud API templates (Bangla/English).
- Human call fallback.
- Assignment, SLA, retries, opt-out.
- Confirm/cancel/hold actions.
- Push status back to the source store.
- Audit trail for every message and decision.
- No automatic refund or price promise without human approval.

### 5.5 Courier reconciliation

- CSV upload first; API connectors later.
- Supported initial formats: Pathao, Steadfast, RedX.
- Match by consignment ID, then order ID, then phone + amount.
- Detect:
  - COD collected but not remitted
  - overcharged delivery fee
  - return without status
  - duplicate statement line
  - missing parcel
- Create a recovery case with evidence and amount.
- Export a reconciliation report.

### 5.6 Automation packs

Packs are prebuilt rules, not a generic builder. The client enables and
configures a pack.

| Pack | Trigger | Actions |
| --- | --- | --- |
| COD Confirm | new COD order | risk check → WhatsApp → call fallback → hold/release |
| Cart Rescue | cart abandoned 1h | WhatsApp/SMS + offer link |
| Delivery Update | shipped/delivered | WhatsApp update → review request |
| Courier Reconciliation | statement uploaded | match → recovery case → alert |
| Review Request | delivered 3d | WhatsApp review link |
| Lead Capture | new lead | CRM record → WhatsApp follow-up |
| Stock/Ads Guard | low stock | pause ads / notify |

v1 ships COD Confirm and Courier Reconciliation. The rest are v1.1.

### 5.7 Pilot operations and admin

- Free pilot activation, fixed term, feature flags.
- Usage counters for orders, confirmations, and statements.
- Platform admin: tenants, connector health, usage, support queue.
- Audit and data export.
- Billing/subscriptions are **post-MVP**, after the pilot proves recovery.

---

## 6. Architecture

### 6.1 Shape

Modular monolith in the existing repo:

```text
Axum API ── Postgres
   │
   ├── Redis worker (sync, webhook, confirmations, automation, reconciliation)
   ├── S3/R2 private storage (statement files)
   └── Vue 3 SPA (client dashboard + platform admin)
```

One deployment, one database, one auth system. No microservices in v1.

### 6.2 Modules

```text
src/modules/
  tenancy/          tenants, memberships, tenant context, audit
  connectors/       WooCommerce, Laravel/custom, CSV, credentials
  orders/           external orders, customers, events
  cod/              risk checks, confirmations, queue
  couriers/         shipments, statements, reconciliation
  recovery/         recovery cases
  automation/       packs, rules, runs, worker jobs
  billing/          pilot usage; plans/subscriptions post-MVP
  admin/            platform tenant/health views
```

Existing `auth`, `common`, `digital`, `services`, `finance`, and `uploads`
remain. The new COD modules must not duplicate wallet or service-order rules.

### 6.3 Tenancy decision

- `tenants` represents one client business.
- `memberships` maps global `users` to tenant roles.
- New COD tables carry `tenant_id` from day one.
- Existing digital/service tables remain legacy single-workspace for v1; do
  not migrate them in the same phase. A later phase will backfill them if the
  product needs one unified tenant model.
- Resolve the active tenant from `X-Tenant-Id` plus a membership check. Never
  trust a tenant id from a request body.
- Every new query includes `tenant_id`; integration tests must prove
  cross-tenant `404`.

### 6.4 Connectors

- A connector is tenant-owned and stores encrypted credentials.
- Credentials are never returned by any DTO.
- Webhooks are signed and idempotent by `(connector_id, event_id)`.
- Each connector has health, last sync, error count, and manual re-sync.
- Start with WooCommerce; add Laravel/custom via a documented REST + webhook
  contract; CSV is the fallback.

### 6.5 Workers

Required queues:

- `connector.sync` — historical/periodic import
- `webhook.process` — async webhook handling
- `confirmation.send` — WhatsApp/SMS send with retry
- `automation.run` — pack actions and delays
- `courier.reconcile` — statement matching
- `billing.reconcile` — post-MVP usage/subscription checks

Every job is idempotent, retried with backoff, and dead-lettered after the
limit. Jobs carry ids, not payload copies.

### 6.6 Storage and security

- Courier statements and exports live under
  `tenants/{tenant_id}/statements/...`; only signed URLs leave the API.
- Encrypt connector credentials at rest; key management is a Phase 1 decision.
- Do not log tokens, phone numbers, full addresses, or signed URLs.
- Rate-limit public webhooks and confirmation sends.
- Keep an append-only `audit_events` row for every money/status decision.

### 6.7 Dependencies to approve

Per `AGENTS.md`, no new crate without approval. The likely additions:

- HTTP client for connectors/WhatsApp (e.g. `reqwest`)
- Redis client/queue (e.g. `redis` or `deadpool-redis`)
- CSV parsing (e.g. `csv`)
- Credential encryption (e.g. `chacha20poly1305` or KMS)
- Background runtime wiring in `src/bin/worker.rs`

List the exact versions and purpose in the phase PR before adding them.

---

## 7. Data model

All new tables include `tenant_id` unless noted. Money is `NUMERIC(12,2)`;
timestamps are `TIMESTAMPTZ`.

| Table | Key fields | Notes |
| --- | --- | --- |
| `tenants` | id, name, slug, status, timezone, currency | one per client business |
| `memberships` | tenant_id, user_id, role, status | owner/admin/staff |
| `audit_events` | tenant_id, actor_id, entity_type, entity_id, action, before, after | append-only |
| `connectors` | tenant_id, kind, status, credentials_encrypted, config, last_sync_at | WooCommerce/Laravel/CSV |
| `external_orders` | tenant_id, connector_id, external_id, order_number, customer_id, amount, cod_amount, status, placed_at, raw | unique (tenant, connector, external_id) |
| `customers` | tenant_id, phone_normalized, name, email, first_seen_at, last_seen_at | tenant-scoped identity |
| `order_events` | tenant_id, order_id, event_type, payload, occurred_at | webhook/status history |
| `cod_checks` | tenant_id, order_id, risk_score, reasons, recommended_action, status | explainable |
| `confirmations` | tenant_id, order_id, channel, status, attempts, last_attempt_at, response | WhatsApp/call |
| `shipments` | tenant_id, order_id, courier, consignment_id, status, cod_amount, delivered_at | courier tracking |
| `courier_statements` | tenant_id, courier, period, file_key, status | uploaded statement |
| `courier_statement_lines` | statement_id, consignment_id, order_id, cod_collected, fee, return_flag, mismatch_reason | reconciliation |
| `recovery_cases` | tenant_id, type, order_id/line_id, amount, status, assigned_to, notes | money recovery |
| `automation_rules` | tenant_id, pack_key, trigger, conditions, actions, is_active | templates, not a builder |
| `automation_runs` | tenant_id, rule_id, status, input, output, error, attempts | logs/retry |
| `subscriptions` | tenant_id, plan, status, current_period_end, gateway, gateway_ref | **post-MVP**; free pilot first |
| `usage_counters` | tenant_id, metric, period, count | pilot usage metrics and later plan limits |

Invariants:

- An order cannot be confirmed and cancelled in the same state transition.
- A recovery case cannot exceed the unmatched amount for its statement line.
- A courier statement line can match at most one order.
- A tenant cannot read another tenant's order, customer, statement, or case.
- Every state change writes an audit event.

---

## 8. API surface (v1)

Use the existing `{ data, meta }` envelope and `AppError` codes.

### Onboarding and tenancy

- `POST /api/v1/auth/register` (existing, extended to create a tenant)
- `POST /api/v1/tenants`
- `GET /api/v1/tenants/current`
- `GET /api/v1/team`, `POST /api/v1/team/invite`
- `PATCH /api/v1/team/{user_id}`

### Connectors

- `GET /api/v1/connectors`
- `POST /api/v1/connectors` (WooCommerce/Laravel/CSV)
- `POST /api/v1/connectors/{id}/test`
- `POST /api/v1/connectors/{id}/sync`
- `DELETE /api/v1/connectors/{id}`
- `POST /api/v1/webhooks/{connector_id}` (public, signed)

### Orders and audit

- `GET /api/v1/orders` (filters: status, risk, date)
- `GET /api/v1/orders/{id}`
- `POST /api/v1/audit/run`
- `GET /api/v1/audit/report`

### COD confirmation

- `GET /api/v1/cod/queue`
- `POST /api/v1/cod/{order_id}/confirm`
- `POST /api/v1/cod/{order_id}/cancel`
- `POST /api/v1/cod/{order_id}/hold`
- `GET /api/v1/confirmations`

### Courier reconciliation

- `POST /api/v1/courier/statements` (upload)
- `GET /api/v1/courier/statements`
- `GET /api/v1/reconciliation`
- `GET /api/v1/recovery/cases`
- `PATCH /api/v1/recovery/cases/{id}`

### Automation

- `GET /api/v1/automation/packs`
- `POST /api/v1/automation/packs/{key}/enable`
- `PATCH /api/v1/automation/packs/{key}`
- `GET /api/v1/automation/runs`

### Pilot/admin

- `GET /api/v1/usage` (pilot metrics)
- `GET /api/v1/admin/tenants` (platform admin only)
- `GET /api/v1/admin/health`

Billing endpoints (`/billing/subscription`, `/billing/checkout`) are
**post-MVP**. Do not build a gateway before the pilot proves recovery.

Every new endpoint must be added to `docs/API_SPEC.md` with success and 4xx
responses before merge.

---

## 9. Frontend screens (Vue 3)

- Auth: login, register, verify.
- Onboarding: business profile → connect store → import → audit → plan →
  WhatsApp → live.
- Dashboard: leak, recovered amount, confirm rate, courier mismatch, alerts.
- Orders: list, filters, risk badges, detail.
- Confirmation inbox: queue, assigned to me, templates, call fallback.
- Risk review: reasons, hold/release, blacklist.
- Courier reconciliation: upload, match table, mismatch detail.
- Recovery cases: amount, evidence, status.
- Automation packs: enable/configure, run logs.
- Team and settings.
- Pilot usage (billing is post-MVP).
- Platform admin: tenants, connector health, exceptions.

Every screen ships loading, empty, error, and disabled states. No signed URL is
stored in Pinia or `localStorage`.

---

## 10. Phase-by-phase build plan

### Phase 0 — Product freeze and spec (1 week)

**Goal:** stop scope drift before code.

Deliverables:

- Approve this document and the ICP/pricing assumptions.
- Interview 10 businesses; record stack, order volume, COD %, tools, pain,
  current spend.
- Select 2 design partners.
- Update `docs/DATABASE.md`, `docs/API_SPEC.md`, and `docs/ROADMAP.md` with the
  COD track.
- Decide connector order, WhatsApp provider, courier format priority, and
  credential encryption approach.

Acceptance: written sign-off; no unresolved product-blocking decision.

### Phase 1 — Tenancy and platform foundation (2 weeks)

**Goal:** one tenant can sign up and be isolated.

Deliverables:

- Migration `0009_tenancy.sql`: tenants, memberships, audit_events.
- `TenantContext` extractor and role checks.
- Signup creates tenant + owner membership in one transaction.
- Team invite/accept flow.
- Existing endpoints remain functional.
- Frontend shell: auth, onboarding step 1, team.

Tests:

- Two tenants cannot read each other's data.
- Missing/invalid tenant header returns `403`/`404`, never leaks.
- Existing test suite passes.

### Phase 2 — Store connectors and order ingest (2 weeks)

**Goal:** orders from an existing store arrive reliably.

Deliverables:

- `connectors`, `external_orders`, `customers`, `order_events`.
- WooCommerce connector: connect, test, initial import, webhook.
- Laravel/custom connector contract: API key + webhook signature + CSV.
- Idempotent upsert and retry/error dashboard.
- Frontend: connect store, import status, error log.

Tests:

- Duplicate webhook creates one order.
- Bad signature returns `400` and writes nothing.
- CSV import handles 5,000 rows.
- Cross-tenant connector access returns `404`.

### Phase 3 — COD Leak Audit and risk engine (2 weeks)

**Goal:** prove the money leak before charging.

Deliverables:

- `cod_checks`, risk rules, reason codes.
- Audit report with estimated loss and recoverable amount.
- Risk queue and order detail.
- Frontend: audit report, risk badges, hold/release.

Tests:

- Known-good orders are not flagged without a reason.
- Audit is deterministic for the same input.
- Report completes for 500 orders in under 60 seconds.
- Every risk decision is explainable and audited.

### Phase 4 — Confirmation workflow (2 weeks)

**Goal:** confirm/cancel a COD order and push the result back.

Deliverables:

- `confirmations`, WhatsApp Cloud API, templates, opt-out.
- Human call fallback and assignment/SLA.
- Confirm/cancel/hold actions; source-store status update.
- Frontend: confirmation inbox and order detail.

Tests:

- New order reaches the queue in under 2 seconds.
- Send is idempotent; retries do not duplicate messages.
- Opt-out stops all messages.
- Confirm/cancel updates the order once and writes audit.

### Phase 5 — Courier reconciliation, CSV + API (3 weeks)

**Goal:** find money owed by couriers, with CSV as the guaranteed path and API
as the faster path.

Deliverables:

- `shipments`, `courier_statements`, `courier_statement_lines`,
  `recovery_cases`.
- CSV parsers for Pathao/Steadfast/RedX formats.
- API clients for the same couriers, behind a connector flag; polling or
  webhook where the provider supports it.
- Credential storage, rate-limit handling, retry, and CSV fallback when an API
  is unavailable.
- Matching and mismatch rules.
- Fixtures: `tests/fixtures/courier/` with courier-specific order samples and
  the universal statement (`universal_cod_statement.csv`); mapping and source
  notes in `docs/COURIER_CSV_SAMPLES.md`.
- Frontend: upload, reconciliation table, mismatch detail, recovery queue.

Sequence:

1. CSV parser + reconciliation end to end.
2. API client for one courier.
3. API clients for the remaining couriers if credentials/approval are ready.
4. CSV stays the fallback for every tenant.

Tests:

- Sample statements detect missing COD, overcharge, duplicate line.
- Re-upload is idempotent.
- API retries do not duplicate lines.
- API failure falls back to the CSV path without creating false cases.
- Unmatched lines never create a false recovery case.
- Tenant isolation on every statement/case.

### Phase 6 — Automation packs and worker (2 weeks)

**Goal:** turn the core loop into repeatable packs.

Deliverables:

- `automation_rules`, `automation_runs`, Redis worker.
- COD Confirm and Courier Reconciliation packs.
- Cart Rescue, Delivery Update, Review Request, Lead Capture packs behind
  feature flags.
- Retry, dead-letter, pause/resume, run logs.
- Frontend: pack enable/configure, run logs.

Tests:

- A pack cannot run for another tenant.
- Retries are idempotent.
- Disabled pack stops future runs.
- Every run records trigger, action, result, error.

### Phase 7 — Pilot operations, onboarding polish, admin (1 week)

**Goal:** a free pilot client can activate without the owner touching the
account; billing is deliberately deferred.

Deliverables:

- `usage_counters` for pilot metrics; no subscription gateway in v1.
- Pilot agreement/activation state and tenant feature flags.
- Onboarding checklist and activation state.
- Platform admin: tenants, health, usage, connector errors.
- Frontend: usage, team, admin; no billing checkout yet.

Tests:

- Pilot activation state is visible and reversible.
- Usage counters are accurate for orders/confirmations/statements.
- Platform admin cannot read tenant business data without an audited reason.

**Billing is post-MVP.** Do not block the pilot on a payment gateway.

### Phase 8 — Hardening and pilot launch (1.5 weeks)

**Goal:** 10 free-pilot/design-partner tenants using it daily.

Deliverables:

- Rate limits, request size limits, webhook replay protection.
- Structured logs/metrics; no secrets/PII.
- Backup/restore check; migration rollback rehearsal.
- Load test: 50 concurrent orders, 1,000 statements.
- 2 case studies with before/after numbers.
- Pricing and cancellation policy.

Acceptance:

- 10 pilot tenants live; first recovered taka tracked.
- No cross-tenant finding in the isolation suite.
- `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` pass.
- CI green.

---

## 11. MVP boundary

**MVP = Phases 0–5 + Phase 7-lite (pilot operations, no billing).**

- **12 build weeks after Week 0 (13 calendar weeks)** with one focused
  developer if courier API access is ready.
- **14–15 calendar weeks** if one or more courier API approvals slip; CSV
  remains the guaranteed path.
- 8–10 weeks with two developers (backend + frontend).
- Automation packs beyond COD Confirm and courier reconciliation are v1.1.
- Payment gateway/subscriptions are post-MVP; the first cohort is a free pilot.

Do not wait for the full platform. The first sale is the audit plus confirmed
orders; everything else follows.

---

## 12. Success metrics

| Metric | Target |
| --- | --- |
| Activation: connect → audit | < 24 hours |
| Time to first confirmation | < 48 hours |
| Time to first recovered taka | < 7 days |
| COD confirmation rate | measured, then +10–20% |
| Fake/cancel/return reduction | 20–40% in pilot |
| Courier mismatch recovered | tracked per tenant |
| Weekly active tenants | 8/10 by week 12 |
| 3-month retention | 80%+ |
| Packs per tenant | 1.5+ by month 6 |

---

## 13. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| False positives block good customers | explainable rules, human review, grace limits |
| Client will not share order data | read-only scopes, signed webhook, CSV fallback |
| WhatsApp policy/template rejection | approved templates, opt-out, SMS/call fallback |
| Courier statement formats vary | CSV first, parser per courier, manual correction |
| Cross-tenant data leak | tenant_id everywhere, isolation tests, audit |
| Low willingness to pay | free audit, pilot, recovery-based fee |
| Meta/WhatsApp API changes | adapter boundary, official API only |
| Support burden | self-serve diagnostics, connector health, run logs |
| Scope creep into storefront/CRM | non-goals list; phase gate approval |

---

## 14. Decisions (approved 2026-11-22)

| # | Decision | Choice | Impact |
| --- | --- | --- | --- |
| 1 | First store connector | **WooCommerce** | Phase 2 starts with WooCommerce; Laravel/custom follows |
| 2 | WhatsApp confirmation path | **Cloud API direct** | Own adapter; local BSP can be added later |
| 3 | Courier data source | **CSV + API together** | Phase 5 extended to 3 weeks; CSV is the fallback if API access slips |
| 4 | Cross-tenant COD risk data | **Tenant-only in v1** | No cross-tenant risk sharing; aggregate later with consent |
| 5 | Pilot billing | **Free pilot, billing later** | Phase 7 is pilot operations/admin; no gateway in v1 |
| 6 | Existing digital/service tables | **New COD tables only** | No tenant refactor of legacy modules in v1 |

### Consequences

- MVP is **12 build weeks after Week 0 (13 calendar weeks) if courier API
  access is ready; 14–15 calendar weeks if it slips**.
- Phase 5 ships CSV first, then API for one courier, then the remaining
  couriers as credentials/approval arrive.
- Phase 7 has no subscription gateway; `subscriptions` stays post-MVP.
- The first cohort is a **free pilot**; convert to paid after measured recovery.
- Existing digital-commerce/service-order tables remain untouched; the new COD
  modules carry `tenant_id`.

---

## 15. Relationship to the existing repo

- Keep `0001`–`0008` untouched; new migrations start at `0009`.
- Reuse `auth`, `common`, `error`, `response`, storage, and the test harness.
- Do not reuse `service_orders` as the COD order model; it is a different
  lifecycle. A later integration can link a COD order to a service order.
- Do not reuse `users.wallet_balance` for platform subscriptions; platform
  billing is separate from tenant commerce money.
- Update `docs/API_SPEC.md`, `docs/DATABASE.md`, `docs/WORKFLOWS.md`, and
  `docs/ROADMAP.md` in the same PR as the first implementation.
- Build the Vue app under `frontend/` per `docs/FRONTEND_ARCHITECTURE.md`; do
  not introduce Next.js or a second backend.

---

## 16. Definition of done for every phase

1. Migrations apply cleanly on PostgreSQL 16.
2. `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` pass.
3. New endpoints are in `docs/API_SPEC.md`; new tables in `docs/DATABASE.md`.
4. Tenant-isolation test covers every new tenant-owned table.
5. No secret, signed URL, full phone number, or address is logged.
6. Frontend screen has loading, empty, error, and disabled states.
7. The phase's acceptance criteria are demonstrated with a test or a seeded
   local run.
