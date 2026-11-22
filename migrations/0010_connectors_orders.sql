-- 0010_connectors_orders.sql
-- Store connectors and the normalized order stream.
--
-- Secrets are encrypted with pgcrypto using CONNECTOR_ENCRYPTION_KEY; the API
-- never returns an encrypted value or a stored secret except once at creation.

CREATE TYPE connector_kind AS ENUM ('woocommerce', 'laravel', 'csv');
CREATE TYPE connector_status AS ENUM ('active', 'paused', 'error');

CREATE TABLE connectors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    kind connector_kind NOT NULL,
    name VARCHAR(120) NOT NULL,
    status connector_status NOT NULL DEFAULT 'active',
    webhook_secret_encrypted BYTEA NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT connectors_unique_name UNIQUE (tenant_id, kind, name)
);

CREATE INDEX idx_connectors_tenant
    ON connectors(tenant_id, status, created_at);

CREATE TABLE connector_webhook_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    connector_id UUID NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
    event_id VARCHAR(255) NOT NULL,
    topic VARCHAR(100),
    payload JSONB NOT NULL,
    status webhook_status NOT NULL DEFAULT 'received',
    error TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    CONSTRAINT connector_webhook_events_unique UNIQUE (connector_id, event_id)
);

CREATE INDEX idx_connector_webhook_events_status
    ON connector_webhook_events(status, received_at);

CREATE TABLE customers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    phone_normalized VARCHAR(20),
    email VARCHAR(255),
    name VARCHAR(200),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_customers_tenant_phone
    ON customers(tenant_id, phone_normalized)
    WHERE phone_normalized IS NOT NULL;
CREATE UNIQUE INDEX idx_customers_tenant_email
    ON customers(tenant_id, email)
    WHERE email IS NOT NULL;

CREATE TABLE external_orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    connector_id UUID NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
    external_id VARCHAR(100) NOT NULL,
    order_number VARCHAR(100) NOT NULL,
    customer_id UUID REFERENCES customers(id) ON DELETE SET NULL,
    status VARCHAR(40) NOT NULL,
    payment_method VARCHAR(50),
    total_amount NUMERIC(12, 2) NOT NULL DEFAULT 0 CHECK (total_amount >= 0),
    cod_amount NUMERIC(12, 2) NOT NULL DEFAULT 0 CHECK (cod_amount >= 0),
    currency CHAR(3) NOT NULL DEFAULT 'BDT',
    placed_at TIMESTAMPTZ NOT NULL,
    raw JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT external_orders_unique_external
        UNIQUE (tenant_id, connector_id, external_id)
);

CREATE INDEX idx_external_orders_tenant_placed
    ON external_orders(tenant_id, placed_at DESC);
CREATE INDEX idx_external_orders_tenant_status
    ON external_orders(tenant_id, status, placed_at DESC);

CREATE TABLE order_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    order_id UUID NOT NULL REFERENCES external_orders(id) ON DELETE CASCADE,
    event_type VARCHAR(80) NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_order_events_order
    ON order_events(order_id, created_at DESC);
