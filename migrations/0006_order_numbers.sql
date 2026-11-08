-- 0006_order_numbers.sql
-- Human-readable, collision-free order numbers.
--
-- A sequence is used instead of counting existing rows: two concurrent
-- checkouts must never compute the same next number. `to_char` is used rather
-- than `lpad` because `lpad` truncates once the value exceeds the width.

CREATE SEQUENCE order_number_seq START 1;

CREATE OR REPLACE FUNCTION next_order_number(prefix TEXT) RETURNS TEXT
LANGUAGE sql VOLATILE AS $$
    SELECT prefix
        || '-'
        || to_char(NOW(), 'YYYYMMDD')
        || '-'
        || to_char(nextval('order_number_seq'), 'FM0000');
$$;
