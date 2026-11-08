-- 0003_wallet_credits.sql
-- Extends the wallet with payment-gateway provenance and adds the AI credit
-- ledger. The `wallet_transactions` table from 0001 is kept as-is: amounts stay
-- positive and direction stays in `txn_type`. Do NOT introduce a second wallet
-- table — see docs/BILLING_WALLET_SPEC.md section 2.

-- ---------------------------------------------------------------------------
-- AI credits live on the user row as a cached projection, backed by a ledger.
-- ---------------------------------------------------------------------------

ALTER TABLE users
    ADD COLUMN ai_credits INTEGER NOT NULL DEFAULT 0;

ALTER TABLE users
    ADD CONSTRAINT users_ai_credits_non_negative CHECK (ai_credits >= 0);

CREATE TYPE ai_credit_txn_type AS ENUM ('grant', 'topup', 'spend', 'refund', 'adjustment');

CREATE TABLE ai_credit_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    txn_type ai_credit_txn_type NOT NULL,
    -- Always positive; direction comes from txn_type.
    credits INTEGER NOT NULL CHECK (credits > 0),
    balance_after INTEGER NOT NULL CHECK (balance_after >= 0),
    reference_type VARCHAR(50),          -- 'generation_job' | 'purchase' | 'manual'
    reference_id UUID,
    note TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_ai_credit_txn_user ON ai_credit_transactions(user_id, created_at DESC);
CREATE INDEX idx_ai_credit_txn_ref
    ON ai_credit_transactions(reference_type, reference_id)
    WHERE reference_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Payment gateway provenance
-- ---------------------------------------------------------------------------

-- `payments.provider` was created in 0001 with ('wallet','card','manual').
-- Extend the same enum instead of adding a parallel column: the gateway that
-- took the money belongs on the payment row.
ALTER TYPE payment_provider ADD VALUE IF NOT EXISTS 'bkash';
ALTER TYPE payment_provider ADD VALUE IF NOT EXISTS 'sslcommerz';
ALTER TYPE payment_provider ADD VALUE IF NOT EXISTS 'stripe';

ALTER TABLE wallet_transactions
    ADD COLUMN gateway VARCHAR(50),          -- 'bkash' | 'sslcommerz' | 'stripe' | 'internal'
    ADD COLUMN gateway_ref VARCHAR(255),     -- gateway transaction id
    ADD COLUMN currency CHAR(3) NOT NULL DEFAULT 'USD';

-- Idempotency: one ledger credit per gateway transaction, enforced by the DB.
CREATE UNIQUE INDEX idx_wallet_txn_gateway_ref
    ON wallet_transactions(gateway, gateway_ref)
    WHERE gateway_ref IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Inbound webhook log (idempotency + replay + debugging)
-- ---------------------------------------------------------------------------

CREATE TYPE webhook_status AS ENUM ('received', 'verified', 'processed', 'rejected', 'failed');

CREATE TABLE payment_webhooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider VARCHAR(50) NOT NULL,
    -- Gateway-side unique event id. The pair is the idempotency key.
    event_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(100),
    payload JSONB NOT NULL,
    signature_valid BOOLEAN NOT NULL DEFAULT false,
    status webhook_status NOT NULL DEFAULT 'received',
    processed_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT payment_webhooks_unique_event UNIQUE (provider, event_id)
);

CREATE INDEX idx_payment_webhooks_status
    ON payment_webhooks(status, created_at);
