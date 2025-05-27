
-- migrations/20240528_create_airdrop_log.sql

CREATE TABLE IF NOT EXISTS airdrop_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address TEXT NOT NULL,
    amount_sent BIGINT NOT NULL,
    tx_signature TEXT,
    sent_at TIMESTAMPTZ DEFAULT now()
);
