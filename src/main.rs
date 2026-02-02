#![recursion_limit = "512"]

use leptos::prelude::*;
use tokio::sync::broadcast;

use thors::errors::Result as AppResult;
use thors::initialize::{
    build_router, fetch_node_pubkey, run_migrations, setup_lnd_clients, spawn_background_tasks,
    Config,
};
use thors::server::{background, db, lnd, AppState, InvoiceEvent};

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::from_env()?;

    // Run database migrations if enabled
    if config.run_migrations {
        run_migrations(&config.database_url)?;
    }

    // Initialize database pool
    let db_pool = db::create_pool(&config.database_url);

    // Setup LND client connections (receive, subscription, send)
    let (mut api_lnd_receive, mut subscription_lnd, mut api_lnd_send) =
        setup_lnd_clients(&config).await?;

    // Fetch node public keys
    let receive_node_id = fetch_node_pubkey(&mut api_lnd_receive, "receiver").await?;
    let send_node_id = fetch_node_pubkey(&mut api_lnd_send, "sender").await?;

    // Wrap LND clients for shared access
    let lnd_receive = lnd::LightningClients::from_client(api_lnd_receive);
    let lnd_send = lnd::LightningClients::from_client(api_lnd_send);

    // Sync existing invoices from LND at startup
    background::sync_invoices_from_lnd(&mut subscription_lnd, &db_pool, &receive_node_id).await;

    // Setup broadcast channel for SSE events
    let (broadcast_tx, _) = broadcast::channel::<InvoiceEvent>(100);

    // Spawn background invoice subscription task
    spawn_background_tasks(
        subscription_lnd,
        db_pool.clone(),
        broadcast_tx.clone(),
        receive_node_id.clone(),
    );

    // Build application state
    let app_state = AppState {
        db_pool,
        lnd_receive,
        lnd_send,
        broadcast_tx,
        receive_node_id,
        send_node_id,
    };

    // Get Leptos configuration
    let leptos_options = get_configuration(None)
        .expect("Failed to load Leptos configuration")
        .leptos_options;

    let addr = leptos_options.site_addr;

    // Build the application router
    let app = build_router(app_state, leptos_options);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on http://{addr}");
    tracing::info!("API endpoints:");
    tracing::info!("  POST /api/invoice");
    tracing::info!("  GET  /api/invoice/:payment_hash");
    tracing::info!("  POST /api/payment");
    tracing::info!("  GET  /api/payment/:payment_hash");
    tracing::info!("  GET  /api/transactions");
    tracing::info!("  GET  /api/balance");
    tracing::info!("  GET  /events (SSE)");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
