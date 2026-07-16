use std::path::Path;
use std::process::ExitCode;

use whytho_core::config::WhyThoConfig;
use whytho_core::file_ops::FileOperationMode;
use whytho_core::media::MediaInput;
use whytho_core::pipeline::{self, PlanRequest};
use whytho_core::presets::Preset;

use super::args::CliArgs;

pub fn run(args: &CliArgs) -> ExitCode {
    let input_path = match args.positional.first() {
        Some(p) => p,
        None => {
            eprintln!("error: whytho plan requires an input file");
            eprintln!("usage: whytho plan <INPUT> [OPTIONS]");
            return ExitCode::from(2);
        }
    };

    let preset_override = args
        .flag("preset")
        .or_else(|| args.flag("p"))
        .and_then(|s| s.parse::<Preset>().ok());

    let config_path = args.flag("config").map(Path::new);
    let mut config = match WhyThoConfig::load(config_path, preset_override) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    apply_cli_overrides(&mut config, args);

    let output_path = args
        .flag("output")
        .or_else(|| args.flag("o"))
        .map(Path::new)
        .map(|p| p.to_path_buf());

    let request = PlanRequest {
        input: MediaInput::new(input_path),
        config,
        output_path,
    };

    match pipeline::plan(request) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

pub fn apply_cli_overrides(config: &mut WhyThoConfig, args: &CliArgs) {
    if let Some(c) = args.flag("container") {
        if let Ok(c) = c.parse() {
            config.container = c;
        }
    }
    if let Some(v) = args.flag("video-codec") {
        if let Ok(v) = v.parse() {
            config.default_video_codec = v;
        }
    }
    if let Some(a) = args.flag("audio-codec") {
        if let Ok(a) = a.parse() {
            config.default_audio_codec = a;
        }
    }
    if let Some(v) = args.flag("verification") {
        if let Ok(v) = v.parse() {
            config.verification = v;
        }
    }
    if args.has_flag("replace-original") {
        config.file_operation = FileOperationMode::ReplaceOriginal;
    }
    if args.has_flag("keep-original") {
        config.file_operation = FileOperationMode::KeepOriginal;
    }
    if let Some(m) = args.flag("max-jobs") {
        if let Ok(m) = m.parse() {
            config.max_jobs = m;
        }
    }
    if let Some(b) = args.flag("backend-policy") {
        if let Ok(b) = b.parse() {
            config.backend_policy = b;
        }
    }
}
