use crate::components::use_websocket_events;
use crate::server::functions::get_balance_fn;
use leptos::prelude::*;

/// Component to display current balance with real-time updates
#[component]
pub fn BalanceDisplay() -> impl IntoView {
    let ws_event = use_websocket_events();

    // LocalResource for WASM compatibility (not Send)
    // Refetch when WebSocket events arrive
    let balance = LocalResource::new(move || {
        let _trigger = ws_event.get(); // Trigger refetch on WS event
        async move { get_balance_fn().await.ok() }
    });

    view! {
        <div class="panel balance-display">
            <h2>"Balance"</h2>

            <Transition fallback=|| view! { <p>"Loading balance..."</p> }>
                {move || Suspend::new(async move {
                    match balance.await {
                        Some(bal) => {
                            view! {
                                <div class="balance-grid">
                                    <div class="balance-item">
                                        <span class="balance-label">"Total Balance"</span>
                                        <span class="balance-value balance-total">
                                            {bal.total_balance}" sats"
                                        </span>
                                    </div>

                                    <div class="balance-item">
                                        <span class="balance-label">"Received"</span>
                                        <span class="balance-value balance-received">
                                            ""{bal.received_sats}" sats"
                                        </span>
                                    </div>

                                    <div class="balance-item">
                                        <span class="balance-label">"Paid"</span>
                                        <span class="balance-value balance-paid">
                                            ""{bal.paid_sats}" sats"
                                        </span>
                                    </div>

                                    <div class="balance-updated">
                                        <small>"Last updated: "{bal.last_updated}</small>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        None => {
                            view! {
                                <p class="error-message">"Failed to load balance"</p>
                            }.into_any()
                        }
                    }
                })}
            </Transition>
        </div>
    }
}
