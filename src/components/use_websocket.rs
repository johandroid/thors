use crate::dto::InvoiceEvent;
use codee::string::JsonSerdeCodec;
use leptos::prelude::*;
use leptos_use::{use_event_source_with_options, UseEventSourceOptions, UseEventSourceReturn};

/// Hook to connect to SSE endpoint and receive real-time invoice events.
/// Uses leptos_use::use_event_source with automatic reconnection.
pub fn use_websocket_events() -> ReadSignal<Option<InvoiceEvent>> {
    let (event, set_event) = signal(None::<InvoiceEvent>);

    let UseEventSourceReturn { message, .. } =
        use_event_source_with_options::<InvoiceEvent, JsonSerdeCodec>(
            "/events",
            UseEventSourceOptions::default()
                .reconnect_limit(leptos_use::ReconnectLimit::Infinite)
                .reconnect_interval(3000),
        );

    Effect::new(move |_| {
        if let Some(msg) = message.get() {
            set_event.set(Some(msg.data));
        }
    });

    event
}
