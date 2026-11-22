# Update Notes

## 2026-11-22 — Tenancy Phase 1 + agent rule system

**Reason:** start the COD platform's tenancy slice and reorganize the agent
instructions around the master guide.

**Important files:**

- `migrations/0009_tenancy.sql`
- `migrations/0010_connectors_orders.sql`
- `migrations/0011_connector_rest_credentials.sql`
- `src/modules/tenancy/`, `src/modules/connectors/`, `src/modules/orders/`
- `src/common/tenant.rs`, `src/app.rs`
- `tests/tenancy_flow.rs`, `tests/connector_flow.rs`
- `src/config.rs`, `.env.example`, `.env.production.example` (connector key)
- `AGENTS.md`, `CODEBASE_MAP.md`, `docs/rules/`, `docs/decisions.md`
- `docs/agent-master-guide.md`, `docs/codebase-map/`
- `scripts/check_agent_rules.sh`, `.github/workflows/ci.yml`
- `docs/COD_PLATFORM_PLAN.md`, `docs/COD_MVP_CHECKLIST.md`

**Verification:**

- `cargo fmt --all -- --check` — pass
- `cargo clippy --all-targets --all-features -- -D warnings` — pass
- `./scripts/test.sh` — 18 unit + 29 integration tests pass
- Tenancy tests: 5 pass, including duplicate-signup rollback and cross-tenant
  `404`
- Connector tests: 4 pass, including signed webhook, replay, bad signature, CSV
  import and cross-tenant isolation
- `./scripts/check_agent_rules.sh` — pass

**Not verified:** the Vue frontend shell for onboarding/team (not built yet);
GitHub branch protection (owner action required).

**Commit status:** uncommitted working tree.
