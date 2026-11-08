-- 0001_init_schema.sql
-- Core schema for the ERP + Digital Commerce platform.
-- Mirrors docs/DATABASE.md sections 1-4. Keep both in sync.

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ---------------------------------------------------------------------------
-- Enums
-- ---------------------------------------------------------------------------

CREATE TYPE user_role AS ENUM ('admin', 'staff', 'client');

CREATE TYPE digital_product_type AS ENUM ('premade_file', 'license_key', 'dynamic_tool');

CREATE TYPE order_status AS ENUM ('pending', 'paid', 'failed', 'refunded', 'cancelled');
CREATE TYPE payment_provider AS ENUM ('wallet', 'card', 'manual');
CREATE TYPE wallet_txn_type AS ENUM ('credit', 'debit', 'refund', 'adjustment');

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

-- ---------------------------------------------------------------------------
-- Identity & wallet
-- ---------------------------------------------------------------------------

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

CREATE TABLE wallet_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    txn_type wallet_txn_type NOT NULL,
    -- Always positive. Direction is derived from txn_type, never from the sign.
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    balance_after NUMERIC(12, 2) NOT NULL CHECK (balance_after >= 0),
    reference_type VARCHAR(50),
    reference_id UUID,
    note TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- ---------------------------------------------------------------------------
-- Digital products
-- ---------------------------------------------------------------------------

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

-- ---------------------------------------------------------------------------
-- Checkout: an entitlement may only come from a paid order
-- ---------------------------------------------------------------------------

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

-- ---------------------------------------------------------------------------
-- Agency services
-- ---------------------------------------------------------------------------

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
