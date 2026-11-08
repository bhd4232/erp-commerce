# AGENTS.md — Operating Rules for Coding Agents

This file is the entry point for any agent working in this repository. Read it
before touching code, then follow the reading order below.

---

## 1. Source of Truth

Specifications beat opinions. If code and docs disagree, the docs win until a
human updates them.

| Read | For |
| --- | --- |
| `docs/ARCHITECTURE.md` | System shape, layers, runtime topology |
| `docs/DATABASE.md` | Schema, invariants, concurrency rules |
| `docs/API_SPEC.md` | HTTP contract, error envelope, status codes |
| `docs/WORKFLOWS.md` | Service-order and digital-delivery state machines |
| `docs/STORAGE_SECURITY.md` | Bucket layout, pre-signed URLs, permissions |
| `docs/BILLING_WALLET_SPEC.md` | Wallet, credits, gateways, invoices |
| `docs/AI_GENERATOR_PIPELINE.md` | Generation jobs, SSE, credit deduction |
| `docs/FRONTEND_ARCHITECTURE.md` | Vue 3 / Pinia / router structure |
| `docs/UI_DESIGN_SYSTEM.md` | Tokens, type, component contracts |
| `docs/UI_SCREEN_SPECS.md` | Screens, data sources, states |
| `docs/UI_USER_FLOWS.md` | Interactions, error handling |
| `docs/TAURI_NATIVE_SPEC.md` | Desktop/mobile capabilities |
| `docs/I18N.md` | Bengali-first formatting rules |
| `docs/WORKER_JOBS.md` | Queue names, retries, job payloads |

Do not start a task without reading the documents it touches.

---

## 2. Repository Layout

```
.
├── AGENTS.md                 # this file
├── Cargo.toml                # backend crate
├── migrations/               # sqlx migrations, append-only
├── src/
│   ├── main.rs               # router composition
│   ├── config.rs             # env parsing, fail fast
│   ├── state.rs              # AppState
│   ├── error.rs              # AppError -> HTTP envelope
│   ├── common/               # auth, extractors, storage
│   └── modules/
│       ├── auth/
│       ├── digital/
│       └── services/
├── assets/fonts/anek-bangla/ # self-hosted font, do not replace with CDN
├── docs/                     # specifications
└── frontend/                 # Vue 3 SPA (see FRONTEND_ARCHITECTURE.md)
```

One directory per bounded context. `models.rs` = DB rows, `dtos.rs` = wire
types, `service.rs` = rules, `handlers.rs` = HTTP only.

---

## 3. Rust / Axum Rules

- **No `.unwrap()` / `.expect()`** in handlers, services, or extractors.
  Propagate with `?` into `AppError`. Panics are acceptable only in `main`
  startup and tests.
- **No SQL in handlers.** Handlers call services; services call repositories.
  `sqlx` must not appear in `handlers.rs`.
- **Every money or credit mutation runs in one transaction.** Never update a
  cached balance without inserting the matching ledger row.
- **Respect the state machines.** `services` transitions go through the
  service layer with `SELECT ... FOR UPDATE`; illegal transitions return
  `409 invalid_state_transition`. Never write a status directly from a handler.
- **Never return storage keys.** Only `common::storage::Storage::presign_get`
  output may leave the API. No `storage_path` / `final_asset_path` in any DTO.
- **Downloads increment atomically** with `UPDATE ... RETURNING`; never
  read-then-write.
- **Errors:** add a variant to `AppError`; do not invent ad-hoc status codes in
  a handler. Every new variant needs a row in the `API_SPEC.md` error table.
- **Migrations are append-only.** Never edit an applied file; add `000N_*.sql`.
- Use `tracing` for logs. Never log tokens, passwords, signed URLs, or PII.

---

## 4. Frontend Rules (Vue 3 + TypeScript)

- `<script setup lang="ts">` everywhere. No Options API in new code.
- **All HTTP goes through `src/api/client.ts`.** Views never call `fetch`
  directly; they call a typed client in `src/api/`.
- **Never hardcode a hex colour.** Use the CSS custom properties from
  `UI_DESIGN_SYSTEM.md` (`var(--bg-surface)`, not `#111827`).
- **Never recompute state rules.** Render buttons from the API's
  `allowed_actions` array. Do not check `status === 'under_review'` in a
  component to decide what a user may do.
- **Font:** `Anek Bangla Variable`, self-hosted from
  `assets/fonts/anek-bangla/fonts.css`. Never import from a CDN.
- **Money:** render through `<MoneyText>`; never do arithmetic on formatted
  strings; use `tabular-nums`.
- **Bengali text** follows `docs/I18N.md` — `line-height ≥ 1.6`, `lang` set,
  no `leading-none` on headings.
- Every screen ships loading, empty, error, and disabled states.

---

## 5. Commands

```bash
just up            # postgres + redis + minio
just migrate       # apply sqlx migrations
just dev           # run the API with logs
just fmt           # cargo fmt
just lint          # cargo clippy -- -D warnings
just test          # cargo test
just fe-dev        # frontend dev server
```

If `just` is unavailable: `docker compose up -d`, `sqlx migrate run`,
`cargo run`, `cargo fmt`, `cargo clippy -- -D warnings`.

---

## 6. Testing Expectations

- New service function → at least one unit test of its guard conditions.
- New endpoint → one integration test against a real Postgres (testcontainers
  or the compose database), covering the success path and the 4xx path.
- State machine changes → a test per transition, including the illegal ones.
- Money/credit changes → a test asserting the ledger row and cached balance
  move together, and roll back together on failure.

---

## 7. Commit Convention

Commits are named after the topic being worked on, in Conventional Commits form:

```
feat(auth): add registration, login and session endpoints
fix(order): block revision when the limit is reached
docs(api): document the AI generation stream
chore(deps): bump sqlx to 0.8.3
```

One topic per commit. Do not mix a refactor with a feature. Never commit
`.env`, build output, or generated binaries.

---

## 8. Definition of Done

A task is done when:

1. `just lint` and `just test` pass.
2. The behaviour matches the relevant `docs/` file.
3. New endpoints appear in `docs/API_SPEC.md`.
4. New tables appear in `docs/DATABASE.md` and a migration.
5. No secret, storage key, or signed URL is logged or returned.
6. The UI states (loading / empty / error / disabled) exist for new screens.
