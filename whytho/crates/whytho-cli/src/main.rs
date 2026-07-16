#![forbid(unsafe_code)]

mod args;
mod bench;
mod doctor;
mod executor;
mod plan;
mod probe;
mod run;
mod setup;
mod verify;

use std::process::ExitCode;

use args::CliArgs;
use whytho_core::presets::Preset;

fn main() -> ExitCode {
    let args = match CliArgs::parse() {
        Ok(a) => a,
        Err(_) => {
            print_help();
            return ExitCode::from(2);
        }
    };

    match args.command.as_str() {
        "-h" | "--help" => {
            print_help();
            ExitCode::SUCCESS
        }
        "plan" => plan::run(&args),
        "run" => run::run(&args),
        "probe" => probe::run(&args),
        "verify" => verify::run(&args),
        "bench" => bench::run(&args),
        "doctor" => doctor::run(&args),
        "setup" => setup::run(&args),
        cmd => {
            eprintln!("unknown command: {cmd}");
            eprintln!("run `whytho --help` for available commands");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!("whytho.");
    println!();
    println!("Usage: whytho <COMMAND> [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  plan      Show what would happen before doing it");
    println!("  run       Execute a transcode job");
    println!("  probe     Inspect media streams and metadata");
    println!("  verify    Verify encoded output quality");
    println!("  bench     Benchmark encode/decode performance");
    println!("  doctor    Diagnose system and codec support");
    println!("  setup     Guided configuration");
    println!();
    println!("Plan options:");
    println!("  <INPUT>                     Input file");
    println!("  -p, --preset <PRESET>       Override preset");
    println!("  -o, --output <PATH>         Output file path");
    println!("  --container <FORMAT>        mkv, mp4");
    println!("  --video-codec <CODEC>       h264, hevc, av1, av2");
    println!("  --audio-codec <CODEC>       aac, opus, passthrough");
    println!("  --replace-original          Replace input file");
    println!("  --keep-original             Keep input file");
    println!("  --config <PATH>             Config file path");
    println!("  --backend-policy <POLICY>   prefer-hardware, require-hardware, cpu-only");
    println!("  --max-jobs <N>              Max simultaneous jobs");
    println!("  --verification <MODE>       sample, strict, benchmark, military");
    println!();
    println!("Probe options:");
    println!("  <INPUT>                     Input file");
    println!("  --json                      Output as JSON");
    println!();
    println!("Verify options:");
    println!("  <ORIGINAL>                  Original file");
    println!("  <ENCODED>                   Encoded file to verify");
    println!();
    println!("Bench options:");
    println!("  <INPUT>                     Input file to benchmark against");
    println!();
    println!("Initial presets:");
    for preset in Preset::ALL {
        println!("  {}", preset.as_str());
    }
}
