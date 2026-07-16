use std::path::PathBuf;
use std::process::ExitCode;

use super::args::CliArgs;

pub fn run(_args: &CliArgs) -> ExitCode {
    println!("whytho. setup — guided configuration");
    println!();

    // Check for existing config
    let config_paths = [
        PathBuf::from("whytho.toml"),
        dirs_config_dir().join("whytho").join("config.toml"),
    ];

    let mut existing_config = None;
    for path in &config_paths {
        if path.exists() {
            existing_config = Some(path);
            break;
        }
    }

    if let Some(path) = existing_config {
        println!("Found existing config: {}", path.display());
        println!("To reconfigure, edit the file directly or delete it and run setup again.");
        return ExitCode::SUCCESS;
    }

    // Generate default config
    let default_config = r#"# whytho. configuration
# See https://whytho.dev/config for all options

[defaults]
preset = "av1-balanced"
container = "mkv"

[concurrency]
max_jobs = 3
cpu_workers = "auto"
chunking = true

[video]
default_codec = "av1"
bitrate_strategy = "half-source-if-h264"

[audio]
default_codec = "opus"
preserve_unless_requested = true

[verification]
mode = "sample"

[file_ops]
replace_original = false
purge_partial_on_cancel = true
"#;

    // Write to current directory
    let output_path = PathBuf::from("whytho.toml");
    match std::fs::write(&output_path, default_config) {
        Ok(()) => {
            println!("Created default config: {}", output_path.display());
            println!();
            println!("Next steps:");
            println!("  1. Edit whytho.toml to customize settings");
            println!("  2. Run `whytho plan <file>` to preview a transcode");
            println!("  3. Run `whytho run <file>` to transcode");
            println!("  4. Run `whytho verify <original> <encoded>` to check quality");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error writing config: {e}");
            ExitCode::from(1)
        }
    }
}

fn dirs_config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|_| PathBuf::from("."))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from(".")
    }
}
