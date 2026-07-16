use std::path::Path;
use std::process::ExitCode;

use whytho_core::probe;

use super::args::CliArgs;

pub fn run(args: &CliArgs) -> ExitCode {
    let input_path = match args.positional.first() {
        Some(p) => p,
        None => {
            eprintln!("error: whytho probe requires an input file");
            eprintln!("usage: whytho probe <INPUT> [--json]");
            return ExitCode::from(2);
        }
    };

    match probe::probe(Path::new(input_path)) {
        Ok(result) => {
            if args.has_flag("json") {
                match serde_json::to_string_pretty(&result) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: failed to serialize probe result: {e}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                print!("{result}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
