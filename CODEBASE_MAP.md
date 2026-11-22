# Codebase Map

One screen. Read this before searching the repository; open only the module file
for the area you are changing. When the map and the code disagree, the code is
right — fix this map in the same commit.

## Runtime

```text
src/main.rs          process startup, migrations flag, listener
src/app.rs           the one HTTP router (tests use it too)
src/config.rs        environment parsing, fail fast
src/state.rs         AppState: db, config, storage
src/error.rs         AppError -> HTTP envelope
src/common/          auth, extractors, pagination, response, storage, tenancy
src/modules/         bounded contexts (one directory each)
migrations/          append-only SQL, 0001–0009
tests/               Postgres-backed integration tests + fixtures
```

## Modules

| Module | Code | Map | Owns |
| --- | --- | --- | --- |
| Auth | `src/modules/auth/` | `docs/codebase-map/auth.md` | users, register, login, me |
| Tenancy | `src/modules/tenancy/` | `docs/codebase-map/tenancy.md` | tenants, memberships, audit, team |
| Digital | `src/modules/digital/` | `docs/codebase-map/digital.md` | products, checkout, vault, downloads, refunds |
| Connectors | `src/modules/connectors/` | `docs/codebase-map/connectors.md` | WooCommerce webhook, CSV import, order ingest |
| Orders | `src/modules/orders/` | `docs/codebase-map/orders.md` | tenant-scoped order list |
| Services | `src/modules/services/` | `docs/codebase-map/services.md` | agency order pipeline, proofs, revisions |
| Finance | `src/modules/finance/` | `docs/codebase-map/finance.md` | wallet, top-ups, invoices, webhooks |
| Uploads | `src/modules/uploads/` | `docs/codebase-map/platform.md` | pre-signed upload issuance |
| Platform | `src/app.rs`, `src/common/`, `src/config.rs`, `src/state.rs`, `src/error.rs` | `docs/codebase-map/platform.md` | router, auth, storage, errors, tenancy context |

## Cross-cutting rules

| Topic | Read before |
| --- | --- |
| Branch, commit, secrets | `docs/rules/git-and-commits.md` |
| Migrations, tenant scope, transactions | `docs/rules/database-and-tenancy.md` |
| Money, wallet, credits | `docs/rules/money-and-ledger.md` |
| API envelope, errors, storage keys | `docs/rules/api-and-storage.md` |
| Vue, tokens, allowed actions, i18n | `docs/rules/frontend.md` |
| Test commands and expectations | `docs/rules/testing.md` |
| Workers, queues, retries | `docs/rules/workers.md` |
| Release notes and hand-off report | `docs/rules/release-and-reporting.md` |

## Migrations

| File | Adds |
| --- | --- |
| `0001_init_schema.sql` | users, products, orders, service orders |
| `0002_create_indexes.sql` | query indexes |
| `0003_wallet_credits.sql` | wallet provenance, AI credits, webhook log |
| `0004_finance_notifications.sql` | invoices, notifications |
| `0005_generation_jobs.sql` | AI/render jobs |
| `0006_order_numbers.sql` | collision-free order numbers |
| `0007_deliverable_source.sql` | raw preview source key |
| `0008_topups_invoices.sql` | top-up intents, invoice numbers |
| `0009_tenancy.sql` | tenants, memberships, audit events |
| `0010_connectors_orders.sql` | connectors, webhook events, customers, external orders |
| `0011_connector_rest_credentials.sql` | WooCommerce REST credentials |

## Tests

| File | Covers |
| --- | --- |
| `tests/digital_flow.rs` | checkout, vault, download limit, refund, concurrency |
| `tests/finance_flow.rs` | top-up, webhook idempotency, invoices |
| `tests/service_flow.rs` | brief → assign → proof → revision → approve/cancel |
| `tests/tenancy_flow.rs` | signup provisioning, tenant isolation, invite/accept, roles |
| `tests/connector_flow.rs` | WooCommerce webhook, replay, CSV import, order isolation |
| `tests/fixtures/courier/` | synthetic courier order/COD statement fixtures |

Run all tests with `./scripts/test.sh` (starts a throwaway PostgreSQL when one
is not running). Never point tests at a real or demo database.

## Documentation

| File | Purpose |
| --- | --- |
| `AGENTS.md` | always-loaded rules and topic index |
| `docs/agent-master-guide.md` | the method behind the rules |
| `docs/COD_PLATFORM_PLAN.md` | COD product/build plan |
| `docs/COD_MVP_CHECKLIST.md` | 12-week execution board |
| `docs/DATABASE.md` | schema and invariants |
| `docs/API_SPEC.md` | HTTP contract |
| `docs/WORKFLOWS.md` | state machines |
| `docs/WORKER_JOBS.md` | queue contracts |
| `docs/decisions.md` | dated owner decisions |
| `CHANGELOG.md` | user-facing changes |
| `UPDATE_NOTES.md` | pending commit log and verification |

## Do not

- Do not add SQL to `handlers.rs`; SQL lives in `service.rs`.
- Do not return `storage_path` / `final_asset_path`; only signed URLs leave.
- Do not edit an applied migration; add `000N_*.sql`.
- Do not add a tenant-owned query without `tenant_id`.
- Do not add a new dependency without the owner's approval.
