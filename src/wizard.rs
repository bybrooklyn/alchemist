use crate::config::Config;
use crate::error::{AlchemistError, Result};
use inquire::{Confirm, Select, Text};
use std::path::Path;

/// Interactive configuration wizard
pub struct ConfigWizard;

impl ConfigWizard {
    /// Run the configuration wizard and create config.toml
    pub fn run(config_path: &Path) -> Result<Config> {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║              ⚗️  ALCHEMIST CONFIGURATION WIZARD              ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        println!("Welcome! This wizard will help you configure Alchemist.");
        println!("Press Enter to accept the default value shown in [brackets].\n");

        if config_path.exists() {
            let overwrite = Confirm::new("config.toml already exists. Overwrite it?")
                .with_default(false)
                .prompt()
                .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

            if !overwrite {
                return Err(AlchemistError::Config("Configuration cancelled".into()));
            }
        }

        // Section 1: Transcoding
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ Section 1/3: Transcoding Settings      │");
        println!("└─────────────────────────────────────────┘\n");

        let size_threshold = Self::prompt_size_threshold()?;
        let min_bpp = Self::prompt_min_bpp()?;
        let min_file_size = Self::prompt_min_file_size()?;
        let concurrent_jobs = Self::prompt_concurrent_jobs()?;

        // Section 2: Hardware
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ Section 2/3: Hardware Settings         │");
        println!("└─────────────────────────────────────────┘\n");

        let allow_cpu_fallback = Self::prompt_cpu_fallback()?;
        let allow_cpu_encoding = Self::prompt_cpu_encoding()?;
        let cpu_preset = Self::prompt_cpu_preset()?;
        let preferred_vendor = Self::prompt_preferred_vendor()?;

        // Section 3: Scanner
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ Section 3/3: Scanner Settings          │");
        println!("└─────────────────────────────────────────┘\n");

        let directories = Self::prompt_directories()?;

        // Build config
        let config = Config {
            transcode: crate::config::TranscodeConfig {
                size_reduction_threshold: size_threshold,
                min_bpp_threshold: min_bpp,
                min_file_size_mb: min_file_size,
                concurrent_jobs,
                threads: 0,
                quality_profile: crate::config::QualityProfile::Balanced,
                output_codec: crate::config::OutputCodec::Av1,
                subtitle_mode: crate::config::SubtitleMode::Copy,
            },
            hardware: crate::config::HardwareConfig {
                preferred_vendor,
                device_path: None,
                allow_cpu_fallback,
                cpu_preset,
                allow_cpu_encoding,
            },
            scanner: crate::config::ScannerConfig {
                directories,
                watch_enabled: false,
            },
            notifications: crate::config::NotificationsConfig::default(),
            quality: crate::config::QualityConfig::default(),
            system: crate::config::SystemConfig::default(),
        };

        // Show summary
        Self::show_summary(&config);

        let confirm = Confirm::new("Save this configuration?")
            .with_default(true)
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        if !confirm {
            return Err(AlchemistError::Config("Configuration cancelled".into()));
        }

        // Write to file
        Self::write_config(config_path, &config)?;

        println!("\n✅ Configuration saved to {}", config_path.display());
        println!("\nYou can now use Alchemist!");
        println!("  • Run: alchemist --server");
        println!("  • Edit: {}\n", config_path.display());

        Ok(config)
    }

    fn prompt_size_threshold() -> Result<f64> {
        println!("📏 Size Reduction Threshold");
        println!("   How much smaller must the output file be to keep it?\n");
        println!("   • 0.3 (30%) - Balanced (recommended)");
        println!("   • 0.2 (20%) - More aggressive");
        println!("   • 0.5 (50%) - Conservative\n");
        println!("   Files that don't compress enough are kept as-is.\n");

        let input = Text::new("Size reduction threshold:")
            .with_default("0.3")
            .with_help_message("Enter a value between 0.0 and 1.0")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        input
            .parse::<f64>()
            .map_err(|_| AlchemistError::Config("Invalid number".into()))
    }

    fn prompt_min_bpp() -> Result<f64> {
        println!("\n🎨 Minimum Bits Per Pixel (BPP)");
        println!("   Skip files that are already heavily compressed.\n");
        println!("   • 0.10 - Good default (recommended)");
        println!("   • 0.05 - Very aggressive");
        println!("   • 0.20 - Conservative\n");
        println!("   Lower BPP = already compressed. Re-encoding destroys quality.\n");

        let input = Text::new("Minimum BPP threshold:")
            .with_default("0.1")
            .with_help_message("Typical range: 0.05 to 0.30")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        input
            .parse::<f64>()
            .map_err(|_| AlchemistError::Config("Invalid number".into()))
    }

    fn prompt_min_file_size() -> Result<u64> {
        println!("\n📦 Minimum File Size");
        println!("   Skip small files to avoid wasting time.\n");
        println!("   • 50 MB  - Good for most libraries (recommended)");
        println!("   • 100 MB - Focus on movies/TV episodes");
        println!("   • 10 MB  - Process everything\n");

        let input = Text::new("Minimum file size (MB):")
            .with_default("50")
            .with_help_message("Files smaller than this are skipped")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        input
            .parse::<u64>()
            .map_err(|_| AlchemistError::Config("Invalid number".into()))
    }

    fn prompt_concurrent_jobs() -> Result<usize> {
        println!("\n⚡ Concurrent Jobs");
        println!("   How many videos to transcode simultaneously.\n");
        println!("   • 1 - Safe default (CPU or single GPU)");
        println!("   • 2-4 - Powerful systems with dedicated GPU");
        println!("   • More = faster BUT uses more resources\n");
        println!("   ⚠️  CPU users: Use 1 only (CPU is very slow)\n");

        let input = Text::new("Number of concurrent jobs:")
            .with_default("1")
            .with_help_message("Recommended: 1-2 for most systems")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        input
            .parse::<usize>()
            .map_err(|_| AlchemistError::Config("Invalid number".into()))
    }

    fn prompt_cpu_fallback() -> Result<bool> {
        println!("💻 CPU Fallback");
        println!("   Allow software encoding if no GPU is detected?\n");
        println!("   If enabled: App uses CPU when no GPU found (SLOW)");
        println!("   If disabled: App fails to start without GPU\n");
        println!("   ⚠️  CPU encoding is 10-50x slower than GPU!\n");

        Confirm::new("Enable CPU fallback?")
            .with_default(true)
            .with_help_message("Recommended: yes for compatibility")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))
    }

    fn prompt_cpu_encoding() -> Result<bool> {
        println!("\n🔧 CPU Encoding");
        println!("   Explicitly allow CPU encoding in production?\n");
        println!("   Safety flag to prevent accidental slow transcodes.");
        println!("   Even with fallback enabled, you can block CPU jobs.\n");

        Confirm::new("Allow CPU encoding?")
            .with_default(true)
            .with_help_message("Recommended: yes (same as fallback)")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))
    }

    fn prompt_cpu_preset() -> Result<crate::config::CpuPreset> {
        println!("\n⚙️  CPU Encoding Preset");
        println!("   How fast/slow should CPU encoding be?\n");
        println!("   • slow (0-4)   - Best quality, very slow");
        println!("   • medium (5-8) - Balanced (recommended)");
        println!("   • fast (9-12)  - Lower quality, faster");
        println!("   • faster (13)  - Lowest quality, fastest\n");

        let choices = vec!["medium", "slow", "fast", "faster"];

        Select::new("CPU preset:", choices)
            .with_help_message("Recommendation: medium for CPU encoding")
            .prompt()
            .map(|s| match s {
                "slow" => crate::config::CpuPreset::Slow,
                "fast" => crate::config::CpuPreset::Fast,
                "faster" => crate::config::CpuPreset::Faster,
                _ => crate::config::CpuPreset::Medium,
            })
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))
    }

    fn prompt_preferred_vendor() -> Result<Option<String>> {
        println!("\n🎯 Preferred GPU Vendor (Optional)");
        println!("   Force a specific GPU if multiple are available.\n");
        println!("   Leave blank to auto-detect (recommended).\n");

        let set_vendor = Confirm::new("Set preferred vendor?")
            .with_default(false)
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        if !set_vendor {
            return Ok(None);
        }

        let choices = vec!["nvidia", "intel", "amd", "apple"];

        Select::new("Vendor:", choices)
            .prompt()
            .map(|s| Some(s.to_string()))
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))
    }

    fn prompt_directories() -> Result<Vec<String>> {
        println!("📁 Auto-Scan Directories");
        println!("   Directories to automatically scan for media files.\n");
        println!("   Used in server mode for automatic discovery.");
        println!("   In CLI mode, you specify directories at runtime.\n");

        let add_dirs = Confirm::new("Add auto-scan directories?")
            .with_default(false)
            .with_help_message("Optional: can configure later")
            .prompt()
            .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

        if !add_dirs {
            return Ok(vec![]);
        }

        let mut directories = Vec::new();
        println!("\nEnter directories one at a time. Leave empty to finish.\n");

        loop {
            let dir = Text::new(&format!("Directory {}:", directories.len() + 1))
                .with_default("")
                .prompt()
                .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

            if dir.is_empty() {
                break;
            }

            let path = Path::new(&dir);
            if !path.exists() {
                println!("⚠️  Warning: {} does not exist", dir);
                let add_anyway = Confirm::new("Add anyway?")
                    .with_default(true)
                    .prompt()
                    .map_err(|e| AlchemistError::Config(format!("Prompt failed: {}", e)))?;

                if !add_anyway {
                    continue;
                }
            }

            directories.push(dir);
            println!("✓ Added");
        }

        Ok(directories)
    }

    fn show_summary(config: &Config) {
        println!("\n┌─────────────────────────────────────────┐");
        println!("│ 📋 Configuration Summary                │");
        println!("└─────────────────────────────────────────┘\n");

        println!("Transcoding:");
        println!(
            "  Size Reduction: {:.0}%",
            config.transcode.size_reduction_threshold * 100.0
        );
        println!("  Min BPP: {:.2}", config.transcode.min_bpp_threshold);
        println!("  Min File Size: {} MB", config.transcode.min_file_size_mb);
        println!("  Concurrent Jobs: {}\n", config.transcode.concurrent_jobs);

        println!("Hardware:");
        println!(
            "  CPU Fallback: {}",
            if config.hardware.allow_cpu_fallback {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!(
            "  CPU Encoding: {}",
            if config.hardware.allow_cpu_encoding {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!("  CPU Preset: {}", config.hardware.cpu_preset);
        if let Some(ref vendor) = config.hardware.preferred_vendor {
            println!("  Preferred Vendor: {}\n", vendor);
        } else {
            println!("  Preferred Vendor: Auto-detect\n");
        }

        println!("Scanner:");
        if config.scanner.directories.is_empty() {
            println!("  (No auto-scan directories)\n");
        } else {
            for d in &config.scanner.directories {
                println!("  📁 {}", d);
            }
            println!();
        }
    }

    fn write_config(path: &Path, config: &Config) -> Result<()> {
        let toml_content = toml::to_string_pretty(config)
            .map_err(|e| AlchemistError::Config(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, toml_content)
            .map_err(|e| AlchemistError::Config(format!("Failed to write config: {}", e)))
    }
}
