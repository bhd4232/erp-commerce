-- 0011_connector_rest_credentials.sql
-- WooCommerce REST credentials for historical import. The webhook path and
-- secret from 0010 remain valid; this only adds the optional outbound API
-- credentials. Secrets stay encrypted with pgcrypto.

ALTER TABLE connectors
    ADD COLUMN base_url TEXT,
    ADD COLUMN api_key_encrypted BYTEA,
    ADD COLUMN api_secret_encrypted BYTEA;

ALTER TABLE connectors
    ADD CONSTRAINT connectors_rest_credentials_together CHECK (
        (api_key_encrypted IS NULL) = (api_secret_encrypted IS NULL)
    );
