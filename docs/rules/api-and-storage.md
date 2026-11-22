# Rule: API, errors and storage

**Tier:** 2 for contract/guard tests; 1 for storage keys and secrets.

## HTTP contract

- Every response uses `{ "data": ..., "meta": ... }`; errors use
  `{ "error": { "code", "message", "details" } }`.
- A new endpoint is not done until it appears in `docs/API_SPEC.md` with success
  and 4xx responses.
- Add an `AppError` variant instead of inventing a status code in a handler.
- Handlers do HTTP only: no SQL, no business rules.
- SQL lives in `service.rs`; reuse a private function before creating a module.
- List endpoints are paginated; `per_page` is capped.

## Storage and secrets

- Storage is private; only `Storage::presign_get` output leaves the API.
- No DTO may contain `storage_path`, `final_asset_path`, or a bucket key.
- Signed URLs are short-lived and never stored in a client store or
  `localStorage`.
- Never log tokens, passwords, full phone numbers, addresses, signed URLs, or
  secrets.
- Never trust a client-supplied tenant id, user id, or storage key.
- Webhooks verify a signature and are idempotent on the provider event id.

## Guard

`scripts/check_agent_rules.sh` fails when a handler imports `sqlx`, a handler
uses `unwrap`/`expect`, or a DTO exposes a storage key. Do not weaken it.
