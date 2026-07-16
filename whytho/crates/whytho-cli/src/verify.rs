use std::fs::File;
use std::path::Path;
use std::process::ExitCode;

use matroska_demuxer::{Frame as MkvFrame, MatroskaFile, TrackType};
use whytho_codecs::{H264Decoder, VideoDecoder};
use whytho_core::probe;

use super::args::CliArgs;

pub fn run(args: &CliArgs) -> ExitCode {
    let original_path = match args.positional.first() {
        Some(p) => p,
        None => {
            eprintln!("error: whytho verify requires two input files");
            eprintln!("usage: whytho verify <ORIGINAL> <ENCODED>");
            return ExitCode::from(2);
        }
    };
    let encoded_path = match args.positional.get(1) {
        Some(p) => p,
        None => {
            eprintln!("error: whytho verify requires two input files");
            eprintln!("usage: whytho verify <ORIGINAL> <ENCODED>");
            return ExitCode::from(2);
        }
    };

    let original = Path::new(original_path);
    let encoded = Path::new(encoded_path);

    if !original.exists() {
        eprintln!("error: original file not found: {original_path}");
        return ExitCode::from(2);
    }
    if !encoded.exists() {
        eprintln!("error: encoded file not found: {encoded_path}");
        return ExitCode::from(2);
    }

    let orig_probe = match probe::probe(original) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error probing original: {e}");
            return ExitCode::from(2);
        }
    };
    let enc_probe = match probe::probe(encoded) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error probing encoded: {e}");
            return ExitCode::from(2);
        }
    };

    let orig_video = orig_probe
        .streams
        .iter()
        .find(|s| matches!(s.kind, probe::StreamKind::Video(_)));
    let enc_video = enc_probe
        .streams
        .iter()
        .find(|s| matches!(s.kind, probe::StreamKind::Video(_)));

    let (orig_stream, orig_info) = match orig_video {
        Some(s) => (
            s,
            match &s.kind {
                probe::StreamKind::Video(v) => v,
                _ => unreachable!(),
            },
        ),
        None => {
            eprintln!("error: original file has no video stream");
            return ExitCode::from(2);
        }
    };
    let (enc_stream, enc_info) = match enc_video {
        Some(s) => (
            s,
            match &s.kind {
                probe::StreamKind::Video(v) => v,
                _ => unreachable!(),
            },
        ),
        None => {
            eprintln!("error: encoded file has no video stream");
            return ExitCode::from(2);
        }
    };

    println!("Original: {original_path}");
    println!(
        "  {}x{}, {:.2} fps, codec: {}",
        orig_info.width,
        orig_info.height,
        orig_info.frame_rate.unwrap_or(0.0),
        orig_stream.codec
    );
    println!("Encoded:  {encoded_path}");
    println!(
        "  {}x{}, {:.2} fps, codec: {}",
        enc_info.width,
        enc_info.height,
        enc_info.frame_rate.unwrap_or(0.0),
        enc_stream.codec
    );

    // Decode both files
    let orig_frames = match decode_mkv(original) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error decoding original: {e}");
            return ExitCode::from(2);
        }
    };
    let enc_frames = match decode_mkv(encoded) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error decoding encoded: {e}");
            return ExitCode::from(2);
        }
    };

    println!(
        "\nDecoded: {} original frames, {} encoded frames",
        orig_frames.len(),
        enc_frames.len()
    );

    if orig_frames.is_empty() || enc_frames.is_empty() {
        eprintln!("error: no frames decoded from one or both files");
        return ExitCode::from(2);
    }

    // Compare frames
    let compare_count = orig_frames.len().min(enc_frames.len());
    let mut total_mae = 0.0f64;
    let mut total_psnr = 0.0f64;
    let mut min_psnr = f64::MAX;
    let mut max_psnr = 0.0f64;

    for i in 0..compare_count {
        let orig_y = &orig_frames[i].y;
        let enc_y = &enc_frames[i].y;

        // Handle size mismatch by comparing the overlapping region
        let w = orig_frames[i].width.min(enc_frames[i].width) as usize;
        let h = orig_frames[i].height.min(enc_frames[i].height) as usize;
        let orig_stride = orig_frames[i].width as usize;
        let enc_stride = enc_frames[i].width as usize;

        let mut sum_err = 0u64;
        let mut sum_sq_err = 0f64;
        let mut count = 0usize;

        for row in 0..h {
            for col in 0..w {
                let o = orig_y[row * orig_stride + col] as i32;
                let e = enc_y[row * enc_stride + col] as i32;
                let diff = (o - e).unsigned_abs() as u64;
                sum_err += diff;
                sum_sq_err += (diff as f64).powi(2);
                count += 1;
            }
        }

        let mae = sum_err as f64 / count as f64;
        let mse = sum_sq_err / count as f64;
        let psnr = if mse > 0.0 {
            10.0 * (255.0f64.powi(2) / mse).log10()
        } else {
            f64::INFINITY
        };

        total_mae += mae;
        total_psnr += psnr;
        min_psnr = min_psnr.min(psnr);
        max_psnr = max_psnr.max(psnr);
    }

    let avg_mae = total_mae / compare_count as f64;
    let avg_psnr = total_psnr / compare_count as f64;

    println!("\nQuality ({compare_count} frames compared):");
    println!("  MAE:   {avg_mae:.2}");
    println!("  PSNR:  {avg_psnr:.2} dB (min: {min_psnr:.2}, max: {max_psnr:.2})");

    let orig_size = std::fs::metadata(original).map(|m| m.len()).unwrap_or(0);
    let enc_size = std::fs::metadata(encoded).map(|m| m.len()).unwrap_or(0);
    let ratio = if orig_size > 0 {
        enc_size as f64 / orig_size as f64
    } else {
        0.0
    };
    println!(
        "\nSize: {} -> {} ({:.1}x)",
        human_size(orig_size),
        human_size(enc_size),
        ratio
    );

    if avg_psnr >= 30.0 {
        println!("\nResult: PASS (PSNR >= 30 dB)");
        ExitCode::SUCCESS
    } else {
        println!("\nResult: FAIL (PSNR < 30 dB)");
        ExitCode::from(1)
    }
}

fn decode_mkv(path: &Path) -> Result<Vec<whytho_codecs::DecodedFrame>, String> {
    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut mkv = MatroskaFile::open(file).map_err(|e| format!("mkv parse: {e}"))?;

    let video_track = mkv
        .tracks()
        .iter()
        .find(|t| t.track_type() == TrackType::Video)
        .ok_or("no video track")?
        .clone();

    let codec_id = video_track.codec_id();
    let codec_private = video_track.codec_private().map(|cp| cp.to_vec());
    let fps = 24.0; // default; actual fps comes from probe

    let mut frames = Vec::new();

    if codec_id.contains("AVC") || codec_id.contains("H264") {
        let mut decoder = H264Decoder::with_fps(fps);
        if let Some(ref cp) = codec_private {
            decoder
                .init_avcc(cp)
                .map_err(|e| format!("avcc init: {e}"))?;
        }

        let mut frame = MkvFrame::default();
        while mkv
            .next_frame(&mut frame)
            .map_err(|e| format!("read frame: {e}"))?
        {
            if frame.track == video_track.track_number().get() {
                let decoded = decoder
                    .decode_sample(&frame.data)
                    .map_err(|e| format!("decode: {e}"))?;
                frames.extend(decoded);
            }
        }
        frames.extend(decoder.flush());
    } else {
        return Err(format!(
            "unsupported codec: {codec_id} (only H.264 supported for verify)"
        ));
    }

    Ok(frames)
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
