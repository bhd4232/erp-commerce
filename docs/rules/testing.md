# Rule: Testing and proof

**Tier:** 2. A rule that is not tested is only a convention.

## Commands

```bash
./scripts/test.sh                 # starts a throwaway PostgreSQL if needed
TEST_THREADS=2 ./scripts/test.sh  # more parallelism
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
./scripts/check_agent_rules.sh
```

- Tests use `#[sqlx::test(migrations = "./migrations")]`; each test gets a fresh
  database and all migrations.
- Never point tests at a real, demo or shared database. Never run
  `migrate:fresh`, seeders, `DROP` or `rm -rf` against non-throwaway data.
- A test must not depend on seed data; create its own rows.

## Expectations

- New endpoint: success path + 4xx path against real PostgreSQL.
- New service function: unit test of its guards.
- State machine: one test per legal transition and one per illegal transition.
- Money/credit: ledger row and cached balance move together; rollback together.
- Tenant-owned resource: cross-tenant access returns `404`.
- Bug: the test fails before the fix and passes after.
- Frontend screen: loading, empty, error, disabled states; view it once.

## Fixtures

- `tests/fixtures/courier/` contains synthetic data only; no real customer PII.
- A fixture change must keep the universal schema valid; document the source in
  `docs/COURIER_CSV_SAMPLES.md`.
