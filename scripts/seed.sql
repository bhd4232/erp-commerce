-- scripts/seed.sql
--
-- Seed-only catalogue for v1 (docs/ROADMAP.md, P1). There is no admin product
-- CRUD: adding a product means editing this file and re-running it.
--
-- Safe to re-run: products are keyed by slug, files by (product_id, version).
--
-- The storage keys below are placeholders. A download only succeeds once the
-- matching object exists in the bucket.

BEGIN;

-- The ONLY sanctioned way to add wallet balance before P3 exists.
-- It updates the cached balance and writes the ledger row together, so the
-- invariant in docs/BILLING_WALLET_SPEC.md section 1 holds. Tests call this
-- directly instead of going through an API.
CREATE OR REPLACE FUNCTION seed_wallet(p_user_id UUID, p_amount NUMERIC)
RETURNS NUMERIC
LANGUAGE plpgsql AS $$
DECLARE
    v_balance NUMERIC(12, 2);
BEGIN
    IF p_amount IS NULL OR p_amount <= 0 THEN
        RAISE EXCEPTION 'seed amount must be positive';
    END IF;

    UPDATE users
       SET wallet_balance = wallet_balance + p_amount,
           updated_at = NOW()
     WHERE id = p_user_id
    RETURNING wallet_balance INTO v_balance;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'no such user: %', p_user_id;
    END IF;

    INSERT INTO wallet_transactions
        (user_id, txn_type, amount, balance_after, reference_type, note)
    VALUES
        (p_user_id, 'credit', p_amount, v_balance, 'manual', 'seed funding');

    RETURN v_balance;
END;
$$;

INSERT INTO digital_products (title, slug, product_type, price, description, is_active)
VALUES
    ('Notion Template Pack',
     'notion-template-pack',
     'premade_file',
     19.00,
     'A ready-to-use Notion workspace for running a small e-commerce store.',
     true),
    ('Store Launch Checklist',
     'store-launch-checklist',
     'premade_file',
     9.00,
     'A 40-point pre-launch checklist covering payments, delivery and support.',
     true),
    ('Banner Generator',
     'banner-generator',
     'dynamic_tool',
     29.00,
     'AI banner generation for product promos. Uses AI credits per render.',
     true)
ON CONFLICT (slug) DO NOTHING;

INSERT INTO product_files
    (product_id, version, storage_path, file_size_bytes, mime_type, is_current)
SELECT p.id,
       '1.0.0',
       'protected-products/' || p.id || '/1.0.0/' || p.slug || '.zip',
       CASE p.slug
           WHEN 'notion-template-pack' THEN 1048576
           ELSE 262144
       END,
       'application/zip',
       true
  FROM digital_products p
 WHERE p.slug IN ('notion-template-pack', 'store-launch-checklist')
ON CONFLICT (product_id, version) DO NOTHING;

COMMIT;
