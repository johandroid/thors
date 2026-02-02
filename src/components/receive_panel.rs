use leptos::prelude::*;

use crate::components::functions::format_expiry;
use crate::components::QrCode;
use crate::server::functions::create_invoice_fn;

/// Panel for receiving Lightning payments (generating invoices)
#[component]
pub fn ReceivePanel(
    /// Increment to clear the panel state (used by SendPanel)
    clear_nonce: ReadSignal<u64>,
    /// Called when the user starts creating an invoice
    on_create_invoice: Callback<()>,
) -> impl IntoView {
    let (created_amount_sats, set_created_amount_sats) = signal(None::<i64>);
    let (created_description, set_created_description) = signal(None::<String>);
    let (created_expiry_seconds, set_created_expiry_seconds) = signal(None::<u64>);
    let (amount, set_amount) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (invoice, set_invoice) = signal(String::new());
    let (payment_hash, set_payment_hash) = signal(None::<String>);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (copied, set_copied) = signal(false);

    let has_invoice = move || !invoice.get().is_empty();

    let reset_panel = move || {
        set_amount.set(String::new());
        set_description.set(String::new());
        set_invoice.set(String::new());
        set_payment_hash.set(None);
        set_created_amount_sats.set(None);
        set_created_description.set(None);
        set_created_expiry_seconds.set(None);
        set_error.set(None);
        set_copied.set(false);
    };

    let last_clear_nonce = RwSignal::new(clear_nonce.get());
    Effect::new(move |_| {
        let current = clear_nonce.get();
        if current != last_clear_nonce.get_untracked() {
            last_clear_nonce.set(current);
            reset_panel();
        }
    });

    let on_submit = move |_| {
        let amount_sats = match amount.get().parse::<i64>() {
            Ok(amt) if amt > 0 => amt,
            _ => {
                set_error.set(Some("Invalid amount".to_string()));
                return;
            }
        };

        on_create_invoice.run(());

        let desc = if description.get().is_empty() {
            None
        } else {
            Some(description.get())
        };

        set_loading.set(true);
        set_error.set(None);
        set_invoice.set(String::new());
        set_payment_hash.set(None);
        set_created_amount_sats.set(None);
        set_created_description.set(None);
        set_created_expiry_seconds.set(None);
        set_copied.set(false);

        leptos::task::spawn_local(async move {
            let expiry_seconds = 3600u64;

            match create_invoice_fn(amount_sats, desc.clone()).await {
                Ok(response) => {
                    set_invoice.set(response.payment_request);
                    set_payment_hash.set(Some(response.payment_hash));
                    set_created_amount_sats.set(Some(amount_sats));
                    set_created_description.set(desc.clone());
                    set_created_expiry_seconds.set(Some(expiry_seconds));
                    set_error.set(None);
                    set_amount.set(String::new());
                    set_description.set(String::new());
                }
                Err(e) => {
                    set_error.set(Some(format!("Error creating invoice: {}", e)));
                }
            }
            set_loading.set(false);
        });
    };

    let copy_invoice = move |_| {
        #[cfg(not(feature = "ssr"))]
        {
            let inv = invoice.get();
            if !inv.is_empty() {
                if let Some(window) = web_sys::window() {
                    let navigator = window.navigator();
                    let clipboard = navigator.clipboard();
                    let _ = clipboard.write_text(&inv);
                    set_copied.set(true);
                }
            }
        }
    };

    view! {
        <div class="panel receive-panel">
            <h2>"Receive Payment"</h2>

            <div class="form-group">
                <label for="amount">"Amount (sats)"</label>
                <input
                    id="amount"
                    type="number"
                    class="input"
                    placeholder="1000"
                    prop:value=amount
                    on:input=move |ev| set_amount.set(event_target_value(&ev))
                />
            </div>

            <div class="form-group">
                <label for="description">"Description (optional)"</label>
                <input
                    id="description"
                    type="text"
                    class="input"
                    placeholder="Payment for..."
                    prop:value=description
                    on:input=move |ev| set_description.set(event_target_value(&ev))
                />
            </div>

            <Show when=move || error.get().is_some()>
                <div class="error-message">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <button
                class="btn btn-primary"
                on:click=on_submit
                disabled=move || loading.get()
            >
                {move || if loading.get() { "Creating..." } else { "Create Invoice" }}
            </button>

            <Show when=has_invoice>
                <div class="invoice-result">
                    <h3>"Invoice Created!"</h3>

                    <div class="qr-container">
                        <QrCode data=Signal::derive(move || invoice.get()) />
                    </div>

                    <div class="invoice-display">
                        <code class="invoice-string">
                            {move || invoice.get()}
                        </code>
                        <button
                            class="btn btn-secondary"
                            on:click=copy_invoice
                        >
                            {move || if copied.get() { "Copied!" } else { "Copy 📝" }}
                        </button>
                    </div>

                    <div class="invoice-details">
                        <p>
                            <strong>"Amount: "</strong>
                            {move || {
                                created_amount_sats
                                    .get()
                                    .map(|amt| format!("{} sats", amt))
                                    .unwrap_or_else(|| "-".to_string())
                            }}
                        </p>
                        <p>
                            <strong>"Description: "</strong>
                            {move || {
                                created_description
                                    .get()
                                    .unwrap_or_else(|| "No message".to_string())
                            }}
                        </p>
                        <p>
                            <strong>"Expiry: "</strong>
                            {move || {
                                created_expiry_seconds
                                    .get()
                                    .map(|secs| format!("{} ({}s)", format_expiry(secs), secs))
                                    .unwrap_or_else(|| "-".to_string())
                            }}
                        </p>
                        <Show when=move || payment_hash.get().is_some()>
                            <p class="payment-hash">
                                <strong>"Payment Hash: "</strong>
                                <code>{move || payment_hash.get().unwrap_or_default()}</code>
                            </p>
                        </Show>
                    </div>
                </div>
            </Show>
        </div>
    }
}
