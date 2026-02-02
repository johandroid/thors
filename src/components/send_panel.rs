use leptos::prelude::*;

use crate::components::functions::{
    decode_payment_request_local, format_amount, format_expiry, DecodedInvoice,
};
use crate::dto::PaymentResponse;
use crate::server::functions::pay_invoice_fn;

/// Panel for sending Lightning payments (paying invoices)
#[component]
pub fn SendPanel(
    /// Increment to clear the panel state (used by ReceivePanel)
    clear_nonce: ReadSignal<u64>,
    /// Called when the user starts a payment
    on_pay_invoice: Callback<()>,
) -> impl IntoView {
    let (payment_request, set_payment_request) = signal(String::new());
    let (decoded_invoice, set_decoded_invoice) = signal(None::<DecodedInvoice>);
    let (decode_error, set_decode_error) = signal(None::<String>);
    let (payment_result, set_payment_result) = signal(None::<PaymentResponse>);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let reset_panel = move || {
        set_payment_request.set(String::new());
        set_decoded_invoice.set(None);
        set_decode_error.set(None);
        set_payment_result.set(None);
        set_error.set(None);
    };

    let last_clear_nonce = RwSignal::new(clear_nonce.get());
    Effect::new(move |_| {
        let current = clear_nonce.get();
        if current != last_clear_nonce.get_untracked() {
            last_clear_nonce.set(current);
            reset_panel();
        }
    });

    let on_input = move |ev| {
        let value = event_target_value(&ev);
        set_payment_request.set(value.clone());

        if value.trim().is_empty() {
            set_decoded_invoice.set(None);
            set_decode_error.set(None);
            return;
        }

        match decode_payment_request_local(&value) {
            Ok(decoded) => {
                set_decoded_invoice.set(Some(decoded));
                set_decode_error.set(None);
            }
            Err(_) => {
                set_decoded_invoice.set(None);
                set_decode_error.set(Some("Invalid invoice".to_string()));
            }
        }
    };

    let on_submit = move |_| {
        let pr = payment_request.get();
        if pr.is_empty() {
            set_error.set(Some("Please enter a payment request".to_string()));
            return;
        }

        on_pay_invoice.run(());

        set_loading.set(true);
        set_error.set(None);
        set_payment_result.set(None);
        set_payment_request.set(String::new());
        set_decoded_invoice.set(None);
        set_decode_error.set(None);

        leptos::task::spawn_local(async move {
            match pay_invoice_fn(pr).await {
                Ok(response) => {
                    set_payment_result.set(Some(response));
                    set_error.set(None);
                }
                Err(e) => {
                    set_error.set(Some(format!("Payment failed: {}", e)));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="panel send-panel">
            <h2>"Send Payment"</h2>

            <div class="form-group">
                <label for="payment_request">"Lightning Invoice"</label>
                <textarea
                    id="payment_request"
                    class="input input-mono textarea-auto"
                    rows="7"
                    placeholder="lnbc..."
                    prop:value=payment_request
                    on:input=on_input
                />
            </div>

            <Show when=move || decode_error.get().is_some()>
                <div class="error-message">
                    {move || decode_error.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || decoded_invoice.get().is_some()>
                <div class="invoice-preview">
                    <h3>"Invoice Details"</h3>
                    {move || {
                        decoded_invoice.get().map(|decoded| {
                            let amount = format_amount(decoded.amount_msats);
                            let description = decoded
                                .description
                                .unwrap_or_else(|| "No message".to_string());
                            let expiry = format_expiry(decoded.expiry_seconds);
                            view! {
                                <div class="invoice-details">
                                    <p>
                                        <strong>"Amount: "</strong>
                                        {amount}
                                    </p>
                                    <p>
                                        <strong>"Message: "</strong>
                                        {description}
                                    </p>
                                    <p>
                                        <strong>"Expiry: "</strong>
                                        {format!("{} ({}s)", expiry, decoded.expiry_seconds)}
                                    </p>
                                </div>
                            }
                        })
                    }}
                </div>
            </Show>

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
                {move || if loading.get() { "Paying..." } else { "Pay Invoice" }}
            </button>

            <Show when=move || payment_result.get().is_some()>
                <div class="payment-success">
                    <h3>"Payment Successful!"</h3>
                    {move || {
                        payment_result.get().map(|result| view! {
                            <div class="payment-details">
                                <p>
                                    <strong>"Amount: "</strong>
                                    {result.amount_sats}" sats"
                                </p>
                                <p>
                                    <strong>"Payment Hash: "</strong>
                                    <code>{result.payment_hash}</code>
                                </p>
                                <p>
                                    <strong>"Preimage: "</strong>
                                    <code>{result.preimage}</code>
                                </p>
                            </div>
                        })
                    }}
                </div>
            </Show>
        </div>
    }
}
