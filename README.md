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

just up                   # postgres + redis + minio (dev stack)
just migrate              # or: sqlx migrate run
just dev                  # API on http://localhost:8080
```

Verify:

```bash
curl http://localhost:8080/healthz          # -> ok
curl http://localhost:8080/api/v1/digital/products
```

## Smoke test

With the API running, one command exercises the P1 paths end to end:

```bash
./scripts/smoke.sh
# other host:      ./scripts/smoke.sh https://api.example.com
# fund a fresh user: DATABASE_URL=postgres://... ./scripts/smoke.sh
```

It checks health, register/login, the catalogue, the `402` path, checkout,
idempotent replay, the vault, a signed download URL, and the `403` refund
guard. Wallet-funded tests are skipped when there is no way to call
`seed_wallet()` — there is no top-up API before P3.

### Local test suite

Every integration test gets its own database and runs all migrations, so
parallel runs are the main memory spike. On a small machine:

```bash
./scripts/test.sh                  # starts a low-memory PostgreSQL if needed
TEST_THREADS=2 ./scripts/test.sh   # allow more parallelism
KEEP_PG=1 ./scripts/test.sh        # leave the server running afterwards
```

The script stops a server it started. If PostgreSQL is already running, it
just runs the suite.

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

## Deploy

`docker-compose.yml` is the production stack: it runs only the API and expects
external PostgreSQL, Redis and S3-compatible storage. Local infrastructure
lives in `docker-compose.dev.yml` (Postgres, Redis, MinIO).

### Coolify

1. Create a **Docker Compose** resource pointing at this repository.
2. Leave **Docker Compose Location** empty (defaults to `/docker-compose.yml`).
3. Add the variables from `.env.production.example` in the Coolify UI
   (Environment Variables). Required ones fail fast with a clear message.
4. Keep `RUN_MIGRATIONS=1` for the first deploy with a single replica, then set
   it to `0` and run migrations as a one-shot job when scaling.
5. Expose container port `8080` and point the domain at it.

Do not point Coolify at `docker-compose.dev.yml`: it starts Postgres, Redis and
MinIO and has no API service.

### Cloudflare R2

1. Create a bucket (for example `erp-commerce-prod`). Leave **public access
   disabled** — every read goes through a signed URL.
2. **Manage R2 API Tokens** → create a token with *Object Read & Write*,
   scoped to that bucket only. Copy the Access Key ID and Secret Access Key.
3. The endpoint is `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`; set
   `S3_REGION=auto` and keep the bucket name out of the endpoint.
4. Browser uploads go straight to R2 via pre-signed PUT, so the bucket needs a
   CORS policy:

   ```json
   [
     {
       "AllowedOrigins": ["https://app.example.com"],
       "AllowedMethods": ["GET", "PUT"],
       "AllowedHeaders": ["*"],
       "ExposeHeaders": ["ETag"],
       "MaxAgeSeconds": 3600
     }
   ]
   ```

5. CORS is not needed for server-to-server reads/writes, only for the browser
   `PUT` that follows `POST /uploads/presign`.

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
