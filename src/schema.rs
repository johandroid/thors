// @generated automatically by Diesel CLI.

#[cfg(feature = "ssr")]
mod schema_inner {
    diesel::table! {
        balance (id) {
            id -> Int4,
            received_sats -> Int8,
            paid_sats -> Int8,
            last_updated -> Timestamptz,
        }
    }

    diesel::table! {
        transactions (id) {
            id -> Int8,
            #[max_length = 20]
            tx_type -> Varchar,
            #[max_length = 64]
            payment_hash -> Varchar,
            payment_request -> Text,
            amount_sats -> Int8,
            description -> Nullable<Text>,
            #[max_length = 20]
            status -> Varchar,
            #[max_length = 64]
            preimage -> Nullable<Varchar>,
            fee_sats -> Nullable<Int8>,
            failure_reason -> Nullable<Text>,
            expires_at -> Nullable<Timestamptz>,
            #[max_length = 66]
            node_id -> Varchar,
            created_at -> Timestamptz,
            updated_at -> Timestamptz,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(balance, transactions,);
}

#[cfg(feature = "ssr")]
pub use schema_inner::*;
