CREATE TABLE balance (
    id INTEGER PRIMARY KEY DEFAULT 1,
    received_sats BIGINT NOT NULL DEFAULT 0,
    paid_sats BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert initial row
INSERT INTO balance (id, received_sats, paid_sats) VALUES (1, 0, 0);
