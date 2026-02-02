use crate::app::{shell, App};
use crate::errors::{AppError, Result as AppResult};
use crate::server::{api, background, db, lnd, sse, AppState, InvoiceEvent};

use axum::routing::{get, post};
use axum::Router;
use diesel::Connection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use tokio::sync::broadcast;
use tonic_lnd::Client as LndClient;
use tower_http::cors::{Any, CorsLayer};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Application configuration loaded from environment variables
pub struct Config {
    pub database_url: String,
    pub run_migrations: bool,
    pub lnd_endpoint: String,
    pub lnd_cert_path: String,
    pub lnd_macaroon_path: String,
    pub lnd_send_endpoint: String,
    pub lnd_send_cert_path: String,
    pub lnd_send_macaroon_path: String,
}

impl Config {
    pub fn from_env() -> AppResult<Self> {
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: read_env("DATABASE_URL")?,
            run_migrations: std::env::var("RUN_MIGRATIONS")
                .map(|v| v == "true")
                .unwrap_or(false),
            lnd_endpoint: read_env("LND_ENDPOINT")?,
            lnd_cert_path: read_env("LND_CERT_PATH")?,
            lnd_macaroon_path: read_env("LND_MACAROON_PATH")?,
            lnd_send_endpoint: read_env("LND_SEND_ENDPOINT")?,
            lnd_send_cert_path: read_env("LND_SEND_CERT_PATH")?,
            lnd_send_macaroon_path: read_env("LND_SEND_MACAROON_PATH")?,
        })
    }
}

fn read_env(name: &str) -> AppResult<String> {
    match std::env::var(name) {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Err(AppError::EmptyEnv(name.to_string()))
            } else {
                Ok(trimmed)
            }
        }
        Err(std::env::VarError::NotPresent) => Err(AppError::MissingConfig(name.to_string())),
        Err(std::env::VarError::NotUnicode(_)) => Err(AppError::InvalidEnv(name.to_string())),
    }
}

/// Run embedded Diesel migrations
pub fn run_migrations(database_url: &str) -> AppResult<()> {
    tracing::info!("Running database migrations...");
    let mut conn = diesel::pg::PgConnection::establish(database_url)
        .map_err(|e| AppError::Server(format!("Cannot connect to database: {e}")))?;

    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| AppError::Server(format!("Failed to run migrations: {e}")))?;

    tracing::info!("Migrations completed successfully");
    Ok(())
}

/// Connect to a single LND node
async fn connect_lnd_client(
    endpoint: &str,
    cert_path: &str,
    macaroon_path: &str,
    label: &str,
) -> AppResult<LndClient> {
    tracing::info!(
        endpoint,
        cert_path,
        macaroon_path,
        "Connecting to LND ({label})"
    );

    lnd::connect(
        endpoint.to_string(),
        cert_path.to_string(),
        macaroon_path.to_string(),
    )
    .await
    .map_err(|e| AppError::Server(format!("Failed to connect to LND ({label}): {e:?}")))
}

/// Create the three LND connections: receive API, subscription, send API
pub async fn setup_lnd_clients(config: &Config) -> AppResult<(LndClient, LndClient, LndClient)> {
    let api_receive = connect_lnd_client(
        &config.lnd_endpoint,
        &config.lnd_cert_path,
        &config.lnd_macaroon_path,
        "receiver",
    )
    .await?;

    let subscription = connect_lnd_client(
        &config.lnd_endpoint,
        &config.lnd_cert_path,
        &config.lnd_macaroon_path,
        "subscription",
    )
    .await?;

    let api_send = connect_lnd_client(
        &config.lnd_send_endpoint,
        &config.lnd_send_cert_path,
        &config.lnd_send_macaroon_path,
        "sender",
    )
    .await?;

    tracing::info!("Connected to all LND nodes successfully");
    Ok((api_receive, subscription, api_send))
}

/// Fetch a node's public key
pub async fn fetch_node_pubkey(client: &mut LndClient, label: &str) -> AppResult<String> {
    let pubkey = lnd::get_node_pubkey(client)
        .await
        .map_err(|e| AppError::Server(format!("Failed to fetch {label} node ID: {e:?}")))?;

    tracing::info!(node_id = %pubkey, "Fetched {label} node ID");
    Ok(pubkey)
}

/// Build the full Axum router (API + Leptos SSR + SSE)
pub fn build_router(app_state: AppState, leptos_options: LeptosOptions) -> Router {
    let routes = generate_route_list(App);
    let sse_broadcast = app_state.broadcast_tx.clone();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/invoice", post(api::create_invoice))
        .route("/invoice/{payment_hash}", get(api::get_invoice))
        .route("/payment", post(api::pay_invoice))
        .route("/payment/{payment_hash}", get(api::get_payment))
        .route("/transactions", get(api::list_transactions))
        .route("/balance", get(api::get_balance))
        .with_state(app_state.clone());

    Router::new()
        .route("/events", get(sse::sse_handler).with_state(sse_broadcast))
        .nest("/api", api_router)
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            move || provide_context(app_state.clone()),
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler::<LeptosOptions, _>(
            shell,
        ))
        .layer(cors)
        .with_state(leptos_options)
}

/// Spawn the background invoice subscription task
pub fn spawn_background_tasks(
    subscription_lnd: LndClient,
    db_pool: db::DbPool,
    broadcast_tx: broadcast::Sender<InvoiceEvent>,
    receive_node_id: String,
) {
    tokio::spawn(background::subscribe_to_invoices(
        subscription_lnd,
        db_pool,
        broadcast_tx,
        receive_node_id,
    ));
}
