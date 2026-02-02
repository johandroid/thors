use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tonic_lnd::lnrpc::invoice::InvoiceState;
use tonic_lnd::{lnrpc, Client as LndClient};

use crate::dto::InvoiceEvent;
use crate::models::{NewTransaction, TxStatus, TxType};
use crate::server::db::{self, DbPool};

/// Sync all existing invoices from LND into the database at startup.
/// For each invoice: insert if not in DB, update status if changed, skip if identical.
pub async fn sync_invoices_from_lnd(lnd_client: &mut LndClient, db_pool: &DbPool, node_id: &str) {
    tracing::info!("Syncing existing invoices from LND...");

    let request = lnrpc::ListInvoiceRequest {
        pending_only: false,
        index_offset: 0,
        num_max_invoices: u64::MAX,
        reversed: false,
    };

    match lnd_client.lightning().list_invoices(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            let total = resp.invoices.len();
            let mut changed = 0u32;
            let mut unchanged = 0u32;

            for inv in &resp.invoices {
                let status = lnd_state_to_tx_status(inv.state);
                let payment_hash = hex::encode(&inv.r_hash);

                let expires_at = if inv.expiry > 0 && inv.creation_date > 0 {
                    DateTime::from_timestamp(inv.creation_date + inv.expiry, 0)
                        .map(|dt| dt.with_timezone(&Utc))
                } else {
                    None
                };

                let new_tx = NewTransaction::new(
                    TxType::Invoice,
                    payment_hash,
                    inv.payment_request.clone(),
                    inv.value,
                    if inv.memo.is_empty() {
                        None
                    } else {
                        Some(inv.memo.clone())
                    },
                    status,
                    expires_at,
                    node_id.to_string(),
                );

                match db::upsert_transaction(db_pool, new_tx).await {
                    Ok(Some(_)) => changed += 1,
                    Ok(None) => unchanged += 1,
                    Err(e) => tracing::error!("Failed to upsert invoice: {}", e),
                }
            }

            tracing::info!(
                "LND sync complete: {} total invoices, {} changed/added, {} unchanged",
                total,
                changed,
                unchanged
            );
        }
        Err(e) => {
            tracing::error!("Failed to list invoices from LND: {}", e);
        }
    }
}

/// Subscribe to LND invoice events using a dedicated LND connection.
/// When a new invoice is created or its state changes, it is upserted into the DB
/// and broadcast via WebSocket to all connected clients.
pub async fn subscribe_to_invoices(
    mut lnd_client: LndClient,
    db_pool: DbPool,
    broadcast_tx: broadcast::Sender<InvoiceEvent>,
    node_id: String,
) {
    tracing::info!("Starting invoice subscription task");

    loop {
        let subscription = lnrpc::InvoiceSubscription {
            add_index: 0,
            settle_index: 0,
        };

        match lnd_client
            .lightning()
            .subscribe_invoices(subscription)
            .await
        {
            Ok(response) => {
                let mut stream = response.into_inner();

                while let Some(invoice_result) = stream.next().await {
                    match invoice_result {
                        Ok(invoice) => {
                            if let Err(e) =
                                handle_invoice_event(&invoice, &db_pool, &broadcast_tx, &node_id)
                                    .await
                            {
                                tracing::error!("Error handling invoice event: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Stream error: {}", e);
                            break;
                        }
                    }
                }

                tracing::warn!("Invoice subscription stream ended, reconnecting in 5s...");
            }
            Err(e) => {
                tracing::error!("Failed to subscribe to invoices: {}", e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn handle_invoice_event(
    invoice: &lnrpc::Invoice,
    db_pool: &DbPool,
    broadcast_tx: &broadcast::Sender<InvoiceEvent>,
    node_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let payment_hash = hex::encode(&invoice.r_hash);
    let status = lnd_state_to_tx_status(invoice.state);

    let expires_at = if invoice.expiry > 0 && invoice.creation_date > 0 {
        DateTime::from_timestamp(invoice.creation_date + invoice.expiry, 0)
            .map(|dt| dt.with_timezone(&Utc))
    } else {
        None
    };

    // Upsert: creates if new, updates if status changed, skips if same
    let new_tx = NewTransaction::new(
        TxType::Invoice,
        payment_hash,
        invoice.payment_request.clone(),
        invoice.value,
        if invoice.memo.is_empty() {
            None
        } else {
            Some(invoice.memo.clone())
        },
        status,
        expires_at,
        node_id.to_string(),
    );

    let result = db::upsert_transaction(db_pool, new_tx).await?;

    // Only broadcast if something actually changed
    if let Some(tx) = result {
        let event = match status {
            TxStatus::Pending => InvoiceEvent::InvoiceCreated { tx },
            TxStatus::Succeeded => InvoiceEvent::InvoiceSettled { tx },
            TxStatus::Expired => InvoiceEvent::InvoiceExpired { tx },
            _ => return Ok(()),
        };

        let _ = broadcast_tx.send(event);
    }

    Ok(())
}

fn lnd_state_to_tx_status(state: i32) -> TxStatus {
    match state {
        s if s == InvoiceState::Open as i32 => TxStatus::Pending,
        s if s == InvoiceState::Settled as i32 => TxStatus::Succeeded,
        s if s == InvoiceState::Canceled as i32 => TxStatus::Expired,
        s if s == InvoiceState::Accepted as i32 => TxStatus::Pending,
        _ => TxStatus::Pending,
    }
}
