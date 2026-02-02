#![recursion_limit = "512"]

pub mod dto;
pub mod errors;
pub mod models;
pub mod schema;

pub mod components;
#[cfg(feature = "ssr")]
pub mod initialize;

#[cfg(any(feature = "ssr", feature = "hydrate"))]
pub mod server;

pub mod app;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
