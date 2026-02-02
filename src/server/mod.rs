// Server functions module is always available (contains #[server] macros)
pub mod functions;

// Server-only modules
#[cfg(feature = "ssr")]
pub mod api;
#[cfg(feature = "ssr")]
pub mod background;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod lnd;
#[cfg(feature = "ssr")]
pub mod sse;

// Re-export commonly used types (SSR only)
pub use crate::dto::InvoiceEvent;
#[cfg(feature = "ssr")]
pub use db::{create_pool, DbPool};
#[cfg(feature = "ssr")]
pub use functions::AppState;
#[cfg(feature = "ssr")]
pub use lnd::{LightningClients, LndError};
