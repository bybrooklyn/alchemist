use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use whytho_codecs::{
    DecodedFrame, H264Decoder, H264Encoder, PixelFormat, VideoDecoder, VideoEncoder,
    VideoEncoderConfig,
};

use super::args::CliArgs;

pub fn run(args: &CliArgs) -> ExitCode {
    let input_path = match args.positional.first() {
        Some(p) => p,
        None => {
            eprintln!("error: whytho bench requires an input file");
            eprintln!("usage: whytho bench <INPUT>");
            return ExitCode::from(2);
        }
    };

    let path = Path::new(input_path);
    if !path.exists() {
        eprintln!("error: file not found: {input_path}");
        return ExitCode::from(2);
    }

    let probe = match whytho_core::probe::probe(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error probing: {e}");
            return ExitCode::from(2);
        }
    };

    let video_stream = probe
        .streams
        .iter()
        .find(|s| matches!(s.kind, whytho_core::probe::StreamKind::Video(_)));

    let (width, height, fps) = match video_stream.map(|s| &s.kind) {
        Some(whytho_core::probe::StreamKind::Video(v)) => {
            (v.width, v.height, v.frame_rate.unwrap_or(24.0))
        }
        _ => {
            eprintln!("error: no video stream found");
            return ExitCode::from(2);
        }
    };

    println!("Benchmark: {input_path}");
    println!("  {width}x{height} @ {fps:.2} fps");
    println!();

    // Generate a synthetic frame for benchmarking
    let frame = make_gradient_frame(width, height);

    // Benchmark H.264 encode at different QPs
    println!("H.264 encode:");
    for qp in [18, 22, 26, 32] {
        let bitrate = match qp {
            18 => 8_000_000,
            22 => 4_000_000,
            26 => 2_000_000,
            32 => 1_000_000,
            _ => 2_000_000,
        };
        let config = VideoEncoderConfig {
            width,
            height,
            fps,
            bitrate,
            keyframe_interval: 240,
            ..Default::default()
        };

        let start = Instant::now();
        let frames_to_encode = 10;
        let mut total_bytes = 0usize;

        let mut enc = H264Encoder::new();
        enc.configure(&config).unwrap();

        for _ in 0..frames_to_encode {
            let packets = enc.encode(&frame).unwrap();
            for pkt in &packets {
                total_bytes += pkt.data.len();
            }
        }
        let _ = enc.flush();

        let elapsed = start.elapsed();
        let fps_actual = frames_to_encode as f64 / elapsed.as_secs_f64();
        let avg_bytes = total_bytes as f64 / frames_to_encode as f64;

        println!(
            "  QP {:2}: {:6.1} fps, avg {:.0} bytes/frame ({:.1} KB)",
            qp,
            fps_actual,
            avg_bytes,
            avg_bytes / 1024.0
        );
    }

    // Benchmark H.264 decode
    println!("\nH.264 decode:");
    let config = VideoEncoderConfig {
        width,
        height,
        fps,
        bitrate: 2_000_000,
        keyframe_interval: 1,
        ..Default::default()
    };
    let mut enc = H264Encoder::new();
    enc.configure(&config).unwrap();
    let packets = enc.encode(&frame).unwrap();

    let decode_iterations = 10;
    let start = Instant::now();
    for _ in 0..decode_iterations {
        let mut dec = H264Decoder::new();
        for pkt in &packets {
            let _ = dec.decode_nal(&pkt.data);
        }
        let _ = dec.flush();
    }
    let elapsed = start.elapsed();
    let decode_fps = decode_iterations as f64 / elapsed.as_secs_f64();
    println!("  {decode_fps:.1} fps (avg of {decode_iterations} iterations)");

    ExitCode::SUCCESS
}

fn make_gradient_frame(width: u32, height: u32) -> DecodedFrame {
    let mut y = vec![0u8; (width * height) as usize];
    for row in 0..height {
        for col in 0..width {
            y[(row * width + col) as usize] = ((row * 4 + col * 2) % 256) as u8;
        }
    }
    let uv = ((width / 2) * (height / 2)) as usize;
    DecodedFrame {
        width,
        height,
        y,
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    }
}
