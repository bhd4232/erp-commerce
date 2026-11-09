# ERP + Digital Commerce

An integrated ERP and digital commerce engine for e-commerce entrepreneurs:
instant digital product delivery (files, licences, AI-generated assets) plus
structured agency service fulfilment (intake briefs, kanban queue, proofs,
revisions, delivery).

- **Backend** — Rust, Axum, Tokio, SQLx, PostgreSQL 16, Redis
- **Frontend** — Vue 3, Vite, Tailwind CSS v4, Pinia
- **Desktop / mobile** — Tauri v2
- **Storage** — private S3 / Cloudflare R2, pre-signed URLs only

Everything is specified before it is implemented. Start with
[`AGENTS.md`](./AGENTS.md), then the documents in [`docs/`](./docs).

---

## Quick start

```bash
cp .env.example .env      # local values are already filled in for docker compose

docker compose up -d      # postgres + redis + minio (+ private bucket)
just migrate              # or: sqlx migrate run
just dev                  # API on http://localhost:8080
```

Verify:

```bash
curl http://localhost:8080/healthz          # -> ok
curl http://localhost:8080/api/v1/digital/products
```

Services:

| Service | URL | Credentials |
| --- | --- | --- |
| API | http://localhost:8080 | — |
| PostgreSQL | `localhost:5432/erp_commerce` | `postgres` / `postgres` |
| Redis | `localhost:6379` | none |
| MinIO API | http://localhost:9000 | `minioadmin` / `minioadmin` |
| MinIO console | http://localhost:9001 | `minioadmin` / `minioadmin` |

---

## Repository layout

```
.
├── AGENTS.md                  # rules every coding agent must follow
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── justfile
├── migrations/                # 0001 … 0005, append-only
├── src/
│   ├── main.rs                # router + startup
│   ├── config.rs  state.rs  error.rs
│   ├── common/                # auth, extractors, storage
│   └── modules/
│       ├── auth/              # register, login, me
│       ├── digital/           # catalogue, vault, checkout, download
│       └── services/          # order pipeline + proofing
├── assets/fonts/anek-bangla/  # self-hosted variable font
├── docs/                      # specifications
└── frontend/                  # Vue SPA (to be scaffolded)
```

---

## Documentation

| Document | Contents |
| --- | --- |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | System design, stack, topology |
| [`docs/ROADMAP.md`](./docs/ROADMAP.md) | Phases P0–P8, exit criteria |
| [`docs/DATABASE.md`](./docs/DATABASE.md) | Schema, indexes, invariants |
| [`docs/API_SPEC.md`](./docs/API_SPEC.md) | Endpoints, errors, rate limits |
| [`docs/WORKFLOWS.md`](./docs/WORKFLOWS.md) | State machines |
| [`docs/STORAGE_SECURITY.md`](./docs/STORAGE_SECURITY.md) | Buckets, signing, permissions |
| [`docs/BILLING_WALLET_SPEC.md`](./docs/BILLING_WALLET_SPEC.md) | Wallet, credits, gateways, invoices |
| [`docs/AI_GENERATOR_PIPELINE.md`](./docs/AI_GENERATOR_PIPELINE.md) | Generation jobs, SSE, credits |
| [`docs/FRONTEND_ARCHITECTURE.md`](./docs/FRONTEND_ARCHITECTURE.md) | Vue/Pinia/router structure |
| [`docs/UI_DESIGN_SYSTEM.md`](./docs/UI_DESIGN_SYSTEM.md) | Tokens, type, components |
| [`docs/UI_SCREEN_SPECS.md`](./docs/UI_SCREEN_SPECS.md) | Per-screen specification |
| [`docs/UI_USER_FLOWS.md`](./docs/UI_USER_FLOWS.md) | Flows and error handling |
| [`docs/TAURI_NATIVE_SPEC.md`](./docs/TAURI_NATIVE_SPEC.md) | Desktop/mobile integration |
| [`docs/I18N.md`](./docs/I18N.md) | Bengali-first formatting |
| [`docs/WORKER_JOBS.md`](./docs/WORKER_JOBS.md) | Queues, retries, job contracts |

---

## Status

| Area | State |
| --- | --- |
| Specifications | complete |
| DB migrations (`0001`–`0006`) | complete |
| Auth (register / login / me) | implemented |
| Digital catalogue, vault, download | implemented |
| Wallet checkout, idempotency, refund | implemented |
| Service order pipeline | routes + types, service stubs (`501`) |
| Billing, AI generator, notifications | specified, not implemented |
| Frontend SPA | not started |
| Tauri shell | not started |
| CI | configured |

Stubs return `501 not_implemented` and carry a TODO naming the exact SQL and
guards required — see `src/modules/*/service.rs`.

---

## Conventions

- Commits are topic-scoped Conventional Commits: `feat(auth): …`,
  `fix(order): …`, `docs(api): …`.
- Migrations are append-only.
- No secret, storage key, or signed URL is ever logged or returned to a client.
- See [`AGENTS.md`](./AGENTS.md) for the full rule set.
