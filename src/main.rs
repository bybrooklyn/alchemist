use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
mod analyzer;
mod hardware;
mod orchestrator;
mod scanner;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directories to scan for media files
    #[arg(required = true)]
    directories: Vec<PathBuf>,

    /// Dry run (don't actually transcode)
    #[arg(short, long)]
    dry_run: bool,

    /// Output directory (optional, defaults to same as input with .av1)
    #[arg(short, long)]
    output_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Alchemist starting...");

    let args = Args::parse();

    // 1. Hardware Detection
    let hw_info = match hardware::detect_hardware() {
        Ok(info) => {
            info!("Hardware detected: {}", info.vendor);
            info
        }
        Err(e) => {
            error!("{}", e);
            if !args.dry_run {
                warn!("Hardware missing. Exiting as requested by security gates.");
                return Err(e);
            }
            warn!("Hardware missing, but continuing due to dry_run/analysis mode (using dummy Intel config).");
            hardware::HardwareInfo {
                vendor: hardware::Vendor::Intel,
                device_path: Some("/dev/dri/renderD129".to_string()),
            }
        }
    };

    // 2. Scan directories
    let scanner = scanner::Scanner::new();
    let files = scanner.scan(args.directories);

    if files.is_empty() {
        info!("No media files found to process.");
        return Ok(());
    }

    // 3. Process Queue
    let orchestrator = orchestrator::Orchestrator::new();
    
    for file_path in files {
        info!("--- Analyzing: {:?} ---", file_path.file_name().unwrap_or_default());
        
        // Preflight Analysis
        match analyzer::Analyzer::probe(&file_path) {
            Ok(metadata) => {
                let (should_encode, reason) = analyzer::Analyzer::should_transcode(&file_path, &metadata);
                
                if should_encode {
                    info!("Decision: ENCODE - {}", reason);
                    
                    let mut output_path = file_path.clone();
                    output_path.set_extension("av1.mkv");
                    
                    if let Err(e) = orchestrator.transcode_to_av1(&file_path, &output_path, &hw_info, args.dry_run) {
                        error!("Transcode failed for {:?}: {}", file_path, e);
                    }
                } else {
                    info!("Decision: SKIP - {}", reason);
                }
            }
            Err(e) => {
                error!("Failed to probe {:?}: {}", file_path, e);
            }
        }
    }

    Ok(())
}


