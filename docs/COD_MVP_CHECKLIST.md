# COD Platform — MVP Execution Checklist

**Status:** DRAFT — run after `docs/COD_PLATFORM_PLAN.md` is approved  
**Date:** 2026-11-22  
**MVP:** 12 build weeks after Week 0 (13 calendar weeks) with one focused
developer if courier API access is ready; 14–15 calendar weeks if it slips.
8–10 weeks with a second developer (backend + frontend). The first cohort is a
**free pilot**; billing is post-MVP.

Use this as the working board. One checked item = one demonstrable outcome, not
just merged code.

---

## Week 0 — Product freeze

- [ ] 10 customer interviews completed; notes stored in `docs/research/`
- [ ] ICP, pricing, and non-goals approved
- [ ] 2 design partners signed for a free audit
- [x] WooCommerce confirmed as the first connector
- [x] WhatsApp Cloud API direct confirmed
- [x] Courier CSV + API approach confirmed
- [x] Tenant-only risk data confirmed
- [x] Free pilot / later billing confirmed
- [x] New COD tables only; legacy tables untouched
- [x] Courier CSV fixtures created in `tests/fixtures/courier/`; universal sample documented in `docs/COURIER_CSV_SAMPLES.md`
- [ ] Courier API access status confirmed (Pathao/Steadfast/RedX)
- [ ] `docs/COD_PLATFORM_PLAN.md` approved
- [x] `docs/API_SPEC.md`, `docs/DATABASE.md`, `docs/ROADMAP.md` updated for the tenancy slice

**Exit:** no open product-blocking decision.

---

## Week 1–2 — Tenancy and signup

### Database

- [x] `0009_tenancy.sql`: `tenants`, `memberships`, `audit_events`
- [x] indexes on `(tenant_id, ...)` for every tenant-owned table
- [x] migration applies from an empty DB and from `0001`–`0008`

### Backend

- [x] `src/modules/tenancy/` models, dtos, service, handlers
- [x] `TenantContext` extractor from `X-Tenant-Id`
- [x] membership role checks: owner/admin/staff
- [x] signup creates tenant + owner in one transaction
- [x] team invite/accept endpoints
- [x] audit event on signup, invite, role change

### Frontend

- [ ] auth shell
- [ ] onboarding step 1: business name/timezone/currency
- [ ] team invite screen
- [ ] loading/empty/error/disabled states

### Tests

- [x] tenant A cannot read tenant B
- [x] missing/invalid tenant returns 400/404
- [x] signup rollback leaves no orphan tenant/user
- [x] existing test suite still passes

**Exit:** a new business can sign up and invite a team member.

---

## Week 3–4 — Connectors and order ingest

### Database

- [x] `connectors`
- [x] `external_orders`
- [x] `customers`
- [x] `order_events`
- [x] `connector_webhook_events` (idempotency)
- [x] unique `(tenant_id, connector_id, external_id)`

### Backend

- [x] connector credential encryption boundary
- [x] connector service with `test`, `sync`, `health`
- [x] WooCommerce signed webhook + order upsert
- [x] WooCommerce REST historical import/sync
- [ ] Laravel/custom webhook contract + signature
- [x] CSV import fallback (orders/customers)
- [x] idempotent webhook/order upsert
- [ ] worker queue `connector.sync` / `webhook.process`
- [x] tenant-scoped `GET /orders`

### Frontend

- [ ] connect store screen
- [ ] import progress and error log
- [ ] manual re-sync
- [ ] connector health badge

### Tests

- [x] duplicate webhook → one order
- [x] bad signature → 401, rejected event, no order
- [x] 5,000-row CSV import
- [x] cross-tenant connector access → 404
- [x] replay does not duplicate order/event
- [x] connector tests in `tests/connector_flow.rs`, including REST sync against a fake WooCommerce server

**Exit:** a WooCommerce store can connect and deliver orders without manual
work.

---

## Week 5–6 — COD Leak Audit and risk

### Database

- [ ] `cod_checks`
- [ ] risk reason codes / signal snapshot
- [ ] indexes for queue filters

### Backend

- [ ] risk rules: phone, address, amount, repeat, courier area, category, time
- [ ] deterministic score and reason list
- [ ] audit run for 90 days of orders
- [ ] recoverable amount calculation
- [ ] queue: high/medium/low risk

### Frontend

- [ ] “COD Leak Audit” report
- [ ] show estimated loss, recoverable amount, reasons
- [ ] risk queue
- [ ] order detail with explanation
- [ ] hold/release actions

### Tests

- [ ] same input → same score
- [ ] good orders do not get unexplained high risk
- [ ] 500-order audit under 60s
- [ ] every decision writes audit

**Exit:** the free audit produces a credible taka number for a design partner.

---

## Week 7–8 — WhatsApp confirmation

### Database

- [ ] `confirmations`
- [ ] template/version fields
- [ ] assignment/SLA fields

### Backend

- [ ] WhatsApp Cloud API adapter
- [ ] approved template send
- [ ] reply parsing: confirm/cancel/other
- [ ] opt-out handling
- [ ] manual call fallback
- [ ] confirm/cancel/hold state transition
- [ ] push status to source store
- [ ] worker queue `confirmation.send`

### Frontend

- [ ] confirmation inbox
- [ ] assigned to me / all
- [ ] template preview
- [ ] confirm/cancel/hold
- [ ] order detail and timeline

### Tests

- [ ] new order reaches queue < 2s
- [ ] retry does not duplicate message
- [ ] opt-out stops all sends
- [ ] confirm/cancel is idempotent
- [ ] source store update writes audit
- [ ] cross-tenant queue access → 404

**Exit:** a staff member can confirm a COD order and the store reflects it.

---

## Week 9–10 — Courier reconciliation: CSV first

### Database

- [ ] `shipments`
- [ ] `courier_statements`
- [ ] `courier_statement_lines`
- [ ] `recovery_cases`
- [ ] unique `(statement_id, consignment_id)`

### Backend

- [ ] CSV upload to private storage
- [ ] parsers for Pathao/Steadfast/RedX fixtures in `tests/fixtures/courier/`
- [ ] universal statement fixture parses and reconciles
- [ ] matching: consignment → order → phone+amount
- [ ] mismatch types: missing COD, overcharge, duplicate, return
- [ ] recovery case creation with evidence
- [ ] export reconciliation report
- [ ] worker queue `courier.reconcile`
- [ ] CSV fallback remains available even when API is connected

### Frontend

- [ ] upload screen
- [ ] reconciliation table
- [ ] mismatch detail
- [ ] recovery queue
- [ ] export button

### Tests

- [ ] sample statements produce expected mismatches
- [ ] re-upload is idempotent
- [ ] unmatched lines do not create false cases
- [ ] tenant isolation on statements/cases
- [ ] recovery amount cannot exceed unmatched amount

**Exit:** a design partner sees at least one real courier mismatch or a clean
reconciliation report.

---

## Week 11 — Courier API + pilot operations

### Backend

- [ ] API client for Pathao, Steadfast, RedX (one at a time)
- [ ] encrypted API credentials + connection test
- [ ] polling/webhook ingest, rate limits, retry
- [ ] API failure falls back to CSV without false cases
- [ ] `usage_counters` for pilot metrics
- [ ] pilot activation state and feature flags
- [ ] audit on pilot/role changes

### Frontend

- [ ] onboarding checklist
- [ ] pilot agreement/activation
- [ ] usage screen
- [ ] team/settings
- [ ] platform admin: tenant list, connector health, usage

### Tests

- [ ] API retry does not duplicate statement lines
- [ ] API failure falls back to CSV
- [ ] usage counters are accurate
- [ ] platform admin access is audited
- [ ] tenant data not visible in admin without reason

**Exit:** one courier API is live or explicitly deferred; the pilot can run on
CSV; usage is measurable. No billing gateway is required.

---

## Week 12 — Free pilot launch

- [ ] 10 free-pilot tenants invited; 5 connect a store
- [ ] 3 complete a free audit
- [ ] 2 enable COD Confirm
- [ ] 1 reconciliation uploaded (CSV or API)
- [ ] first recovered taka recorded
- [ ] pilot agreement signed (free, fixed term)
- [ ] 2 case studies with before/after
- [ ] rate limits, webhook replay protection, request size limits
- [ ] backup/restore check
- [ ] load test: 50 concurrent orders / 1,000 statement lines
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test` pass
- [ ] CI green

**Launch gate:** no cross-tenant finding; no secret/PII in logs; a client can
go from signup to live without manual setup.

---

## Post-MVP (v1.1)

- [ ] Subscription billing + gateway (after pilot conversion)
- [ ] Cart Rescue pack
- [ ] Delivery Update + Review Request pack
- [ ] Lead Capture pack
- [ ] Stock/Ads Guard pack
- [ ] WhatsApp template management UI
- [ ] Agency/white-label plan

## Later (v1.2+)

- [ ] Landing page + Meta CAPI/ROAS module
- [ ] Returns/exchange OS
- [ ] AI inbox/CRM
- [ ] Meta Ads/Commerce Manager integrations
- [ ] Advanced analytics/customer 360

---

## Definition of done (every item)

- migration + `docs/DATABASE.md`
- endpoint + `docs/API_SPEC.md`
- tenant-isolation test
- success + 4xx test
- no secret/signed URL/PII in logs
- frontend loading/empty/error/disabled states
- audit event for state/money decisions
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` pass
