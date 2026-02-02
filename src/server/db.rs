use chrono::{DateTime, Utc};
use diesel::dsl::{max, sql};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::sql_types::{BigInt, Nullable};
use diesel_async::RunQueryDsl;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};

use crate::models::*;
use crate::schema::{balance, transactions};

#[derive(Debug, Clone)]
pub struct BalanceSummary {
    pub received_sats: i64,
    pub paid_sats: i64,
    pub pending_received_sats: i64,
    pub pending_paid_sats: i64,
    pub last_updated: DateTime<Utc>,
}

pub type DbPool = Pool<AsyncPgConnection>;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Diesel(#[from] diesel::result::Error),
    #[error("Pool error: {0}")]
    Pool(#[from] deadpool::managed::PoolError<diesel_async::pooled_connection::PoolError>),
}

pub fn create_pool(database_url: &str) -> DbPool {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    Pool::builder(config)
        .max_size(8)
        .build()
        .expect("Failed to create pool")
}

pub async fn create_transaction(
    pool: &DbPool,
    new_tx: NewTransaction,
) -> Result<Transaction, DbError> {
    let mut conn = pool.get().await?;

    let result = diesel::insert_into(transactions::table)
        .values(&new_tx)
        .returning(Transaction::as_returning())
        .get_result(&mut conn)
        .await?;

    Ok(result)
}

pub async fn update_transaction_status(
    pool: &DbPool,
    tx_type: TxType,
    payment_hash: &str,
    update: UpdateTransaction,
) -> Result<Transaction, DbError> {
    let mut conn = pool.get().await?;

    let result = diesel::update(transactions::table)
        .filter(transactions::payment_hash.eq(payment_hash))
        .filter(transactions::tx_type.eq(tx_type.as_str()))
        .set(&update)
        .returning(Transaction::as_returning())
        .get_result(&mut conn)
        .await?;

    Ok(result)
}

pub async fn get_transaction_by_hash(
    pool: &DbPool,
    tx_type: TxType,
    payment_hash: &str,
) -> Result<Option<Transaction>, DbError> {
    let mut conn = pool.get().await?;

    let result = transactions::table
        .filter(transactions::payment_hash.eq(payment_hash))
        .filter(transactions::tx_type.eq(tx_type.as_str()))
        .select(Transaction::as_select())
        .first(&mut conn)
        .await
        .optional()?;

    Ok(result)
}

pub async fn list_transactions(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transaction>, DbError> {
    let mut conn = pool.get().await?;

    let results = transactions::table
        .order(transactions::created_at.desc())
        .limit(limit)
        .offset(offset)
        .select(Transaction::as_select())
        .load(&mut conn)
        .await?;

    Ok(results)
}

/// Upsert a transaction: insert if it doesn't exist, update status if it changed.
/// Returns Some(transaction) if a change was made, None if already up-to-date.
pub async fn upsert_transaction(
    pool: &DbPool,
    new_tx: NewTransaction,
) -> Result<Option<Transaction>, DbError> {
    let mut conn = pool.get().await?;

    let updated = diesel::update(transactions::table)
        .filter(transactions::payment_hash.eq(&new_tx.payment_hash))
        .filter(transactions::tx_type.eq(&new_tx.tx_type))
        .filter(transactions::status.ne(&new_tx.status))
        .set((
            transactions::status.eq(&new_tx.status),
            transactions::updated_at.eq(Utc::now()),
        ))
        .returning(Transaction::as_returning())
        .get_result(&mut conn)
        .await
        .optional()?;

    if updated.is_some() {
        return Ok(updated);
    }

    let existing: Option<Transaction> = transactions::table
        .filter(transactions::payment_hash.eq(&new_tx.payment_hash))
        .filter(transactions::tx_type.eq(&new_tx.tx_type))
        .select(Transaction::as_select())
        .first(&mut conn)
        .await
        .optional()?;

    if existing.is_some() {
        return Ok(None);
    }

    let insert_result = diesel::insert_into(transactions::table)
        .values(&new_tx)
        .returning(Transaction::as_returning())
        .get_result(&mut conn)
        .await;

    match insert_result {
        Ok(tx) => Ok(Some(tx)),
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub async fn get_balance(pool: &DbPool) -> Result<Balance, DbError> {
    let mut conn = pool.get().await?;

    let result = balance::table
        .find(1)
        .select(Balance::as_select())
        .first(&mut conn)
        .await?;

    Ok(result)
}

pub async fn get_balance_summary(
    pool: &DbPool,
    receive_node_id: &str,
    send_node_id: &str,
) -> Result<BalanceSummary, DbError> {
    let mut conn = pool.get().await?;

    let received: Option<i64> = transactions::table
        .filter(transactions::node_id.eq(receive_node_id))
        .filter(transactions::tx_type.eq(TxType::Invoice.as_str()))
        .filter(
            transactions::status.eq_any([TxStatus::Pending.as_str(), TxStatus::Succeeded.as_str()]),
        )
        .select(sql::<Nullable<BigInt>>("SUM(amount_sats)::BIGINT"))
        .first(&mut conn)
        .await?;

    let paid_amount: Option<i64> = transactions::table
        .filter(transactions::node_id.eq(send_node_id))
        .filter(transactions::tx_type.eq(TxType::Payment.as_str()))
        .filter(transactions::status.eq(TxStatus::Succeeded.as_str()))
        .select(sql::<Nullable<BigInt>>("SUM(amount_sats)::BIGINT"))
        .first(&mut conn)
        .await?;

    let pending_received: Option<i64> = transactions::table
        .filter(transactions::node_id.eq(receive_node_id))
        .filter(transactions::tx_type.eq(TxType::Invoice.as_str()))
        .filter(transactions::status.eq(TxStatus::Pending.as_str()))
        .select(sql::<Nullable<BigInt>>("SUM(amount_sats)::BIGINT"))
        .first(&mut conn)
        .await?;

    let pending_paid: Option<i64> = transactions::table
        .filter(transactions::node_id.eq(send_node_id))
        .filter(transactions::tx_type.eq(TxType::Payment.as_str()))
        .filter(transactions::status.eq(TxStatus::Pending.as_str()))
        .select(sql::<Nullable<BigInt>>("SUM(amount_sats)::BIGINT"))
        .first(&mut conn)
        .await?;

    let last_updated_receive: Option<DateTime<Utc>> = transactions::table
        .filter(transactions::node_id.eq(receive_node_id))
        .select(max(transactions::updated_at))
        .first(&mut conn)
        .await?;

    let last_updated_send: Option<DateTime<Utc>> = transactions::table
        .filter(transactions::node_id.eq(send_node_id))
        .select(max(transactions::updated_at))
        .first(&mut conn)
        .await?;

    let last_updated = match (last_updated_receive, last_updated_send) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    Ok(BalanceSummary {
        received_sats: received.unwrap_or(0),
        paid_sats: paid_amount.unwrap_or(0),
        pending_received_sats: pending_received.unwrap_or(0),
        pending_paid_sats: pending_paid.unwrap_or(0),
        last_updated: last_updated.unwrap_or_else(Utc::now),
    })
}
