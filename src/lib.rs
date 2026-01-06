pub mod analyzer;
pub mod config;
pub mod db;
pub mod hardware;
pub mod orchestrator;
pub mod scanner;
pub mod server;

pub mod app;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    use leptos::*;

    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    mount_to_body(App);
}
