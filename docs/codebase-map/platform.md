# Module: Platform, Uploads and Tests

**Directories:** `src/app.rs`, `src/common/`, `src/config.rs`, `src/state.rs`,
`src/error.rs`, `src/modules/uploads/`, `migrations/`, `tests/`, `scripts/`

## Runtime and router

- `src/main.rs` — startup, env, DB pool, optional migrations, listener.
- `src/app.rs` — the single router used by `main` and integration tests.
- `src/config.rs` — fail-fast environment parsing.
- `src/state.rs` — `AppState { db, cfg, storage }`.
- `src/error.rs` — `AppError` → `{ error: { code, message, details } }`.

## Common

- `auth.rs` — Argon2 hashing, JWT issue/decode.
- `extractors.rs` — `AuthUser`, `Role`, optional auth.
- `tenant.rs` — `TenantContext`, `TenantRole`, `X-Tenant-Id` membership check.
- `pagination.rs` — page/per-page clamps.
- `response.rs` — `{ data, meta }` envelope.
- `storage.rs` — private S3/R2 client; only `presign_get` output leaves.

## Uploads

`POST /uploads/presign` issues a pre-signed PUT after an allow-list check.
Storage keys never enter a response except through a signed URL.

## Migrations

Append-only, `0001`–`0009`. Never edit an applied file. Every new tenant-owned
table carries `tenant_id`; indexes include it.

## Tests and fixtures

- `tests/*.rs` use `#[sqlx::test(migrations = "./migrations")]`; each test gets a
  fresh database.
- `tests/fixtures/courier/` contains synthetic courier and universal COD
  statement fixtures. Never put real customer PII there.
- `scripts/test.sh` starts a throwaway PostgreSQL when needed.
- `scripts/check_agent_rules.sh` enforces the static agent-rule guards in CI.

## CI

`.github/workflows/ci.yml` runs fmt, clippy, migrations and tests; the Docker
job builds the image. Add the agent guard script to CI when changing rules.
