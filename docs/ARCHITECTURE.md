# High-Level System Architecture Specification

## 1. Overview

An integrated ERP and Digital Commerce engine tailored for e-commerce entrepreneurs. The platform combines instant digital product delivery (assets, templates, dynamic generator tools) with structured agency service fulfillment (intake briefs, kanban queues, proofs, revisions, delivery).

Two fulfilment domains share one identity layer, one wallet, and one audit trail:

| Domain | Primary Entity | Fulfilment Model |
| --- | --- | --- |
| Digital Commerce | `digital_products` | Instant, automated, entitlement-gated download |
| Agency Services | `agency_services` | Human workflow, kanban queue, proof → revision → approval |

## 2. Core Tech Stack

- **Backend**: Rust (Axum, Tokio, Tower, Serde, SQLx)
- **Primary Database**: PostgreSQL 16+ (normalized transactional layer, strict foreign keys, `JSONB` for flexible briefs)
- **Cache & Queue**: Redis (session caching, background workers, LLM streaming state)
- **Frontend**: Vue 3 + Vite + Tailwind CSS (Single Page Application architecture)
- **Cross-Platform Wrapper**: Tauri v2 (Desktop Windows/macOS and mobile readiness)
- **File Storage**: S3-compatible private object storage (Cloudflare R2 / AWS S3)
- **Reverse Proxy / TLS**: Any standard terminator (Caddy, Nginx, or platform edge) in front of the API

### 2.1 Dependency Rules

- No `axum` handler may import `sqlx` directly. Handlers call services; services call repositories.
- Only the repository/data layer holds SQL strings.
- Only the service layer enforces state machines. A database `CHECK`/trigger is a backstop, never the primary guard.
- Workers consume the same service layer as the API — no duplicated business rules.

## 3. Layered Design

```
┌──────────────────────────────────────────────────────────────┐
│ Transport Layer   Axum routers, JWT extractor, RBAC middleware│
│                   request validation, response serialization  │
├──────────────────────────────────────────────────────────────┤
│ Service Layer     business rules, state machines, transactions│
│                   authorization-by-ownership, orchestration   │
├──────────────────────────────────────────────────────────────┤
│ Data Layer        SQLx repositories, compile-time checked SQL │
│                   transactions, row locking                   │
├──────────────────────────────────────────────────────────────┤
│ Worker Layer      watermarking, signed URL issuance, AI copy  │
│                   streaming, retries, dead-letter handling    │
└──────────────────────────────────────────────────────────────┘
```

- **Transport Layer**: Axum REST API with JWT auth extractor and RBAC middleware. Responsible for parsing, validation, and HTTP semantics only. Emits `409 Conflict` for illegal state transitions, never silently coerces state.
- **Service Layer**: Business logic isolated from database concerns; enforces state machine rules and the ownership/role matrix. Every multi-row mutation runs inside one transaction.
- **Data Layer**: SQLx strongly-typed queries using compile-time SQL verification or parameterized prepared statements. All list queries are paginated; no unbounded `SELECT *`.
- **Worker Layer**: Background async tasks for image generation/watermarking, AI copy streaming, and temporary signed URL creation. Workers are idempotent and safe to retry.

## 4. Runtime Topology

```
            ┌───────────┐
  Browser ──┤  CDN/Edge ├── TLS
  Tauri  ────┴─────┬─────┘
                   ▼
           ┌───────────────┐        ┌──────────────┐
           │  Axum API     │◄──────►│   Redis      │
           │  (N replicas) │        │ cache + queue│
           └───────┬───────┘        └──────┬───────┘
                   │                        │
                   ▼                        ▼
           ┌───────────────┐        ┌──────────────┐
           │ PostgreSQL 16 │        │ Worker Pool  │
           │  (primary)    │        │ watermark/URL│
           └───────────────┘        └──────┬───────┘
                                           │
                                    ┌──────▼───────┐
                                    │ S3 / R2      │
                                    │ private only │
                                    └──────────────┘
```

- API and workers are stateless; scaling is horizontal.
- No component is allowed to hold a long-lived secret in the frontend. Only the backend signs object-storage URLs.
- Signed URLs are generated per request and never cached in Redis or returned by any list endpoint.

## 5. Security Baseline

- Passwords hashed with Argon2id; never logged.
- JWT access tokens are short-lived; refresh handled via rotating token or re-login.
- Role checks are enforced server-side per route, plus ownership checks per resource.
- Every download and every order transition writes an audit record.
- All storage access is private-by-default; see `STORAGE_SECURITY.md` for the permission matrix.

## 6. Reliability Rules

- Payment/checkout → entitlement creation is one atomic transaction; either both exist or neither does.
- Object upload happens before the database row that references it is committed; orphan sweeps run periodically.
- Background jobs are retried with exponential backoff and a max attempt count, then parked for manual inspection.
- A failed watermark job must never expose the unwatermarked original on the client-reachable path.

## 7. Documentation Reading Order

An implementing agent must read these in order before writing code:

1. `docs/ARCHITECTURE.md` — the shape of the system (this file)
2. `docs/DATABASE.md` — the schema and its invariants
3. `docs/API_SPEC.md` — the HTTP contract
4. `docs/WORKFLOWS.md` — the state machines behind the contract
5. `docs/STORAGE_SECURITY.md` — how bytes are protected
6. `docs/UI_DESIGN_SYSTEM.md` — tokens, type, component contracts
7. `docs/UI_SCREEN_SPECS.md` — screen layouts and data sources
8. `docs/UI_USER_FLOWS.md` — interaction flows and error handling

UI work additionally requires `assets/fonts/anek-bangla/README.md`. The font is
self-hosted; importing it from a CDN is not permitted.

Then, and only then, implement database migrations and models first.
