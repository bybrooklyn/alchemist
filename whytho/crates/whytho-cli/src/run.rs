use std::path::Path;
use std::process::ExitCode;

use whytho_core::config::WhyThoConfig;
use whytho_core::media::MediaInput;
use whytho_core::presets::Preset;
use whytho_core::transcode::TranscodeJob;

use super::args::CliArgs;
use super::executor;

pub fn run(args: &CliArgs) -> ExitCode {
    let input_path = match args.positional.first() {
        Some(p) => p,
        None => {
            eprintln!("error: whytho run requires an input file");
            eprintln!("usage: whytho run <INPUT> [OPTIONS]");
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

    super::plan::apply_cli_overrides(&mut config, args);

    let probe = match whytho_core::probe::probe(Path::new(input_path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let output_path = args
        .flag("output")
        .or_else(|| args.flag("o"))
        .map(Path::new)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let input = Path::new(input_path);
            let stem = input.file_stem().unwrap_or_default();
            let ext = input.extension().unwrap_or_default();
            let parent = input.parent().unwrap_or(Path::new("."));
            parent.join(format!(
                "{}.whytho.{}",
                stem.to_string_lossy(),
                ext.to_string_lossy()
            ))
        });

    let job = TranscodeJob {
        id: 1,
        input: MediaInput::new(input_path),
        output: output_path,
        config,
        probe,
        priority: whytho_core::scheduler::JobPriority::Normal,
    };

    eprintln!(
        "transcoding {} -> {}",
        job.input.path().display(),
        job.output.display()
    );

    match executor::execute(&job) {
        Ok(report) => {
            eprintln!(
                "done: {} frames in {:.1}s ({:.1} fps), {} bytes written",
                report.frames_encoded,
                report.elapsed.as_secs_f64(),
                report.avg_fps,
                report.output_size,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
