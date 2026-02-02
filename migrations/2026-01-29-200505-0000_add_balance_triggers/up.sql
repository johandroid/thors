-- PostgreSQL trigger to auto-update balance when transactions succeed
CREATE OR REPLACE FUNCTION update_balance()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'succeeded' THEN
        IF NEW.tx_type = 'invoice' THEN
            UPDATE balance SET
                received_sats = received_sats + NEW.amount_sats,
                last_updated = NOW()
            WHERE id = 1;
        ELSIF NEW.tx_type = 'payment' THEN
            UPDATE balance SET
                paid_sats = paid_sats + NEW.amount_sats + COALESCE(NEW.fee_sats, 0),
                last_updated = NOW()
            WHERE id = 1;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER transaction_balance_update
AFTER INSERT OR UPDATE OF status ON transactions
FOR EACH ROW
WHEN (NEW.status = 'succeeded')
EXECUTE FUNCTION update_balance();
