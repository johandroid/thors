ALTER TABLE transactions
    DROP CONSTRAINT IF EXISTS transactions_payment_hash_key;

ALTER TABLE transactions
    ADD CONSTRAINT transactions_payment_hash_type_key UNIQUE (tx_type, payment_hash);
