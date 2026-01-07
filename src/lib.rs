#[cfg(feature = "ssr")]
pub mod analyzer;
pub mod config;
pub mod db;
pub mod error;
#[cfg(feature = "ssr")]
pub mod ffmpeg;
pub mod hardware;
#[cfg(feature = "ssr")]
pub mod notifications;
#[cfg(feature = "ssr")]
pub mod scanner;
#[cfg(feature = "ssr")]
pub mod watcher;

#[cfg(feature = "ssr")]
pub mod orchestrator;
#[cfg(feature = "ssr")]
pub mod processor;
#[cfg(feature = "ssr")]
pub mod server;
#[cfg(feature = "ssr")]
pub mod wizard;

pub mod app;

pub use config::QualityProfile;
pub use db::AlchemistEvent;
#[cfg(feature = "ssr")]
pub use ffmpeg::{EncodeStats, EncoderCapabilities, HardwareAccelerators};
#[cfg(feature = "ssr")]
pub use notifications::NotificationService;
#[cfg(feature = "ssr")]
pub use orchestrator::Transcoder;
#[cfg(feature = "ssr")]
pub use processor::Agent;
#[cfg(feature = "ssr")]
pub use watcher::FileWatcher;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    use leptos::*;

    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    mount_to_body(App);
}
