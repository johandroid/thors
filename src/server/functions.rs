use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::models::{NewTransaction, TxStatus, TxType};
#[cfg(feature = "ssr")]
use crate::server::db::{create_transaction, get_balance_summary, list_transactions, DbPool};
#[cfg(feature = "ssr")]
use crate::server::lnd::LightningClients;
#[cfg(feature = "ssr")]
use tokio::sync::broadcast;

use crate::dto::*;

// Simple error type for server functions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppError(pub String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {}

impl std::str::FromStr for AppError {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(AppError(s.to_string()))
    }
}

// AppState structure that will be provided as context (SSR only)
#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct AppState {
    pub db_pool: DbPool,
    pub lnd_receive: LightningClients,
    pub lnd_send: LightningClients,
    pub broadcast_tx: broadcast::Sender<InvoiceEvent>,
    pub receive_node_id: String,
    pub send_node_id: String,
}

#[server]
pub async fn create_invoice_fn(
    amount_sats: i64,
    description: Option<String>,
) -> Result<InvoiceResponse, ServerFnError> {
    let app_state = expect_context::<AppState>();
    let lnd = app_state.lnd_receive.clone();

    // Validate amount
    if amount_sats <= 0 {
        return Err(AppError("Invalid amount".to_string()).into());
    }

    // Create invoice in LND
    let lnd_invoice = lnd
        .create_invoice(amount_sats, description.clone())
        .await
        .map_err(|e| AppError(e.to_string()))?;

    // No DB insert here; background invoice subscription handles persistence.

    Ok(InvoiceResponse {
        payment_request: lnd_invoice.payment_request,
        payment_hash: hex::encode(&lnd_invoice.r_hash),
        amount_sats,
    })
}

#[server]
pub async fn pay_invoice_fn(payment_request: String) -> Result<PaymentResponse, ServerFnError> {
    let app_state = expect_context::<AppState>();
    let lnd = app_state.lnd_send.clone();

    // Decode invoice
    let decoded = lnd
        .decode_payment_request(payment_request.clone())
        .await
        .map_err(|e| AppError(e.to_string()))?;

    // Save as pending
    let new_tx = NewTransaction::new(
        TxType::Payment,
        decoded.payment_hash.clone(),
        payment_request.clone(),
        decoded.num_satoshis,
        Some(decoded.description.clone()),
        TxStatus::Pending,
        None,
        app_state.send_node_id.clone(),
    );

    create_transaction(&app_state.db_pool, new_tx)
        .await
        .map_err(|e| AppError(e.to_string()))?;

    // Send payment
    let payment = lnd
        .send_payment(payment_request)
        .await
        .map_err(|e| AppError(e.to_string()))?;

    if !payment.payment_error.is_empty() {
        return Err(AppError(payment.payment_error).into());
    }

    // Update status to succeeded (trigger will update balance)
    let update = crate::models::UpdateTransaction::new(
        Some(TxStatus::Succeeded),
        Some(hex::encode(&payment.payment_preimage)),
        payment
            .payment_route
            .as_ref()
            .map(|r| r.total_fees_msat / 1000),
        None,
    );

    let tx = crate::server::db::update_transaction_status(
        &app_state.db_pool,
        TxType::Payment,
        &decoded.payment_hash,
        update,
    )
    .await
    .map_err(|e| AppError(e.to_string()))?;

    let _ = app_state
        .broadcast_tx
        .send(InvoiceEvent::PaymentSucceeded { tx });

    Ok(PaymentResponse {
        payment_hash: decoded.payment_hash,
        preimage: hex::encode(&payment.payment_preimage),
        amount_sats: decoded.num_satoshis,
    })
}

#[server]
pub async fn get_transactions_fn(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<crate::models::Transaction>, ServerFnError> {
    let app_state = expect_context::<AppState>();

    let txs = list_transactions(&app_state.db_pool, limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| AppError(e.to_string()))?;

    Ok(txs)
}

#[server]
pub async fn get_balance_fn() -> Result<crate::dto::BalanceDto, ServerFnError> {
    let app_state = expect_context::<AppState>();

    let balance = get_balance_summary(
        &app_state.db_pool,
        &app_state.receive_node_id,
        &app_state.send_node_id,
    )
    .await
    .map_err(|e| AppError(e.to_string()))?;

    Ok(BalanceDto {
        received_sats: balance.received_sats,
        paid_sats: balance.paid_sats,
        total_balance: balance.pending_received_sats - balance.pending_paid_sats,
        last_updated: balance.last_updated.to_rfc3339(),
    })
}
