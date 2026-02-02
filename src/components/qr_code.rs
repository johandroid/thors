use leptos::prelude::*;

/// Renders an SVG QR code from a data string (e.g., a Lightning invoice).
/// Uses the `qrcode` crate to generate the QR code as an inline SVG.
#[component]
pub fn QrCode(
    /// The data to encode in the QR code
    #[prop(into)]
    data: Signal<String>,
) -> impl IntoView {
    let svg_string = move || {
        let data_val = data.get();
        if data_val.is_empty() {
            return String::new();
        }

        match qrcode::QrCode::new(data_val.as_bytes()) {
            Ok(code) => {
                let svg = code
                    .render::<qrcode::render::svg::Color>()
                    .min_dimensions(200, 200)
                    .dark_color(qrcode::render::svg::Color("#000000"))
                    .light_color(qrcode::render::svg::Color("#ffffff"))
                    .quiet_zone(true)
                    .build();
                svg
            }
            Err(_) => String::from("<p>Failed to generate QR code</p>"),
        }
    };

    view! {
        <div class="qr-code" inner_html=svg_string />
    }
}
