# Rule: Database, migrations, tenancy and state

**Tier:** 1 for migrations and tenant isolation; 2 for transaction/state guards.

## Migrations

- Append-only: never edit an applied file; add `000N_*.sql`.
- One migration changes one topic.
- Every migration applies cleanly on PostgreSQL 16 from an empty database and
  from the previous head.
- Every tenant-owned table has `tenant_id`, an index that starts with
  `tenant_id`, and a unique constraint scoped to the tenant where relevant.
- New table/column goes into `docs/DATABASE.md` in the same commit.

## Tenancy

- A tenant is one client business; `users` are global.
- `memberships` is the authorization boundary; roles are `owner`, `admin`,
  `staff`.
- Tenant-scoped routes require `X-Tenant-Id`. The server validates an active
  membership. Missing header is `400`; unknown or foreign tenant is `404`
  (never `403`, so ids are not enumerable).
- Every query on tenant-owned data filters `tenant_id`. A query without it is a
  defect even if the test passes.
- A cross-tenant access test must exist for every new tenant-owned resource.
- Never trust a tenant id from a request body; derive it from the authenticated
  context.

## Transactions and state

- Every money, credit or multi-row mutation runs in one transaction.
- State machines lock the row with `SELECT ... FOR UPDATE`; an illegal
  transition returns `409 invalid_state_transition`.
- Never write a status directly from a handler.
- Every state or money decision writes an `audit_events` row where the domain
  has one.
- Concurrent paths get an integration test (two requests, one winner, loser
  rolled back).
