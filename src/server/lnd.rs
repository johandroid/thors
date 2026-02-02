use std::sync::Arc;
use tokio::sync::Mutex;
use tonic_lnd::{lnrpc, tonic, Client as LndClient};

#[derive(Debug, thiserror::Error)]
pub enum LndError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("RPC error: {0}")]
    Rpc(#[from] tonic::Status),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct LightningClients {
    pub client: Arc<Mutex<LndClient>>,
}

/// Create a persistent LND connection. Returns the raw client.
pub async fn connect(
    endpoint: String,
    cert_path: String,
    macaroon_path: String,
) -> Result<LndClient, LndError> {
    tonic_lnd::connect(endpoint, cert_path, macaroon_path)
        .await
        .map_err(|e| LndError::Connection(e.to_string()))
}

/// Fetch the node's public key (identity) for labeling transactions.
pub async fn get_node_pubkey(client: &mut LndClient) -> Result<String, LndError> {
    let response = client
        .lightning()
        .get_info(lnrpc::GetInfoRequest {})
        .await?
        .into_inner();

    Ok(response.identity_pubkey)
}

impl LightningClients {
    pub fn from_client(client: LndClient) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    pub async fn create_invoice(
        &self,
        amount_sats: i64,
        memo: Option<String>,
    ) -> Result<lnrpc::AddInvoiceResponse, LndError> {
        let memo = memo.unwrap_or_default();
        tracing::info!(amount_sats, memo = %memo, "Creating invoice");
        let invoice = lnrpc::Invoice {
            value: amount_sats,
            memo,
            expiry: 3600, // 1 hour
            ..Default::default()
        };

        tracing::info!(?invoice, "Prepared invoice");
        let mut client = self.client.lock().await;
        tracing::info!("Locked LND client for creating invoice");
        let response = client.lightning().add_invoice(invoice).await?.into_inner();
        tracing::info!(?response.r_hash, "Created invoice with r_hash");

        Ok(response)
    }

    pub async fn decode_payment_request(
        &self,
        payment_request: String,
    ) -> Result<lnrpc::PayReq, LndError> {
        let request = lnrpc::PayReqString {
            pay_req: payment_request,
        };

        let mut client = self.client.lock().await;
        let response = client
            .lightning()
            .decode_pay_req(request)
            .await?
            .into_inner();

        Ok(response)
    }

    pub async fn send_payment(
        &self,
        payment_request: String,
    ) -> Result<lnrpc::SendResponse, LndError> {
        let request = lnrpc::SendRequest {
            payment_request,
            fee_limit: Some(lnrpc::FeeLimit {
                limit: Some(lnrpc::fee_limit::Limit::Percent(5)),
            }),
            ..Default::default()
        };

        let mut client = self.client.lock().await;
        let response = client
            .lightning()
            .send_payment_sync(request)
            .await?
            .into_inner();

        Ok(response)
    }
}
