-- Use VARCHAR instead of ENUM for simplicity
CREATE TABLE transactions (
    id BIGSERIAL PRIMARY KEY,
    tx_type VARCHAR(20) NOT NULL CHECK (tx_type IN ('invoice', 'payment')),
    payment_hash VARCHAR(64) UNIQUE NOT NULL,
    payment_request TEXT NOT NULL,
    amount_sats BIGINT NOT NULL,
    description TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'failed', 'expired')),
    preimage VARCHAR(64),
    fee_sats BIGINT,
    failure_reason TEXT,
    expires_at TIMESTAMPTZ,
    node_id VARCHAR(66) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tx_type ON transactions(tx_type);
CREATE INDEX idx_tx_status ON transactions(status);
CREATE INDEX idx_tx_created ON transactions(created_at DESC);
