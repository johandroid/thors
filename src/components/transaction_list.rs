#[cfg(not(feature = "ssr"))]
use crate::components::use_websocket_events;
use crate::models::Transaction;
use leptos::prelude::*;

#[cfg(not(feature = "ssr"))]
use crate::dto::InvoiceEvent;
#[cfg(not(feature = "ssr"))]
use crate::server::functions::get_transactions_fn;

/// Component to display transaction history with real-time updates.
/// Loads the full list once on mount, then reactively updates individual
/// entries when WebSocket events arrive (no full refetch needed).
#[component]
pub fn TransactionList() -> impl IntoView {
    let (transactions, _set_transactions) = signal(Vec::<Transaction>::new());
    let (loading, _set_loading) = signal(true);
    let (expanded_id, set_expanded_id) = signal(None::<i64>);

    // Load initial transactions on mount
    #[cfg(not(feature = "ssr"))]
    let set_transactions = _set_transactions;
    #[cfg(not(feature = "ssr"))]
    let set_loading = _set_loading;
    #[cfg(not(feature = "ssr"))]
    let ws_event = use_websocket_events();

    #[cfg(not(feature = "ssr"))]
    {
        let set_transactions = set_transactions.clone();
        let set_loading = set_loading.clone();
        leptos::task::spawn_local(async move {
            match get_transactions_fn(Some(50), Some(0)).await {
                Ok(txs) => set_transactions.set(txs),
                Err(_) => set_transactions.set(Vec::new()),
            }
            set_loading.set(false);
        });
    }

    // React to WebSocket events: update the list in-place
    #[cfg(not(feature = "ssr"))]
    {
        let set_transactions = set_transactions.clone();
        Effect::new(move |_| {
            if let Some(event) = ws_event.get() {
                let tx = match &event {
                    InvoiceEvent::InvoiceCreated { tx } => tx.clone(),
                    InvoiceEvent::InvoiceSettled { tx } => tx.clone(),
                    InvoiceEvent::InvoiceExpired { tx } => tx.clone(),
                    InvoiceEvent::PaymentSucceeded { tx } => tx.clone(),
                };

                let tx_type = tx.tx_type();
                set_transactions.update(|txs| {
                    if let Some(existing) = txs
                        .iter_mut()
                        .find(|t| t.payment_hash == tx.payment_hash && t.tx_type() == tx_type)
                    {
                        *existing = tx;
                    } else {
                        txs.insert(0, tx);
                    }
                });
            }
        });
    }

    view! {
        <div class="panel transaction-list">
            <h2>"Transaction History"</h2>

            {move || {
                if loading.get() {
                    view! { <p>"Loading transactions..."</p> }.into_any()
                } else if transactions.get().is_empty() {
                    view! { <p class="empty-state">"No transactions yet"</p> }.into_any()
                } else {
                    view! {
                        <table class="tx-table">
                            <thead>
                                <tr>
                                    <th>"Type"</th>
                                    <th>"Amount"</th>
                                    <th>"Status"</th>
                                    <th>"Description"</th>
                                    <th>"Date (GMT)"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || transactions.get()
                                    key=|tx| tx.id
                                    children=move |tx: Transaction| {
                                        let tx_id = tx.id;
                                        let tx_type = tx.tx_type();
                                        let status = tx.status();
                                        let created_at = tx.created_at.format("%Y-%m-%d %H:%M").to_string();
                                        let created_at_full = tx.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        let updated_at_full = tx.updated_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        let description = tx.description.clone().unwrap_or_else(|| "-".to_string());
                                        let preimage = tx.preimage.clone().unwrap_or_else(|| "-".to_string());
                                        let fee_sats = tx
                                            .fee_sats
                                            .map(|fee| format!("{} sats", fee))
                                            .unwrap_or_else(|| "-".to_string());
                                        let failure_reason = tx.failure_reason.clone().unwrap_or_else(|| "-".to_string());
                                        let expires_at = tx
                                            .expires_at
                                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                            .unwrap_or_else(|| "-".to_string());

                                        let toggle_row = Callback::new(move |_| {
                                            set_expanded_id.update(|current| {
                                                if *current == Some(tx_id) {
                                                    *current = None;
                                                } else {
                                                    *current = Some(tx_id);
                                                }
                                            });
                                        });

                                        view! {
                                            <>
                                                <tr class="tx-row" on:click=move |_| toggle_row.run(())>
                                                    <td>
                                                        <span class={format!("badge badge-{}", match tx_type {
                                                            crate::models::TxType::Invoice => "invoice",
                                                            crate::models::TxType::Payment => "payment",
                                                        })}>
                                                            {match tx_type {
                                                                crate::models::TxType::Invoice => "Invoice",
                                                                crate::models::TxType::Payment => "Payment",
                                                            }}
                                                        </span>
                                                    </td>
                                                    <td class="amount">
                                                        {tx.amount_sats}" sats"
                                                    </td>
                                                    <td>
                                                        <span class={format!("badge badge-{}", match status {
                                                            crate::models::TxStatus::Pending => "pending",
                                                            crate::models::TxStatus::Succeeded => "success",
                                                            crate::models::TxStatus::Failed => "error",
                                                            crate::models::TxStatus::Expired => "expired",
                                                        })}>
                                                            {match status {
                                                                crate::models::TxStatus::Pending => "Pending",
                                                                crate::models::TxStatus::Succeeded => "Succeeded",
                                                                crate::models::TxStatus::Failed => "Failed",
                                                                crate::models::TxStatus::Expired => "Expired",
                                                            }}
                                                        </span>
                                                    </td>
                                                    <td class="description">
                                                        {description.clone()}
                                                    </td>
                                                    <td class="date">
                                                        {created_at}
                                                    </td>
                                                </tr>
                                                <Show when=move || expanded_id.get() == Some(tx_id)>
                                                    <tr class="tx-details">
                                                        <td colspan="5">
                                                            <div class="tx-details__content">
                                                                <div class="tx-details__header">
                                                                    <span class="tx-details__title">"Transaction Details"</span>
                                                                </div>
                                                                <p><strong>"ID: "</strong>{tx.id}</p>
                                                                <p><strong>"Type: "</strong>{
                                                                    match tx_type {
                                                                        crate::models::TxType::Invoice => "Invoice",
                                                                        crate::models::TxType::Payment => "Payment",
                                                                    }
                                                                }</p>
                                                                <p><strong>"Status: "</strong>{
                                                                    match status {
                                                                        crate::models::TxStatus::Pending => "Pending",
                                                                        crate::models::TxStatus::Succeeded => "Succeeded",
                                                                        crate::models::TxStatus::Failed => "Failed",
                                                                        crate::models::TxStatus::Expired => "Expired",
                                                                    }
                                                                }</p>
                                                                <p><strong>"Amount: "</strong>{tx.amount_sats}" sats"</p>
                                                                <p><strong>"Description: "</strong>{description.clone()}</p>
                                                                <p><strong>"Payment Hash: "</strong><code>{tx.payment_hash.clone()}</code></p>
                                                                <p><strong>"Payment Request: "</strong><code>{tx.payment_request.clone()}</code></p>
                                                                <p><strong>"Preimage: "</strong><code>{preimage.clone()}</code></p>
                                                                <p><strong>"Fee: "</strong>{fee_sats.clone()}</p>
                                                                <p><strong>"Failure Reason: "</strong>{failure_reason.clone()}</p>
                                                                <p><strong>"Expires At (UTC): "</strong>{expires_at.clone()}</p>
                                                                <p><strong>"Node ID: "</strong><code>{tx.node_id.clone()}</code></p>
                                                                <p><strong>"Created At (UTC): "</strong>{created_at_full.clone()}</p>
                                                                <p><strong>"Updated At (UTC): "</strong>{updated_at_full.clone()}</p>
                                                                <button
                                                                        class="btn btn-secondary btn-inline"
                                                                        on:click=move |_| toggle_row.run(())
                                                                        type="button"
                                                                    >
                                                                        "Close"
                                                                    </button>
                                                            </div>
                                                        </td>
                                                    </tr>
                                                </Show>
                                            </>
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                    }.into_any()
                }
            }}
        </div>
    }
}
