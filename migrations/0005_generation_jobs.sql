-- 0005_generation_jobs.sql
-- Durable record of every AI / render job. The job row is the idempotency unit
-- for credit deduction and the source of truth for the SSE stream.

CREATE TYPE generation_tool AS ENUM ('promo_banner', 'ad_copy', 'packing_slip');
CREATE TYPE generation_status AS ENUM ('queued', 'running', 'succeeded', 'failed', 'cancelled');

CREATE TABLE generation_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tool generation_tool NOT NULL,
    status generation_status DEFAULT 'queued' NOT NULL,
    -- Tool-specific inputs. Validated against a schema before insert.
    params JSONB NOT NULL,
    credits_charged INTEGER NOT NULL CHECK (credits_charged >= 0),
    -- S3 key of the produced asset. Never exposed directly.
    output_path TEXT,
    output_mime VARCHAR(100),
    error TEXT,
    -- Client-supplied; makes retries safe.
    idempotency_key VARCHAR(100),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT generation_jobs_idempotent UNIQUE (user_id, idempotency_key)
);

CREATE INDEX idx_generation_jobs_user ON generation_jobs(user_id, created_at DESC);
CREATE INDEX idx_generation_jobs_active
    ON generation_jobs(status, created_at)
    WHERE status IN ('queued', 'running');
