pub mod analyzer;
pub mod config;
pub mod db;
pub mod hardware;
pub mod scanner;

#[cfg(feature = "ssr")]
pub mod orchestrator;
#[cfg(feature = "ssr")]
pub mod processor;
#[cfg(feature = "ssr")]
pub mod server;

pub mod app;

#[cfg(feature = "ssr")] pub use orchestrator::Orchestrator;
#[cfg(feature = "ssr")] pub use processor::Processor;
pub use db::AlchemistEvent;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    use leptos::*;

    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    mount_to_body(App);
}
