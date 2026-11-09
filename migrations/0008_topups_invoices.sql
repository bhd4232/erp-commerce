-- 0008_topups_invoices.sql
-- Top-up intents (a gateway checkout in flight) and the invoice-number series.

CREATE TYPE topup_status AS ENUM ('pending', 'paid', 'expired', 'cancelled');

CREATE TABLE topup_intents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    provider VARCHAR(50) NOT NULL,
    status topup_status NOT NULL DEFAULT 'pending',
    provider_ref VARCHAR(255),
    expires_at TIMESTAMPTZ NOT NULL,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_topup_user ON topup_intents(user_id, created_at DESC);
CREATE INDEX idx_topup_pending
    ON topup_intents(status, expires_at)
    WHERE status = 'pending';

CREATE SEQUENCE invoice_number_seq START 1;

CREATE OR REPLACE FUNCTION next_invoice_number() RETURNS TEXT
LANGUAGE sql VOLATILE AS $$
    SELECT 'INV-'
        || to_char(NOW(), 'YYYYMM')
        || '-'
        || to_char(nextval('invoice_number_seq'), 'FM000000');
$$;
