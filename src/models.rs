#[cfg(feature = "ssr")]
use crate::schema::{balance, transactions};
use chrono::{DateTime, Utc};
#[cfg(feature = "ssr")]
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

// Simple enums (not mapped to PostgreSQL ENUM types)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Invoice,
    Payment,
}

impl TxType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TxType::Invoice => "invoice",
            TxType::Payment => "payment",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "invoice" => Some(TxType::Invoice),
            "payment" => Some(TxType::Payment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TxStatus {
    Pending,
    Succeeded,
    Failed,
    Expired,
}

impl TxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TxStatus::Pending => "pending",
            TxStatus::Succeeded => "succeeded",
            TxStatus::Failed => "failed",
            TxStatus::Expired => "expired",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TxStatus::Pending),
            "succeeded" => Some(TxStatus::Succeeded),
            "failed" => Some(TxStatus::Failed),
            "expired" => Some(TxStatus::Expired),
            _ => None,
        }
    }
}

// Transaction model (String fields instead of enums)
#[cfg_attr(feature = "ssr", derive(Queryable, Selectable))]
#[cfg_attr(feature = "ssr", diesel(table_name = transactions))]
#[cfg_attr(feature = "ssr", diesel(check_for_backend(diesel::pg::Pg)))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    tx_type: String, // Private, use getter
    pub payment_hash: String,
    pub payment_request: String,
    pub amount_sats: i64,
    pub description: Option<String>,
    status: String, // Private, use getter
    pub preimage: Option<String>,
    pub fee_sats: Option<i64>,
    pub failure_reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub node_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Transaction {
    pub fn tx_type(&self) -> TxType {
        TxType::from_str(&self.tx_type).unwrap()
    }

    pub fn status(&self) -> TxStatus {
        TxStatus::from_str(&self.status).unwrap()
    }
}

// Insert struct
#[cfg(feature = "ssr")]
#[derive(Debug, Insertable)]
#[diesel(table_name = transactions)]
pub struct NewTransaction {
    pub tx_type: String,
    pub payment_hash: String,
    pub payment_request: String,
    pub amount_sats: i64,
    pub description: Option<String>,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub node_id: String,
}

#[cfg(feature = "ssr")]
impl NewTransaction {
    pub fn new(
        tx_type: TxType,
        payment_hash: String,
        payment_request: String,
        amount_sats: i64,
        description: Option<String>,
        status: TxStatus,
        expires_at: Option<DateTime<Utc>>,
        node_id: String,
    ) -> Self {
        Self {
            tx_type: tx_type.as_str().to_string(),
            payment_hash,
            payment_request,
            amount_sats,
            description,
            status: status.as_str().to_string(),
            expires_at,
            node_id,
        }
    }
}

// Update struct
#[cfg(feature = "ssr")]
#[derive(Debug, AsChangeset)]
#[diesel(table_name = transactions)]
pub struct UpdateTransaction {
    pub status: Option<String>,
    pub preimage: Option<String>,
    pub fee_sats: Option<i64>,
    pub failure_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(feature = "ssr")]
impl UpdateTransaction {
    pub fn new(
        status: Option<TxStatus>,
        preimage: Option<String>,
        fee_sats: Option<i64>,
        failure_reason: Option<String>,
    ) -> Self {
        Self {
            status: status.map(|s| s.as_str().to_string()),
            preimage,
            fee_sats,
            failure_reason,
            updated_at: Utc::now(),
        }
    }
}

// Balance model
#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = balance)]
pub struct Balance {
    pub id: i32,
    pub received_sats: i64,
    pub paid_sats: i64,
    pub last_updated: DateTime<Utc>,
}

#[cfg(feature = "ssr")]
impl Balance {
    pub fn total_balance(&self) -> i64 {
        self.received_sats - self.paid_sats
    }
}
