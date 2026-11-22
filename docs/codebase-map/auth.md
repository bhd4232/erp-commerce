# Module: Auth

**Directory:** `src/modules/auth/`  
**Tables:** `users`  
**Endpoints:** `POST /auth/register`, `POST /auth/login`, `GET /auth/me`

## Files

- `models.rs` — `User` row, `UserRole` enum.
- `dtos.rs` — request/response projections; never exposes `password_hash`.
- `service.rs` — validation, Argon2 hashing, login, tenant provisioning on
  signup.
- `handlers.rs` — HTTP only.

## Invariants

- Email is lowercased and unique.
- Password minimum is 12 characters; hashes are Argon2id.
- Login returns the same error for unknown email and wrong password.
- `users.is_active = false` is rejected.
- Registration creates the user, tenant, owner membership and audit row in one
  transaction.
- JWT contains only user id, global role, issued-at and expiry.

## Tests

- `tests/tenancy_flow.rs` — signup provisioning, duplicate-signup rollback.
