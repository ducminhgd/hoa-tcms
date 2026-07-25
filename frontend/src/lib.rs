//! HOA TCMS — Leptos frontend (Client-Side Rendering).
//!
//! This crate contains the WASM client that runs in the browser. It calls the
//! REST API served by the Actix-Web backend.

pub mod api;
pub mod app;
pub mod components;
pub mod layouts;
pub mod pages;

use wasm_bindgen::prelude::*;

/// Entry point called from the HTML page on load.
#[wasm_bindgen(start)]
pub fn main() {
    console_log::init_with_level(log::Level::Debug).ok();
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
