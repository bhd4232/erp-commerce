# Module: Tenancy

**Directory:** `src/modules/tenancy/`  
**Tables:** `tenants`, `memberships`, `audit_events`  
**Endpoints:** `/tenants`, `/tenants/current`, `/team`, `/team/invite`,
`/team/invitations`, `/team/invitations/{id}/accept`, `/team/{user_id}`

## Files

- `models.rs` — team/invitation query rows.
- `dtos.rs` — tenant summary, team member, invitation, invite/update requests.
- `service.rs` — provisioning, member listing, invite/accept, role updates,
  audit writes.
- `handlers.rs` — HTTP only.
- `src/common/tenant.rs` — `TenantContext`, `TenantRole`, `X-Tenant-Id`
  extraction and membership check.

## Invariants

- A tenant is one client business; `users` are global.
- `memberships` is unique per `(tenant_id, user_id)`.
- Only `owner`/`admin` can invite or change roles.
- Owners cannot be changed through the member update endpoint.
- A user cannot change their own membership.
- An admin cannot change another admin.
- Missing `X-Tenant-Id` is `400`; unknown/foreign tenant is `404`.
- Every signup, invite, accept and role change writes `audit_events`.

## Tests

- `tests/tenancy_flow.rs` — signup, isolation, invite/accept, role guards,
  duplicate-signup rollback.
