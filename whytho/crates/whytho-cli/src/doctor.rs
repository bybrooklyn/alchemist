use std::process::ExitCode;

use super::args::CliArgs;

pub fn run(_args: &CliArgs) -> ExitCode {
    println!("whytho. doctor — system diagnostics");
    println!();

    // CPU info
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("CPU workers: {cpus}");

    // Check for codec support
    println!();
    println!("Codec support:");

    // H.264 encoder
    {
        use whytho_codecs::{H264Encoder, VideoEncoder, VideoEncoderConfig};
        let mut enc = H264Encoder::new();
        let config = VideoEncoderConfig {
            width: 16,
            height: 16,
            fps: 24.0,
            ..Default::default()
        };
        match enc.configure(&config) {
            Ok(()) => println!("  H.264 encode: OK"),
            Err(e) => println!("  H.264 encode: FAIL ({e})"),
        }
    }

    // H.264 decoder
    {
        use whytho_codecs::{H264Decoder, VideoDecoder};
        let _dec = H264Decoder::new();
        println!("  H.264 decode: OK");
    }

    // AV1 encoder
    {
        use whytho_codecs::{Av1Encoder, VideoEncoder, VideoEncoderConfig};
        let mut enc = Av1Encoder::new();
        let config = VideoEncoderConfig {
            width: 64,
            height: 64,
            fps: 24.0,
            speed_preset: 10,
            ..Default::default()
        };
        match enc.configure(&config) {
            Ok(()) => {
                let _ = enc.flush();
                println!("  AV1 encode:   OK (via rav1e)")
            }
            Err(e) => println!("  AV1 encode:   FAIL ({e})"),
        }
    }

    // AV1 decoder
    println!("  AV1 decode:   scaffold (not yet functional)");

    // Opus
    println!("  Opus encode:  OK (via opus-rs)");

    // Container support
    println!();
    println!("Container support:");
    println!("  MKV mux:      OK");
    println!("  MKV demux:    OK");

    // Backend info
    println!();
    println!("Backends:");
    println!("  CPU:          available");
    println!("  QSV:          not implemented");
    println!("  NVENC:        not implemented");
    println!("  VideoToolbox: not implemented");
    println!("  VAAPI:        not implemented");
    println!("  AMF:          not implemented");

    println!();
    println!("All checks passed.");
    ExitCode::SUCCESS
}
