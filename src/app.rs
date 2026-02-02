use leptos::prelude::*;
use leptos_meta::*;

use crate::components::*;

/// SSR shell: provides the full HTML document structure for server-side rendering.
/// This is called by `leptos_routes_with_context` in main.rs.
#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/thors.css"/>
        <Title text="THOrs Payments"/>
        <HomePage/>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let (clear_receive_nonce, set_clear_receive_nonce) = signal(0u64);
    let (clear_send_nonce, set_clear_send_nonce) = signal(0u64);

    let clear_receive = Callback::new(move |_| {
        set_clear_receive_nonce.update(|value| *value += 1);
    });
    let clear_send = Callback::new(move |_| {
        set_clear_send_nonce.update(|value| *value += 1);
    });

    view! {
        <div class="container">
            <header class="app-header">
                <h1>"⚡ THOrs Payments"</h1>
                <p class="subtitle">"Lightning network invoices payment example by JohanDroid ❤️"</p>
            </header>

            <main class="app-main">
                <div class="top-row">
                    <BalanceDisplay/>
                </div>

                <div class="panels-row">
                    <ReceivePanel
                        clear_nonce=clear_receive_nonce
                        on_create_invoice=clear_send
                    />
                    <SendPanel
                        clear_nonce=clear_send_nonce
                        on_pay_invoice=clear_receive
                    />
                </div>

                <div class="bottom-row">
                    <TransactionList/>
                </div>
            </main>
        </div>
    }
}
