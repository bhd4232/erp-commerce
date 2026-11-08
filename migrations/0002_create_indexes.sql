-- 0002_create_indexes.sql
-- Query-path indexes. Mirrors docs/DATABASE.md section 5.

-- ---------------------------------------------------------------------------
-- Digital domain
-- ---------------------------------------------------------------------------

CREATE INDEX idx_entitlements_user            ON user_entitlements(user_id);
CREATE INDEX idx_entitlements_user_product    ON user_entitlements(user_id, product_id);
CREATE INDEX idx_product_files_product        ON product_files(product_id);
CREATE INDEX idx_audit_download_entitlement   ON audit_download_logs(entitlement_id, downloaded_at DESC);
CREATE INDEX idx_download_logs_created        ON audit_download_logs(downloaded_at DESC);

CREATE INDEX idx_orders_user_created          ON orders(user_id, created_at DESC);
CREATE INDEX idx_order_items_order            ON order_items(order_id);
CREATE INDEX idx_payments_order               ON payments(order_id);

CREATE UNIQUE INDEX idx_payments_provider_ref
    ON payments(provider, provider_ref) WHERE provider_ref IS NOT NULL;

-- A product has at most one current file version.
CREATE UNIQUE INDEX idx_product_files_current
    ON product_files(product_id) WHERE is_current;

-- ---------------------------------------------------------------------------
-- Agency domain
-- ---------------------------------------------------------------------------

CREATE INDEX idx_service_orders_status        ON service_orders(status);
CREATE INDEX idx_service_orders_client        ON service_orders(client_id);
CREATE INDEX idx_service_orders_staff         ON service_orders(assigned_staff_id);

-- Kanban board query: "everything in this column, oldest first".
CREATE INDEX idx_service_orders_board
    ON service_orders(status, updated_at) WHERE status <> 'completed';

CREATE INDEX idx_deliverables_order           ON order_deliverables(order_id, version DESC);
CREATE INDEX idx_revisions_order              ON order_revisions(order_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Wallet ledger
-- ---------------------------------------------------------------------------

CREATE INDEX idx_wallet_txn_user_created      ON wallet_transactions(user_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Catalogue
-- ---------------------------------------------------------------------------

CREATE INDEX idx_digital_products_active
    ON digital_products(created_at DESC) WHERE is_active;
CREATE INDEX idx_agency_services_active
    ON agency_services(created_at DESC) WHERE is_active;
