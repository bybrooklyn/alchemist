pub mod config;
pub mod db;
pub mod error;
pub mod media;
pub mod notifications;
pub mod orchestrator;
pub mod scheduler;
pub mod server;
pub mod system;
pub mod telemetry;
pub mod wizard;

pub use config::QualityProfile;
pub use db::AlchemistEvent;
pub use media::ffmpeg::{EncodeStats, EncoderCapabilities, HardwareAccelerators};
pub use media::processor::Agent;
pub use orchestrator::Transcoder;
// pub use system::notifications::NotificationService; // Deprecated user-facing export?
pub use system::watcher::FileWatcher;
