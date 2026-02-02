use crate::models::Transaction;
use serde::{Deserialize, Serialize};

// ===== Invoice DTOs =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceRequest {
    pub amount_sats: i64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceResponse {
    pub payment_request: String,
    pub payment_hash: String,
    pub amount_sats: i64,
}

// ===== Payment DTOs =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayInvoiceRequest {
    pub payment_request: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub payment_hash: String,
    pub preimage: String,
    pub amount_sats: i64,
}

// ===== Balance DTOs =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceDto {
    pub received_sats: i64,
    pub paid_sats: i64,
    pub total_balance: i64,
    pub last_updated: String,
}

// ===== Real-time Event DTOs =====

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InvoiceEvent {
    InvoiceCreated { tx: Transaction },
    InvoiceSettled { tx: Transaction },
    InvoiceExpired { tx: Transaction },
    PaymentSucceeded { tx: Transaction },
}
