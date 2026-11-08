# Storage, Bucket Policies & Security Design

## 1. Bucket Isolation Architecture

All assets reside in a single private S3/R2 bucket with **zero public read permissions**. There is no CDN in front of the bucket for originals and no anonymous access of any kind.

```
bucket-root/
├── protected-products/
│   └── {product_id}/
│       └── {version}/
│           └── asset.zip
├── service-deliverables/
│   └── {order_id}/
│       ├── raw/            <-- Strictly private, only assigned staff/admin can access
│       ├── previews/       <-- Watermarked files exposed to clients during review
│       └── final/          <-- Unlocked only when order status is 'completed'
└── user-uploads/
    └── {user_id}/
        └── briefs/         <-- Client-submitted brand assets, logos, references
```

### 1.1 Storage-Key Rules

- Keys are generated **server-side**. The client never chooses a key, only uploads to a key the server issued.
- Keys are opaque: no user-supplied filenames, no `..`, no absolute paths.
- Keys are stored as `storage_path` in the database and are never sent to a client.
- The database is the source of truth; the bucket is dumb storage.

---

## 2. Permission Matrix

| Actor | `protected-products/` | `service-deliverables/.../raw/` | `.../previews/` | `.../final/` | `user-uploads/{own}/` | `user-uploads/{other}/` |
| --- | --- | --- | --- | --- | --- | --- |
| **Client (owner)** | read via signed URL, if entitled | ✗ | read via signed URL, `under_review` / `revision_requested` | read via signed URL, only after `completed` | write via pre-signed PUT | ✗ |
| **Staff (assigned)** | ✗ | read/write | read/write | read/write | ✗ | ✗ |
| **Staff (unassigned)** | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Admin** | read/write | read/write | read/write | read/write | read | read |
| **Worker** | read | read/write | write | read/write | read | read |

"IAM" in this table means the backend decides and signs; the bucket policy itself grants nothing to end users. Only the API/worker credentials touch the bucket directly.

### 2.1 Bucket Policy Skeleton

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "DenyAllAnonymous",
      "Effect": "Deny",
      "Principal": "*",
      "Action": "s3:*",
      "Resource": ["arn:aws:s3:::bucket-root", "arn:aws:s3:::bucket-root/*"],
      "Condition": { "StringEquals": { "aws:PrincipalType": "Anonymous" } }
    }
  ]
}
```

Bucket-level: block public ACLs, block public policy, disable website hosting, enforce TLS-only requests, enable versioning, enable default encryption at rest.

---

## 3. Pre-Signed URL Specification

### 3.1 Issuance Rules

- TTL is **900 seconds (15 minutes)**. Never longer, never configurable per request.
- Signed with SigV4 using an API-scoped credential that can only `GetObject` on the specific key.
- Always include response overrides:
  - `response-content-disposition: attachment; filename="<safe-name>"`
  - `response-content-type: <mime_type from product_files>`
- The signed URL is generated per request. It is never persisted, cached, or reused.
- The response contains only `download_url`, `expires_at`, `filename` — never the raw key or bucket host path.

### 3.2 Pre-Signed Upload

For client brief uploads:

1. Client calls `POST /uploads/presign` with `{ purpose, filename, content_type, size_bytes }`.
2. Server validates purpose, extension, MIME against an allow-list, and size limit.
3. Server returns `{ upload_url, key, expires_at }` with a 300 s TTL and a `content-length-range` policy.
4. Client PUTs directly to storage.
5. Client submits the key with the brief; the server verifies the object exists and the size/type match before accepting it.

Never trust a client-submitted key without verifying the object exists and lives under the caller's own prefix.

---

## 4. Watermarking & Security Rules

- Preview files for video/images must be processed via background Rust workers applying a dynamic semi-transparent watermark overlay (`PROOFS ONLY - [ORDER_NUMBER]`) before being moved to `previews/`.
- Watermarks must be burned into the pixels, not applied as a removable CSS overlay or metadata.
- Watermark placement must survive cropping or must be tiled.
- No direct public S3 URLs are ever exposed to the client. All download links must be issued on-the-fly using short-lived AWS SDK SigV4 signed URLs.
- The unwatermarked original stays in `raw/` and is only signed for the **assigned staff** or **admin**.
- `final/` is signed for the client only when `service_orders.status = 'completed'` **and** the deliverable is approved.

### 4.1 Fail-Closed Behaviour

- If watermarking fails after max retries, the order stays `in_progress`; the client sees nothing.
- If a signed URL cannot be produced, return `500 internal_error` rather than falling back to a public link.
- If the storage key is missing, return `404 not_found`; do not leak whether the object ever existed.

---

## 5. Audit & Retention

- Every signed download writes `audit_download_logs` (entitlement, IP, user agent, timestamp). The write is asynchronous but must not be dropped; use a durable queue with retry.
- Service deliverable access (preview and final) is logged to the same audit stream with `reference_type = 'deliverable'`.
- Audit rows older than 365 days may be archived; they are never deleted while the related entitlement or order is live.
- Object versioning is enabled so an accidental overwrite in `final/` can be recovered.
- Orphan sweep: objects with no database row older than 24 h are removed on a schedule.

---

## 6. Credential & Key Management

| Secret | Where it lives | Rotation |
| --- | --- | --- |
| Bucket access key | Backend env / secrets manager | 90 days |
| JWT signing key | Backend env / secrets manager | 90 days, dual-key overlap |
| Database URL | Backend env / secrets manager | on demand |
| Redis URL | Backend env / secrets manager | on demand |

- Never ship storage credentials to the SPA or Tauri client.
- The Tauri app holds only the user's JWT, stored in the OS keychain, not in plain local storage.
- Logs redact `Authorization`, cookies, and any query string containing `X-Amz-Signature`.

---

## 7. Threat Checklist

| Threat | Mitigation |
| --- | --- |
| Guessing another user's `entitlement_id` | Ownership check on every download |
| Sharing a signed URL | Short TTL, per-request signing, audit trail |
| Bypassing revision limit | `SELECT ... FOR UPDATE` + DB `CHECK` |
| Viewing final assets before approval | Status gate before signing `final/` |
| Uploading a malicious file to `briefs/` | MIME + extension + size allow-list, virus scan in worker |
| Path traversal via filename | Server-generated keys only |
| Leaked signing key | Scoped `GetObject`-only credential, 15-min URLs, rotation |
