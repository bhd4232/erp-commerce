# Database Schema & State Transition Specification

Target: PostgreSQL 16+. All timestamps are `TIMESTAMPTZ`. All money columns are `NUMERIC(12,2)` and never floats.

---

## 1. Extensions & Enums

```sql
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Identity
CREATE TYPE user_role AS ENUM ('admin', 'staff', 'client');

-- Digital products
CREATE TYPE digital_product_type AS ENUM ('premade_file', 'license_key', 'dynamic_tool');

-- Checkout
CREATE TYPE order_status AS ENUM ('pending', 'paid', 'failed', 'refunded', 'cancelled');
CREATE TYPE payment_provider AS ENUM ('wallet', 'card', 'manual');
CREATE TYPE wallet_txn_type AS ENUM ('credit', 'debit', 'refund', 'adjustment');

-- Agency services
CREATE TYPE service_status AS ENUM (
    'pending_brief',
    'in_queue',
    'in_progress',
    'under_review',
    'revision_requested',
    'approved',
    'completed',
    'cancelled'
);
```

---

## 2. Core Identity & RBAC

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    full_name VARCHAR(100) NOT NULL,
    role user_role DEFAULT 'client' NOT NULL,
    wallet_balance NUMERIC(12, 2) DEFAULT 0.00 NOT NULL,
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT users_wallet_non_negative CHECK (wallet_balance >= 0)
);
```

### 2.1 Wallet Ledger

`users.wallet_balance` is a cached projection. The ledger is the source of truth; both must change inside the same transaction.

```sql
CREATE TABLE wallet_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    txn_type wallet_txn_type NOT NULL,
    -- Always positive. Direction comes from txn_type, never from the sign.
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    balance_after NUMERIC(12, 2) NOT NULL CHECK (balance_after >= 0),
    reference_type VARCHAR(50),          -- 'digital_order' | 'service_order' | 'manual'
    reference_id UUID,
    note TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_wallet_txn_user_created ON wallet_transactions(user_id, created_at DESC);
```

---

## 3. Digital Products Domain

```sql
CREATE TABLE digital_products (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(255) UNIQUE NOT NULL,
    product_type digital_product_type NOT NULL,
    price NUMERIC(10, 2) NOT NULL CHECK (price >= 0),
    description TEXT,
    thumbnail_url TEXT,
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE TABLE product_files (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL REFERENCES digital_products(id) ON DELETE CASCADE,
    version VARCHAR(30) DEFAULT '1.0.0' NOT NULL,
    storage_path TEXT NOT NULL,
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes >= 0),
    mime_type VARCHAR(100) NOT NULL,
    checksum_sha256 CHAR(64),
    is_current BOOLEAN DEFAULT false NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT product_files_unique_version UNIQUE (product_id, version)
);
```

A product must have at most one current file version:

```sql
CREATE UNIQUE INDEX idx_product_files_current
    ON product_files(product_id) WHERE is_current;
```

```sql
CREATE TABLE user_entitlements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES digital_products(id),
    order_ref VARCHAR(100) NOT NULL,
    download_count INT DEFAULT 0 NOT NULL CHECK (download_count >= 0),
    -- -1 means unlimited
    max_downloads INT DEFAULT -1 NOT NULL,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT user_entitlements_unique_grant
        UNIQUE (user_id, product_id, order_ref)
);

CREATE TABLE audit_download_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entitlement_id UUID NOT NULL REFERENCES user_entitlements(id) ON DELETE CASCADE,
    ip_address VARCHAR(45) NOT NULL,
    user_agent TEXT,
    downloaded_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);
```

### 3.1 Checkout (required before an entitlement can exist)

An entitlement may only be created by a paid order or an explicit admin grant. Both paths write `order_ref`, so nothing is unaccounted for.

```sql
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_number VARCHAR(50) UNIQUE NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id),
    status order_status DEFAULT 'pending' NOT NULL,
    currency CHAR(3) DEFAULT 'USD' NOT NULL,
    total_amount NUMERIC(12, 2) NOT NULL CHECK (total_amount >= 0),
    idempotency_key VARCHAR(100) UNIQUE,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE TABLE order_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES digital_products(id),
    unit_price NUMERIC(10, 2) NOT NULL CHECK (unit_price >= 0),
    quantity INT DEFAULT 1 NOT NULL CHECK (quantity > 0),
    CONSTRAINT order_items_unique_product UNIQUE (order_id, product_id)
);

CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    provider payment_provider NOT NULL,
    provider_ref VARCHAR(255),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount >= 0),
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE UNIQUE INDEX idx_payments_provider_ref
    ON payments(provider, provider_ref) WHERE provider_ref IS NOT NULL;
```

---

## 4. Agency Services Domain

```sql
CREATE TABLE agency_services (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(255) UNIQUE NOT NULL,
    base_price NUMERIC(10, 2) NOT NULL CHECK (base_price >= 0),
    brief_template JSONB DEFAULT '{}'::jsonb NOT NULL,
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE TABLE service_orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_number VARCHAR(50) UNIQUE NOT NULL,
    client_id UUID NOT NULL REFERENCES users(id),
    service_id UUID NOT NULL REFERENCES agency_services(id),
    assigned_staff_id UUID REFERENCES users(id),
    status service_status DEFAULT 'pending_brief' NOT NULL,
    price_charged NUMERIC(10, 2) NOT NULL CHECK (price_charged >= 0),
    revision_limit INT DEFAULT 2 NOT NULL CHECK (revision_limit >= 0),
    revisions_used INT DEFAULT 0 NOT NULL CHECK (revisions_used >= 0),
    due_at TIMESTAMPTZ,
    cancelled_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT service_orders_revision_budget CHECK (revisions_used <= revision_limit)
);
```

```sql
CREATE TABLE order_briefs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID UNIQUE NOT NULL REFERENCES service_orders(id) ON DELETE CASCADE,
    payload JSONB NOT NULL,
    submitted_by UUID NOT NULL REFERENCES users(id),
    submitted_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE TABLE order_deliverables (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES service_orders(id) ON DELETE CASCADE,
    version INT NOT NULL CHECK (version > 0),
    watermarked_preview_path TEXT,
    final_asset_path TEXT NOT NULL,
    notes TEXT,
    uploaded_by UUID NOT NULL REFERENCES users(id),
    is_approved BOOLEAN DEFAULT false NOT NULL,
    approved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT order_deliverables_unique_version UNIQUE (order_id, version)
);

CREATE TABLE order_revisions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES service_orders(id) ON DELETE CASCADE,
    deliverable_id UUID NOT NULL REFERENCES order_deliverables(id),
    requested_by UUID NOT NULL REFERENCES users(id),
    client_feedback TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);
```

---

## 5. Indexes

```sql
-- Digital domain
CREATE INDEX idx_entitlements_user            ON user_entitlements(user_id);
CREATE INDEX idx_entitlements_user_product    ON user_entitlements(user_id, product_id);
CREATE INDEX idx_product_files_product        ON product_files(product_id);
CREATE INDEX idx_audit_download_entitlement   ON audit_download_logs(entitlement_id, downloaded_at DESC);
CREATE INDEX idx_download_logs_created        ON audit_download_logs(downloaded_at DESC);
CREATE INDEX idx_orders_user_created          ON orders(user_id, created_at DESC);
CREATE INDEX idx_payments_order               ON payments(order_id);

-- Agency domain
CREATE INDEX idx_service_orders_status        ON service_orders(status);
CREATE INDEX idx_service_orders_client        ON service_orders(client_id);
CREATE INDEX idx_service_orders_staff         ON service_orders(assigned_staff_id);
-- Kanban board query: "everything in this column, oldest first"
CREATE INDEX idx_service_orders_board
    ON service_orders(status, updated_at) WHERE status <> 'completed';
CREATE INDEX idx_deliverables_order           ON order_deliverables(order_id, version DESC);
CREATE INDEX idx_revisions_order              ON order_revisions(order_id, created_at DESC);

-- Catalogue
CREATE INDEX idx_digital_products_active
    ON digital_products(created_at DESC) WHERE is_active;
CREATE INDEX idx_agency_services_active
    ON agency_services(created_at DESC) WHERE is_active;
```

---

## 6. Entity Relationships

```
users 1───N orders 1───N order_items N───1 digital_products
users 1───N user_entitlements N───1 digital_products
user_entitlements 1───N audit_download_logs
digital_products 1───N product_files

users(client) 1───N service_orders N───1 agency_services
users(staff)  1───N service_orders            (assigned_staff_id)
service_orders 1───1 order_briefs
service_orders 1───N order_deliverables 1───N order_revisions
```

---

## 7. Atomicity & Concurrency Rules

These are enforced in the service layer inside a single transaction; the `CHECK` constraints above are only a backstop.

1. **Checkout → entitlement**
   `UPDATE orders SET status = 'paid'` and every `INSERT INTO user_entitlements` happen in one transaction. `idempotency_key` prevents double-granting on client retry.
2. **Download counter**
   Never read-then-write. Increment and validate in one statement:
   ```sql
   UPDATE user_entitlements
      SET download_count = download_count + 1
    WHERE id = $1
      AND (expires_at IS NULL OR expires_at > NOW())
      AND (max_downloads = -1 OR download_count < max_downloads)
   RETURNING download_count, max_downloads;
   ```
   Zero rows returned means "denied" — respond `410` for expiry, `429` for limit, without a second query racing the first.
3. **Wallet debit**
   ```sql
   UPDATE users SET wallet_balance = wallet_balance - $1
    WHERE id = $2 AND wallet_balance >= $1
   RETURNING wallet_balance;
   ```
   Insert the matching `wallet_transactions` row in the same transaction using the returned `balance_after`.
4. **Revision budget**
   `POST /revision` must re-read the order row `FOR UPDATE`, check `revisions_used < revision_limit`, then increment. Concurrent double-click cannot exceed the limit.
5. **Order state safety**
   All transitions use `SELECT ... FOR UPDATE` on `service_orders` before validating the current status.

---

## 8. Deployment Notes

- Enable `pg_stat_statements`; slow query budget is 50 ms p95 for board endpoints.
- Run migrations with a single-instance advisory lock (`pg_advisory_lock`) to avoid parallel DDL.
- Soft retention: `audit_download_logs` older than 365 days can be archived; never deleted while the entitlement is live.
- Backups: daily base backup + WAL archiving; point-in-time recovery target ≤ 5 minutes.

---

## 9. Wallet, Credits & Finance Extensions

Added by `migrations/0003_wallet_credits.sql`,
`0004_finance_notifications.sql`, and `0005_generation_jobs.sql`. Section 2.1's
`wallet_transactions` contract is unchanged: `amount` stays positive and
direction stays in `txn_type`.

### 9.1 Payments & AI credits

```sql
-- users
ai_credits INTEGER NOT NULL DEFAULT 0 CHECK (ai_credits >= 0)

-- wallet_transactions (existing table, new columns)
gateway      VARCHAR(50)   -- 'bkash' | 'sslcommerz' | 'stripe' | 'internal'
gateway_ref  VARCHAR(255)
currency     CHAR(3) NOT NULL DEFAULT 'USD'

-- Idempotency: one ledger credit per gateway transaction.
CREATE UNIQUE INDEX idx_wallet_txn_gateway_ref
  ON wallet_transactions(gateway, gateway_ref) WHERE gateway_ref IS NOT NULL;

CREATE TYPE ai_credit_txn_type AS ENUM ('grant','topup','spend','refund','adjustment');

ai_credit_transactions(
  id UUID PK,
  user_id UUID -> users(id) ON DELETE CASCADE,
  txn_type ai_credit_txn_type,
  credits INTEGER > 0,              -- always positive, direction from txn_type
  balance_after INTEGER >= 0,
  reference_type VARCHAR(50),       -- 'generation_job' | 'purchase' | 'manual'
  reference_id UUID,
  note TEXT,
  created_at TIMESTAMPTZ
)
```

`users.ai_credits` is a cached projection of `ai_credit_transactions`; the two
are written in one transaction. See `BILLING_WALLET_SPEC.md` §3–4.

### 9.2 Webhooks

```sql
CREATE TYPE webhook_status AS ENUM ('received','verified','processed','rejected','failed');

payment_webhooks(
  id UUID PK,
  provider VARCHAR(50),
  event_id VARCHAR(255),
  event_type VARCHAR(100),
  payload JSONB,
  signature_valid BOOLEAN,
  status webhook_status,
  processed_at TIMESTAMPTZ,
  error TEXT,
  created_at TIMESTAMPTZ,
  UNIQUE (provider, event_id)       -- idempotency key
)
```

Duplicate `(provider, event_id)` → `200`, no ledger change.

### 9.3 Invoices & notifications

```sql
CREATE TYPE invoice_status AS ENUM ('draft','open','paid','void','refunded');

invoices(
  id UUID PK,
  invoice_number VARCHAR(50) UNIQUE,   -- INV-YYYYMM-000001
  user_id UUID -> users(id),
  order_id UUID -> orders(id),
  currency CHAR(3) DEFAULT 'USD',
  subtotal NUMERIC(12,2) >= 0,
  tax_amount NUMERIC(12,2) >= 0,
  total NUMERIC(12,2) >= 0,
  status invoice_status DEFAULT 'open',
  issued_at TIMESTAMPTZ,
  paid_at TIMESTAMPTZ,
  pdf_path TEXT,                        -- storage key, never returned
  created_at TIMESTAMPTZ
)

CREATE TYPE notification_kind AS ENUM
  ('order_status','deliverable_ready','revision_requested','download','payment','system');

notifications(
  id UUID PK,
  user_id UUID -> users(id) ON DELETE CASCADE,
  kind notification_kind,
  title VARCHAR(200),
  body TEXT,
  reference_type VARCHAR(50),
  reference_id UUID,
  read_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ
)
```

Invoices are append-only. `pdf_path` is signed on read with a 900 s TTL
(`STORAGE_SECURITY.md` §3).

### 9.4 Generation jobs

```sql
CREATE TYPE generation_tool   AS ENUM ('promo_banner','ad_copy','packing_slip');
CREATE TYPE generation_status AS ENUM ('queued','running','succeeded','failed','cancelled');

generation_jobs(
  id UUID PK,
  user_id UUID -> users(id) ON DELETE CASCADE,
  tool generation_tool,
  status generation_status DEFAULT 'queued',
  params JSONB,
  credits_charged INTEGER >= 0,
  output_path TEXT,                     -- storage key, never returned
  output_mime VARCHAR(100),
  error TEXT,
  idempotency_key VARCHAR(100),
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ,
  UNIQUE (user_id, idempotency_key)     -- safe retries, no double charge
)
```

Job lifecycle and refund rules: `AI_GENERATOR_PIPELINE.md` §2–3.

### 9.5 New indexes

| Index | Purpose |
| --- | --- |
| `idx_wallet_txn_gateway_ref` (partial unique) | gateway idempotency |
| `idx_ai_credit_txn_user` | ledger pagination |
| `idx_ai_credit_txn_ref` (partial) | refund lookup by job |
| `idx_payment_webhooks_status` | webhook worker scan |
| `idx_invoices_user_issued` | billing list |
| `idx_invoices_open` (partial) | unpaid invoice sweep |
| `idx_notifications_unread` (partial) | unread badge |
| `idx_generation_jobs_active` (partial) | reaper + queue depth |

### 9.6 Invariants added

1. `users.ai_credits` = derived balance of `ai_credit_transactions`.
2. `(gateway, gateway_ref)` is unique when `gateway_ref` is present — a gateway
   transaction can never credit the wallet twice.
3. `(provider, event_id)` is unique in `payment_webhooks`.
4. A `generation_jobs` row and its `spend` ledger row are created in one
   transaction; a failed job and its `refund` row are also one transaction.
5. `invoices` are never deleted or updated after `paid`, except
   `paid → refunded`.

### 9.7 Order numbers

```sql
CREATE SEQUENCE order_number_seq START 1;

next_order_number('DIG')  -- DIG-20260108-0001
```

`migrations/0006_order_numbers.sql`. A sequence, not `COUNT(*) + 1`: two
concurrent checkouts must never compute the same next number. The prefix
identifies the domain (`DIG` for digital, `SVC` for service orders). `to_char`
is used instead of `lpad`, which silently truncates once the counter passes
9999.

### 9.8 Deliverable source key

```sql
ALTER TABLE order_deliverables ADD COLUMN source_preview_path TEXT;
```

`source_preview_path` is the raw upload (`.../raw/`), read only by the
watermark worker. `watermarked_preview_path` stays NULL until the worker has
written the safe copy, so `GET /services/orders/{id}` can never sign the raw
file. `final_asset_path` is only signed after `completed` + `is_approved`.


### 9.9 Top-up intents and invoice numbers

```sql
CREATE TYPE topup_status AS ENUM ('pending', 'paid', 'expired', 'cancelled');

topup_intents(
  id UUID PK, user_id UUID -> users(id), amount NUMERIC(12,2) > 0,
  currency CHAR(3) DEFAULT 'USD', provider VARCHAR(50),
  status topup_status DEFAULT 'pending', provider_ref VARCHAR(255),
  expires_at TIMESTAMPTZ, paid_at TIMESTAMPTZ, created_at TIMESTAMPTZ
)
```

A top-up is a gateway checkout in flight; the wallet is credited only by the
webhook handler, inside one transaction with the `credit` ledger row, the
`paid` status and the invoice.

```sql
CREATE SEQUENCE invoice_number_seq START 1;
next_invoice_number()  -- INV-202611-000001
```

Invoices are append-only and issued per paid top-up (and later per paid order).
`pdf_path` stays nullable until the PDF worker exists; the API returns a signed
`pdf_url` only when it is set.
