//! HOA TCMS — Leptos frontend library (shared by WASM and SSR).

pub mod app;
pub mod pages;

/// Base URL of the HOA TCMS backend API.
///
/// Override at build time with the `TCMS_API_BASE` environment variable
/// (e.g. via the cargo-leptos `.env` file). Defaults to the local dev backend.
pub const API_BASE: &str = match option_env!("TCMS_API_BASE") {
    Some(v) => v,
    None => "http://localhost:8080",
};

/// Client-side hydration entry point, called by cargo-leptos in the browser.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);

    leptos::mount::hydrate_body(|| leptos::prelude::view! { <app::App /> });
}
