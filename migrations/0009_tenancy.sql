-- 0009_tenancy.sql
-- Tenancy for the COD platform. Append-only: do not edit after apply.
--
-- A tenant is one client business. Users stay global and join a tenant through
-- `memberships`. Existing digital/service tables are intentionally untouched in
-- this phase; only the new COD modules use tenant_id.

CREATE TYPE tenant_status AS ENUM ('active', 'suspended');

CREATE TYPE tenant_role AS ENUM ('owner', 'admin', 'staff');

CREATE TYPE membership_status AS ENUM ('invited', 'active', 'disabled');

CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(120) NOT NULL,
    slug VARCHAR(80) UNIQUE NOT NULL,
    status tenant_status NOT NULL DEFAULT 'active',
    timezone VARCHAR(64) NOT NULL DEFAULT 'Asia/Dhaka',
    currency CHAR(3) NOT NULL DEFAULT 'BDT',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE memberships (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role tenant_role NOT NULL DEFAULT 'staff',
    status membership_status NOT NULL DEFAULT 'invited',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT memberships_unique_user UNIQUE (tenant_id, user_id)
);

CREATE INDEX idx_memberships_user
    ON memberships(user_id, status, created_at DESC);
CREATE INDEX idx_memberships_tenant
    ON memberships(tenant_id, status, created_at);

CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID,
    action VARCHAR(80) NOT NULL,
    before JSONB,
    after JSONB,
    ip_address VARCHAR(45),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_tenant_created
    ON audit_events(tenant_id, created_at DESC);
CREATE INDEX idx_audit_entity
    ON audit_events(entity_type, entity_id, created_at DESC)
    WHERE entity_id IS NOT NULL;
