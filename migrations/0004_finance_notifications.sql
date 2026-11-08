-- 0004_finance_notifications.sql
-- Invoice ledger and in-app notifications. Multi-tenancy (workspaces) is out of
-- scope for v1: all users belong to a single workspace/agency.

CREATE TYPE invoice_status AS ENUM ('draft', 'open', 'paid', 'void', 'refunded');

CREATE TABLE invoices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_number VARCHAR(50) UNIQUE NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id),
    order_id UUID REFERENCES orders(id),
    currency CHAR(3) DEFAULT 'USD' NOT NULL,
    subtotal NUMERIC(12, 2) NOT NULL CHECK (subtotal >= 0),
    tax_amount NUMERIC(12, 2) DEFAULT 0 NOT NULL CHECK (tax_amount >= 0),
    total NUMERIC(12, 2) NOT NULL CHECK (total >= 0),
    status invoice_status DEFAULT 'open' NOT NULL,
    issued_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    paid_at TIMESTAMPTZ,
    -- S3 key of the rendered PDF. Never exposed directly.
    pdf_path TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_invoices_user_issued ON invoices(user_id, issued_at DESC);
CREATE INDEX idx_invoices_order ON invoices(order_id);
CREATE INDEX idx_invoices_open ON invoices(status) WHERE status = 'open';

CREATE TYPE notification_kind AS ENUM (
    'order_status',
    'deliverable_ready',
    'revision_requested',
    'download',
    'payment',
    'system'
);

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind notification_kind NOT NULL,
    title VARCHAR(200) NOT NULL,
    body TEXT,
    reference_type VARCHAR(50),
    reference_id UUID,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Fast unread badge query.
CREATE INDEX idx_notifications_unread
    ON notifications(user_id, created_at DESC)
    WHERE read_at IS NULL;

CREATE INDEX idx_notifications_user
    ON notifications(user_id, created_at DESC);
