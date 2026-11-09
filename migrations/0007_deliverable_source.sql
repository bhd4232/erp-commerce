-- 0007_deliverable_source.sql
-- The raw preview a staff member uploads is the watermark worker's input. It
-- must be stored separately from the watermarked output: the client-facing
-- `watermarked_preview_path` is NULL until the worker has produced a safe copy.

ALTER TABLE order_deliverables
    ADD COLUMN source_preview_path TEXT;
