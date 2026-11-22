# Changelog

## Unreleased

### Added

- Tenancy foundation: `tenants`, `memberships`, `audit_events`, tenant context,
  team invite/accept, roles and isolation tests.
- COD platform plan, MVP checklist, courier CSV fixtures and the agent master
  guide.
- Agent rule system: `CODEBASE_MAP.md`, per-module maps, `docs/rules/`,
  `docs/decisions.md`, and `scripts/check_agent_rules.sh` in CI.

### Changed

- Signup now creates a tenant and an owner membership in the same transaction.
- Agent rules are split into a short always-loaded file and per-topic files.
- Router construction moved to the library so integration tests exercise the
  real HTTP surface.

### Security

- Tenant-scoped requests require an active membership; a foreign tenant returns
  `404`.
