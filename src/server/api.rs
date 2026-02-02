use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use super::AppState;
use crate::dto::*;
use crate::models::{NewTransaction, TxStatus, TxType, UpdateTransaction};
use crate::server::{db, lnd};

// ===== Typed API errors =====

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("Payment already exists for this invoice")]
    DuplicatePayment,

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error(transparent)]
    Lnd(#[from] lnd::LndError),

    #[error(transparent)]
    Database(#[from] db::DbError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Self::BadRequest(_) | Self::DuplicatePayment | Self::PaymentFailed(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Lnd(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

// ===== POST /api/invoice =====

pub async fn create_invoice(
    State(state): State<AppState>,
    Json(body): Json<CreateInvoiceRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.amount_sats <= 0 {
        return Err(ApiError::BadRequest("amount_sats must be positive".into()));
    }

    let lnd_invoice = state
        .lnd_receive
        .create_invoice(body.amount_sats, body.description.clone())
        .await?;

    // Do not insert here: invoice events are persisted by the background LND
    // subscription to avoid duplicate inserts and sequence gaps.

    Ok((
        StatusCode::CREATED,
        Json(InvoiceResponse {
            payment_request: lnd_invoice.payment_request,
            payment_hash: hex::encode(&lnd_invoice.r_hash),
            amount_sats: body.amount_sats,
        }),
    ))
}

// ===== GET /api/invoice/{payment_hash} =====

pub async fn get_invoice(
    State(state): State<AppState>,
    Path(payment_hash): Path<String>,
) -> Result<Json<crate::models::Transaction>, ApiError> {
    let tx = db::get_transaction_by_hash(&state.db_pool, TxType::Invoice, &payment_hash).await?;

    match tx {
        Some(tx) if tx.tx_type() == TxType::Invoice => Ok(Json(tx)),
        Some(_) => Err(ApiError::NotFound(
            "Invoice not found (hash belongs to a payment)".into(),
        )),
        None => Err(ApiError::NotFound("Invoice not found".into())),
    }
}

// ===== POST /api/payment =====

pub async fn pay_invoice(
    State(state): State<AppState>,
    Json(body): Json<PayInvoiceRequest>,
) -> Result<Json<PaymentResponse>, ApiError> {
    if body.payment_request.is_empty() {
        return Err(ApiError::BadRequest("payment_request is required".into()));
    }

    // Decode invoice
    let decoded = state
        .lnd_send
        .decode_payment_request(body.payment_request.clone())
        .await?;

    // Avoid duplicate payment records for the same invoice
    let existing =
        db::get_transaction_by_hash(&state.db_pool, TxType::Payment, &decoded.payment_hash).await?;

    if existing.is_some() {
        return Err(ApiError::DuplicatePayment);
    }

    // Save as pending
    let new_tx = NewTransaction::new(
        TxType::Payment,
        decoded.payment_hash.clone(),
        body.payment_request.clone(),
        decoded.num_satoshis,
        Some(decoded.description.clone()),
        TxStatus::Pending,
        None,
        state.send_node_id.clone(),
    );

    db::create_transaction(&state.db_pool, new_tx).await?;

    // Send payment via LND
    let payment = state.lnd_send.send_payment(body.payment_request).await?;

    if !payment.payment_error.is_empty() {
        // Update status to failed
        let update = UpdateTransaction::new(
            Some(TxStatus::Failed),
            None,
            None,
            Some(payment.payment_error.clone()),
        );
        let _ = db::update_transaction_status(
            &state.db_pool,
            TxType::Payment,
            &decoded.payment_hash,
            update,
        )
        .await;

        return Err(ApiError::PaymentFailed(payment.payment_error));
    }

    // Update status to succeeded
    let update = UpdateTransaction::new(
        Some(TxStatus::Succeeded),
        Some(hex::encode(&payment.payment_preimage)),
        payment
            .payment_route
            .as_ref()
            .map(|r| r.total_fees_msat / 1000),
        None,
    );

    let tx = db::update_transaction_status(
        &state.db_pool,
        TxType::Payment,
        &decoded.payment_hash,
        update,
    )
    .await?;

    let _ = state
        .broadcast_tx
        .send(InvoiceEvent::PaymentSucceeded { tx });

    Ok(Json(PaymentResponse {
        payment_hash: decoded.payment_hash,
        preimage: hex::encode(&payment.payment_preimage),
        amount_sats: decoded.num_satoshis,
    }))
}

// ===== GET /api/payment/{payment_hash} =====

pub async fn get_payment(
    State(state): State<AppState>,
    Path(payment_hash): Path<String>,
) -> Result<Json<crate::models::Transaction>, ApiError> {
    let tx = db::get_transaction_by_hash(&state.db_pool, TxType::Payment, &payment_hash).await?;

    match tx {
        Some(tx) if tx.tx_type() == TxType::Payment => Ok(Json(tx)),
        Some(_) => Err(ApiError::NotFound(
            "Payment not found (hash belongs to an invoice)".into(),
        )),
        None => Err(ApiError::NotFound("Payment not found".into())),
    }
}

// ===== GET /api/transactions =====

#[derive(Debug, Deserialize)]
pub struct TransactionsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_transactions(
    State(state): State<AppState>,
    Query(params): Query<TransactionsQuery>,
) -> Result<Json<Vec<crate::models::Transaction>>, ApiError> {
    let txs = db::list_transactions(
        &state.db_pool,
        params.limit.unwrap_or(50),
        params.offset.unwrap_or(0),
    )
    .await?;

    Ok(Json(txs))
}

// ===== GET /api/balance =====

pub async fn get_balance(State(state): State<AppState>) -> Result<Json<BalanceDto>, ApiError> {
    let balance =
        db::get_balance_summary(&state.db_pool, &state.receive_node_id, &state.send_node_id)
            .await?;

    Ok(Json(BalanceDto {
        received_sats: balance.received_sats,
        paid_sats: balance.paid_sats,
        total_balance: balance.pending_received_sats - balance.pending_paid_sats,
        last_updated: balance.last_updated.to_rfc3339(),
    }))
}
