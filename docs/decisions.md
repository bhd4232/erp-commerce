# Decisions

Dated owner decisions, newest first. A rule's long reason lives here when it
does not fit in `AGENTS.md`.

## 2026-11-22 — Agent rules follow the master guide

- The always-loaded rule file is short; per-topic details live in `docs/rules/`.
- Tier 1 rules must be enforced outside the agent. Branch protection is still an
  owner action: the GitHub `main` branch is currently unprotected.
- A guard script (`scripts/check_agent_rules.sh`) runs in CI for the rules it
  can prove statically.
- One topic per branch, one agent per branch at a time.

## 2026-11-22 — COD platform Phase 1: tenancy

- Tenant = one client business; `users` stay global and join through
  `memberships`.
- Only new COD tables carry `tenant_id`; existing digital/service tables are
  intentionally untouched in this phase.
- `X-Tenant-Id` must resolve to an active membership. Missing header is `400`;
  a foreign/unknown tenant is `404`.
- Signup creates user + tenant + owner membership + audit row in one
  transaction.
- Team invites only work for users who already have an account. Email invites
  are later.

## 2026-11-22 — COD platform scope decisions

- First connector: **WooCommerce**.
- WhatsApp confirmation: **Cloud API direct**, with an adapter for a later BSP.
- Courier reconciliation: **CSV + API**. CSV is the guaranteed fallback; API
  connectors are added one courier at a time.
- Cross-tenant risk data: **tenant-only in v1**; aggregate later only with
  consent.
- Pilot billing: **free pilot, billing later**.
- Existing digital/service tables: **not tenant-scoped in v1**.

## 2026-11-22 — COD is the first commercial wedge

The first product track is COD Profit Recovery, not a broad storefront/CRM/
booking suite. The plan is `docs/COD_PLATFORM_PLAN.md`; the execution board is
`docs/COD_MVP_CHECKLIST.md`. The old P2–P8 agency/SaaS sections in
`docs/ROADMAP.md` are legacy context until Phase 0 rewrites them.

## 2026-11-09 — Digital commerce and finance contracts

Wallet money is ledger-first: every cached balance change is written with the
matching `wallet_transactions` row in one transaction. The same rule applies to
AI credits. This is a Tier 1 rule; see `docs/rules/money-and-ledger.md`.
