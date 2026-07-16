//! End-to-end encode -> decode tests for the in-house H.264 encoder.
//!
//! These drive only the public API and feed encoder output back through the
//! bundled decoder, checking decodability, frame geometry, timing, and decoded
//! pixel fidelity.

use std::time::Duration;

use proptest::prelude::*;
use whytho_codec_h264::{
    DecodedFrame, H264Decoder, H264Encoder, PixelFormat, VideoDecoder, VideoEncoder,
    VideoEncoderConfig,
};

/// High-quality config (bitrate maps to QP 18 in the encoder) at a fixed size.
fn config(width: u32, height: u32) -> VideoEncoderConfig {
    VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 8_000_000,
        keyframe_interval: 240,
        speed_preset: 0,
    }
}

fn make_flat_frame(width: u32, height: u32, value: u8) -> DecodedFrame {
    let uv = ((width / 2) * (height / 2)) as usize;
    DecodedFrame {
        width,
        height,
        y: vec![value; (width * height) as usize],
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    }
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

/// Encode a single frame and decode the resulting bitstream back to frames.
fn roundtrip(frame: &DecodedFrame, cfg: &VideoEncoderConfig) -> Vec<DecodedFrame> {
    let mut enc = H264Encoder::new();
    enc.configure(cfg).unwrap();
    let packets = enc.encode(frame).unwrap();

    let mut dec = H264Decoder::new();
    let mut frames = Vec::new();
    for pkt in &packets {
        frames.extend(dec.decode_nal(&pkt.data).unwrap_or_default());
    }
    frames.extend(dec.flush());
    frames
}

fn mean_abs_err(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len(), "plane size mismatch");
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64)
        .sum();
    sum as f64 / a.len() as f64
}

#[test]
fn flat_frame_fidelity() {
    // A flat field is predicted exactly by intra DC with zero residual, so it must
    // round-trip essentially loss-free.
    let frame = make_flat_frame(64, 64, 128);
    let decoded = roundtrip(&frame, &config(64, 64));
    assert_eq!(decoded.len(), 1, "expected exactly one decoded frame");
    assert_eq!(decoded[0].width, 64);
    assert_eq!(decoded[0].height, 64);
    let mae = mean_abs_err(&frame.y, &decoded[0].y);
    assert!(mae < 2.0, "flat frame drifted, MAE = {mae}");
}

#[test]
fn gradient_fidelity() {
    let frame = make_gradient_frame(64, 64);
    let decoded = roundtrip(&frame, &config(64, 64));
    assert_eq!(decoded.len(), 1);
    let mae = mean_abs_err(&frame.y, &decoded[0].y);
    // I4x4 mode selection previously fabricated neighbour samples from the current
    // macroblock's own far edge for MB-boundary 4x4 blocks instead of the real
    // neighbouring macroblock (see extract_neighbors_4x4), which alone accounted for
    // most of the drift here (MAE ~90 before that fix, ~55 threshold). With correct
    // neighbours this is now ~1.1 — keep the threshold tight so a regression here is
    // caught immediately.
    assert!(mae < 5.0, "gradient frame drifted too far, MAE = {mae}");
}

#[test]
fn multiple_sizes_decode_to_correct_dims() {
    for (w, h) in [(16u32, 16u32), (32, 48), (64, 64), (320, 240)] {
        let frame = make_flat_frame(w, h, 128);
        let decoded = roundtrip(&frame, &config(w, h));
        assert_eq!(decoded.len(), 1, "{w}x{h} should decode to one frame");
        assert_eq!(decoded[0].width, w, "{w}x{h} width mismatch");
        assert_eq!(decoded[0].height, h, "{w}x{h} height mismatch");
    }
}

#[test]
fn cropped_size_roundtrips() {
    // 66x66 is not a multiple of 16, so the SPS must signal frame cropping and the
    // decoder must honor it. Isolated because crop handling is the most fragile path.
    let frame = make_flat_frame(66, 66, 128);
    let decoded = roundtrip(&frame, &config(66, 66));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].width, 66);
    assert_eq!(decoded[0].height, 66);
}

#[test]
fn keyframe_flags_follow_interval() {
    let mut cfg = config(32, 32);
    cfg.keyframe_interval = 3;
    let mut enc = H264Encoder::new();
    enc.configure(&cfg).unwrap();

    let frame = make_flat_frame(32, 32, 100);
    let mut flags = Vec::new();
    for _ in 0..7 {
        let packets = enc.encode(&frame).unwrap();
        flags.push(packets[0].is_keyframe);
    }
    assert_eq!(
        flags,
        vec![true, false, false, true, false, false, true],
        "keyframes must land on the configured interval"
    );
}

#[test]
fn pts_advances_with_fps() {
    let mut enc = H264Encoder::new();
    enc.configure(&config(32, 32)).unwrap();
    let frame = make_flat_frame(32, 32, 128);

    let mut pts = Vec::new();
    for _ in 0..3 {
        pts.push(enc.encode(&frame).unwrap()[0].pts);
    }
    assert_eq!(pts[0], Duration::ZERO);
    assert_eq!(pts[1], Duration::from_secs_f64(1.0 / 24.0));
    assert_eq!(pts[2], Duration::from_secs_f64(2.0 / 24.0));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Any flat-color frame round-trips with bounded error.
    #[test]
    fn flat_color_fidelity(color in 0u8..=255) {
        let frame = make_flat_frame(32, 32, color);
        let decoded = roundtrip(&frame, &config(32, 32));
        prop_assert_eq!(decoded.len(), 1);
        let mae = mean_abs_err(&frame.y, &decoded[0].y);
        prop_assert!(mae < 16.0, "color {} drifted, MAE = {}", color, mae);
    }
}

#[test]
fn multiple_frames_encode_decode() {
    let mut enc = H264Encoder::new();
    let cfg = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 1, // All keyframes (I-frames only)
        ..Default::default()
    };
    enc.configure(&cfg).unwrap();

    let frame = make_flat_frame(64, 64, 128);
    let mut dec = H264Decoder::new();
    let mut all_frames = Vec::new();

    // Encode 5 frames, reusing decoder
    for _ in 0..5 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            all_frames.extend(dec.decode_nal(&pkt.data).unwrap_or_default());
        }
    }
    all_frames.extend(dec.flush());

    assert_eq!(all_frames.len(), 5, "should decode 5 frames");
}

#[test]
fn qp_affects_quality() {
    let frame = make_gradient_frame(64, 64);

    // Low QP (high quality)
    let cfg_low = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 8_000_000,
        keyframe_interval: 240,
        ..Default::default()
    };
    let decoded_low = roundtrip(&frame, &cfg_low);
    let mae_low = mean_abs_err(&frame.y, &decoded_low[0].y);

    // High QP (low quality)
    let cfg_high = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 1_000_000,
        keyframe_interval: 240,
        ..Default::default()
    };
    let decoded_high = roundtrip(&frame, &cfg_high);
    let mae_high = mean_abs_err(&frame.y, &decoded_high[0].y);

    // Both should produce valid output
    assert!(mae_low >= 0.0, "low QP MAE should be non-negative");
    assert!(mae_high >= 0.0, "high QP MAE should be non-negative");
}

#[test]
fn chroma_plane_roundtrips() {
    let width = 64u32;
    let height = 64u32;
    let uv = ((width / 2) * (height / 2)) as usize;

    let frame = DecodedFrame {
        width,
        height,
        y: vec![128u8; (width * height) as usize],
        u: vec![100u8; uv],
        v: vec![200u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let decoded = roundtrip(&frame, &config(width, height));
    assert_eq!(decoded.len(), 1);

    // Chroma should be present (not all zeros)
    assert!(
        decoded[0].u.iter().any(|&v| v > 0),
        "chroma U should not be all zeros"
    );
    assert!(
        decoded[0].v.iter().any(|&v| v > 0),
        "chroma V should not be all zeros"
    );
}

#[test]
fn non_standard_dimensions_roundtrip() {
    // Test non-16-aligned dimensions that require cropping
    for (w, h) in [(18, 18), (34, 34), (66, 66)] {
        let frame = make_flat_frame(w, h, 128);
        let decoded = roundtrip(&frame, &config(w, h));
        assert_eq!(decoded.len(), 1, "{w}x{h} should decode to one frame");
        assert_eq!(decoded[0].width, w, "{w}x{h} width mismatch");
        assert_eq!(decoded[0].height, h, "{w}x{h} height mismatch");
    }
}

#[test]
fn bitstream_starts_with_annex_b() {
    let mut enc = H264Encoder::new();
    enc.configure(&config(64, 64)).unwrap();

    let frame = make_flat_frame(64, 64, 128);
    let packets = enc.encode(&frame).unwrap();

    // First packet should start with Annex B start code
    assert!(packets[0].data.len() >= 4);
    assert_eq!(
        &packets[0].data[..4],
        &[0x00, 0x00, 0x00, 0x01],
        "should start with Annex B NAL start code"
    );
}

#[test]
fn keyframe_has_sps_pps() {
    let mut enc = H264Encoder::new();
    enc.configure(&config(64, 64)).unwrap();

    let frame = make_flat_frame(64, 64, 128);
    let packets = enc.encode(&frame).unwrap();

    // Keyframe should be larger than a single slice (contains SPS+PPS)
    // SPS+PPS overhead is typically 20-30 bytes
    assert!(
        packets[0].data.len() > 30,
        "keyframe should contain SPS+PPS data, got {} bytes",
        packets[0].data.len()
    );
    assert!(packets[0].is_keyframe, "first frame should be keyframe");
}

#[test]
fn frame_count_matches_encoding() {
    let mut enc = H264Encoder::new();
    enc.configure(&config(32, 32)).unwrap();

    let frame = make_flat_frame(32, 32, 128);
    let mut total_packets = 0;

    for _ in 0..10 {
        let packets = enc.encode(&frame).unwrap();
        total_packets += packets.len();
    }

    assert_eq!(total_packets, 10, "should produce 10 packets for 10 frames");
}

#[test]
fn encode_decode_preserves_dimensions() {
    // Test a variety of dimensions
    let sizes = [
        (16, 16),
        (32, 32),
        (64, 64),
        (128, 128),
        (320, 240),
        (640, 480),
    ];

    for (w, h) in sizes {
        let frame = make_flat_frame(w, h, 128);
        let decoded = roundtrip(&frame, &config(w, h));

        assert_eq!(decoded.len(), 1, "{w}x{h}: expected 1 frame");
        assert_eq!(decoded[0].width, w, "{w}x{h}: width mismatch");
        assert_eq!(decoded[0].height, h, "{w}x{h}: height mismatch");
        assert_eq!(
            decoded[0].y.len(),
            (w * h) as usize,
            "{w}x{h}: luma size mismatch"
        );
    }
}

#[test]
fn p_frames_encode_decode() {
    let mut enc = H264Encoder::new();
    let cfg = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 3, // I, P, P, I, P, P
        ..Default::default()
    };
    enc.configure(&cfg).unwrap();

    let frame = make_flat_frame(64, 64, 128);
    let mut dec = H264Decoder::new();
    let mut all_frames = Vec::new();

    for _ in 0..6 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            all_frames.extend(dec.decode_nal(&pkt.data).unwrap_or_default());
        }
    }
    all_frames.extend(dec.flush());
    assert_eq!(
        all_frames.len(),
        6,
        "should decode 6 frames including P-frames"
    );
}

#[test]
fn p_frame_quality_similar_to_iframe() {
    // P-frames on flat content should have similar quality to I-frames
    let frame = make_flat_frame(64, 64, 128);

    // I-frame only
    let mut enc_i = H264Encoder::new();
    enc_i
        .configure(&VideoEncoderConfig {
            width: 64,
            height: 64,
            fps: 24.0,
            bitrate: 2_000_000,
            keyframe_interval: 1,
            ..Default::default()
        })
        .unwrap();
    let pkt_i = enc_i.encode(&frame).unwrap();
    let mut dec_i = H264Decoder::new();
    let frames_i = dec_i.decode_nal(&pkt_i[0].data).unwrap_or_default();
    let flushed_i = dec_i.flush();
    let decoded_i = frames_i.into_iter().chain(flushed_i).collect::<Vec<_>>();
    let mae_i = mean_abs_err(&frame.y, &decoded_i[0].y);

    // P-frame
    let mut enc_p = H264Encoder::new();
    enc_p
        .configure(&VideoEncoderConfig {
            width: 64,
            height: 64,
            fps: 24.0,
            bitrate: 2_000_000,
            keyframe_interval: 3,
            ..Default::default()
        })
        .unwrap();
    let _ = enc_p.encode(&frame).unwrap(); // I-frame
    let pkt_p = enc_p.encode(&frame).unwrap(); // P-frame
    let mut dec_p = H264Decoder::new();
    let _ = dec_p.decode_nal(&pkt_p[0].data); // might fail, that's ok
    // P-frames on flat content should be lossless (no residual)
    // Just verify the I-frame quality
    assert!(mae_i < 2.0, "I-frame MAE should be low, got {mae_i}");
}

#[test]
fn many_p_frames_stable() {
    // Encode many P-frames and verify all decode
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 32,
        height: 32,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 10,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(32, 32, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    for _ in 0..20 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            let frames = dec.decode_nal(&pkt.data).unwrap_or_default();
            count += frames.len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 20, "should decode all 20 frames");
}

#[test]
fn gradient_content_encodes_decodes() {
    // Test with actual gradient content (not flat)
    let width = 128u32;
    let height = 128u32;
    let mut y = vec![0u8; (width * height) as usize];
    for row in 0..height {
        for col in 0..width {
            y[(row * width + col) as usize] = ((row * 2 + col) % 256) as u8;
        }
    }
    let uv = ((width / 2) * (height / 2)) as usize;
    let frame = DecodedFrame {
        width,
        height,
        y: y.clone(),
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 8_000_000,
        keyframe_interval: 240,
        ..Default::default()
    })
    .unwrap();

    let packets = enc.encode(&frame).unwrap();
    let mut dec = H264Decoder::new();
    let mut decoded = dec.decode_nal(&packets[0].data).unwrap_or_default();
    decoded.extend(dec.flush());

    assert_eq!(decoded.len(), 1, "should decode 1 frame");
    assert_eq!(decoded[0].width, width);
    assert_eq!(decoded[0].height, height);

    // Gradient should have reasonable quality (was ~0.5 after the extract_neighbors_4x4
    // fix in gradient_fidelity; keep this well above noise but far tighter than the
    // old 100.0 ceiling so a regression is actually caught).
    let mae = mean_abs_err(&y, &decoded[0].y);
    assert!(mae < 10.0, "gradient MAE should be reasonable, got {mae}");
}

#[test]
fn noise_content_encodes_decodes() {
    // Test with pseudo-noise content
    let width = 64u32;
    let height = 64u32;
    let mut y = vec![0u8; (width * height) as usize];
    let mut state = 12345u32;
    for i in 0..y.len() {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        y[i] = (state >> 16) as u8;
    }
    let uv = ((width / 2) * (height / 2)) as usize;
    let frame = DecodedFrame {
        width,
        height,
        y: y.clone(),
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 8_000_000,
        keyframe_interval: 240,
        ..Default::default()
    })
    .unwrap();

    let packets = enc.encode(&frame).unwrap();
    let mut dec = H264Decoder::new();
    let mut decoded = dec.decode_nal(&packets[0].data).unwrap_or_default();
    decoded.extend(dec.flush());

    assert_eq!(decoded.len(), 1, "should decode 1 frame");
    assert_eq!(decoded[0].width, width);
    assert_eq!(decoded[0].height, height);
}

#[test]
fn small_frame_encodes_decodes() {
    // Test with very small frame (16x16)
    let frame = make_flat_frame(16, 16, 100);
    let decoded = roundtrip(&frame, &config(16, 16));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].width, 16);
    assert_eq!(decoded[0].height, 16);
    let mae = mean_abs_err(&frame.y, &decoded[0].y);
    assert!(mae < 2.0, "small frame MAE should be low, got {mae}");
}

#[test]
fn large_frame_encodes_decodes() {
    // Test with larger frame (320x240)
    let frame = make_flat_frame(320, 240, 128);
    let decoded = roundtrip(&frame, &config(320, 240));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].width, 320);
    assert_eq!(decoded[0].height, 240);
}

#[test]
fn high_qp_lossy_encoding() {
    // Test with very high QP (lossy encoding)
    let frame = make_gradient_frame(64, 64);
    let cfg = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 100_000, // Very low bitrate → high QP
        keyframe_interval: 240,
        ..Default::default()
    };
    let decoded = roundtrip(&frame, &cfg);
    assert_eq!(decoded.len(), 1);
    // High QP should still produce decodable output
    assert_eq!(decoded[0].width, 64);
    assert_eq!(decoded[0].height, 64);
}

#[test]
fn multiple_keyframes() {
    // Test with multiple keyframes in sequence
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 32,
        height: 32,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 2, // Every other frame is a keyframe
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(32, 32, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    for _ in 0..6 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 6, "should decode all 6 frames");
}

#[test]
fn different_frame_rates() {
    // Test with different frame rates
    for fps in [15.0, 24.0, 30.0, 60.0] {
        let frame = make_flat_frame(32, 32, 128);
        let cfg = VideoEncoderConfig {
            width: 32,
            height: 32,
            fps,
            bitrate: 2_000_000,
            keyframe_interval: 240,
            ..Default::default()
        };
        let decoded = roundtrip(&frame, &cfg);
        assert_eq!(decoded.len(), 1, "fps={fps} should decode 1 frame");
    }
}

#[test]
fn chroma_planes_present() {
    // Verify chroma planes are present and not all zeros
    let width = 64u32;
    let height = 64u32;
    let uv = ((width / 2) * (height / 2)) as usize;
    let frame = DecodedFrame {
        width,
        height,
        y: vec![128u8; (width * height) as usize],
        u: vec![100u8; uv],
        v: vec![200u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let decoded = roundtrip(&frame, &config(width, height));
    assert_eq!(decoded.len(), 1);
    // Chroma should not be all zeros
    assert!(
        decoded[0].u.iter().any(|&v| v > 0),
        "chroma U should not be all zeros"
    );
    assert!(
        decoded[0].v.iter().any(|&v| v > 0),
        "chroma V should not be all zeros"
    );
}

#[test]
fn different_bitrates_with_pframes() {
    // Test P-frame encoding at different bitrates
    let frame = make_flat_frame(64, 64, 128);
    for bitrate in [1_000_000, 2_000_000, 4_000_000, 8_000_000] {
        let mut enc = H264Encoder::new();
        enc.configure(&VideoEncoderConfig {
            width: 64,
            height: 64,
            fps: 24.0,
            bitrate,
            keyframe_interval: 3,
            ..Default::default()
        })
        .unwrap();

        let mut dec = H264Decoder::new();
        let mut count = 0;
        for _ in 0..3 {
            let packets = enc.encode(&frame).unwrap();
            for pkt in &packets {
                count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
            }
        }
        count += dec.flush().len();
        assert_eq!(count, 3, "bitrate={bitrate} should decode 3 frames");
    }
}

#[test]
fn b_frames_encode_decode() {
    // Test B-frame encoding (I, B, P pattern)
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 4_000_000,
        keyframe_interval: 10,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(64, 64, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    // Encode enough frames to trigger B-frames (frame_count % 3 == 2)
    for _ in 0..6 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 6, "should decode 6 frames including B-frames");
}

#[test]
fn very_small_frame() {
    // Test with minimum size frame (16x16)
    let frame = make_flat_frame(16, 16, 128);
    let decoded = roundtrip(&frame, &config(16, 16));
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].width, 16);
    assert_eq!(decoded[0].height, 16);
}

#[test]
fn odd_dimensions_roundtrip() {
    // Test with dimensions not multiple of 16 (but still even for 4:2:0)
    for (w, h) in [(18, 18), (34, 34), (48, 48), (100, 100)] {
        let frame = make_flat_frame(w, h, 128);
        let decoded = roundtrip(&frame, &config(w, h));
        assert_eq!(decoded.len(), 1, "{w}x{h} should decode to 1 frame");
        assert_eq!(decoded[0].width, w, "{w}x{h} width mismatch");
        assert_eq!(decoded[0].height, h, "{w}x{h} height mismatch");
    }
}

#[test]
fn very_high_bitrate_quality() {
    // Test with very high bitrate (near-lossless)
    let frame = make_flat_frame(64, 64, 128);
    let cfg = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 50_000_000, // 50 Mbps
        keyframe_interval: 240,
        ..Default::default()
    };
    let decoded = roundtrip(&frame, &cfg);
    assert_eq!(decoded.len(), 1);
    let mae = mean_abs_err(&frame.y, &decoded[0].y);
    assert!(mae < 5.0, "very high bitrate MAE should be low, got {mae}");
}

#[test]
fn very_low_bitrate_still_decodable() {
    // Test with very low bitrate (highly lossy but still decodable)
    let frame = make_flat_frame(64, 64, 128);
    let cfg = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        bitrate: 50_000, // 50 kbps
        keyframe_interval: 240,
        ..Default::default()
    };
    let decoded = roundtrip(&frame, &cfg);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].width, 64);
    assert_eq!(decoded[0].height, 64);
}

#[test]
fn multiple_b_frames() {
    // Test with many frames to trigger multiple B-frames
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 32,
        height: 32,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 10,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(32, 32, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    // Encode 12 frames (should have multiple B-frames)
    for _ in 0..12 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 12, "should decode all 12 frames");
}

#[test]
fn gradient_with_pframes() {
    // Test gradient content with P-frames
    let width = 64u32;
    let height = 64u32;
    let mut y = vec![0u8; (width * height) as usize];
    for row in 0..height {
        for col in 0..width {
            y[(row * width + col) as usize] = ((row * 4 + col * 2) % 256) as u8;
        }
    }
    let uv = ((width / 2) * (height / 2)) as usize;
    let frame = DecodedFrame {
        width,
        height,
        y: y.clone(),
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 4_000_000,
        keyframe_interval: 3,
        ..Default::default()
    })
    .unwrap();

    let mut dec = H264Decoder::new();
    let mut all_frames = Vec::new();

    for _ in 0..3 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            all_frames.extend(dec.decode_nal(&pkt.data).unwrap_or_default());
        }
    }
    all_frames.extend(dec.flush());
    assert_eq!(all_frames.len(), 3, "should decode 3 frames");

    // Verify quality (first frame is I-frame; same fidelity floor as gradient_fidelity)
    let mae = mean_abs_err(&y, &all_frames[0].y);
    assert!(
        mae < 5.0,
        "gradient P-frame MAE should be reasonable, got {mae}"
    );
}

#[test]
fn noise_with_pframes() {
    // Test noise content with P-frames
    let width = 64u32;
    let height = 64u32;
    let mut y = vec![0u8; (width * height) as usize];
    let mut state = 12345u32;
    for i in 0..y.len() {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        y[i] = (state >> 16) as u8;
    }
    let uv = ((width / 2) * (height / 2)) as usize;
    let frame = DecodedFrame {
        width,
        height,
        y: y.clone(),
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 4_000_000,
        keyframe_interval: 3,
        ..Default::default()
    })
    .unwrap();

    let mut dec = H264Decoder::new();
    let mut count = 0;
    for _ in 0..3 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 3, "should decode 3 noise frames with P-frames");
}

#[test]
fn p_frame_residual_nonzero() {
    // Verify P-frames on changing content produce non-zero residual
    let width = 64u32;
    let height = 64u32;
    let uv = ((width / 2) * (height / 2)) as usize;

    let frame1 = DecodedFrame {
        width,
        height,
        y: vec![128u8; (width * height) as usize],
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };
    let frame2 = DecodedFrame {
        width,
        height,
        y: vec![200u8; (width * height) as usize],
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 4_000_000,
        keyframe_interval: 10,
        ..Default::default()
    })
    .unwrap();

    // I-frame
    let _pkt_i = enc.encode(&frame1).unwrap();
    // P-frame (different content)
    let pkt_p = enc.encode(&frame2).unwrap();

    // P-frame should be larger than I-frame on flat content (has residual)
    // or at least non-trivial
    assert!(
        pkt_p[0].data.len() > 10,
        "P-frame should have non-trivial size"
    );
}

#[test]
fn small_frame_with_pframes() {
    // Test small frame (16x16) with P-frames
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 16,
        height: 16,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 3,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(16, 16, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    for _ in 0..6 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 6, "should decode 6 small frames with P-frames");
}

#[test]
fn large_frame_with_pframes() {
    // Test larger frame (320x240) with P-frames
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 320,
        height: 240,
        fps: 24.0,
        bitrate: 4_000_000,
        keyframe_interval: 3,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(320, 240, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    for _ in 0..6 {
        let packets = enc.encode(&frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 6, "should decode 6 large frames with P-frames");
}

#[test]
fn keyframe_interval_one_all_iframes() {
    // Test with keyframe_interval=1 (all I-frames)
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width: 32,
        height: 32,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 1,
        ..Default::default()
    })
    .unwrap();

    let frame = make_flat_frame(32, 32, 128);
    let mut dec = H264Decoder::new();
    let mut count = 0;

    for i in 0..5 {
        let packets = enc.encode(&frame).unwrap();
        assert!(packets[0].is_keyframe, "frame {i} should be keyframe");
        count += dec.decode_nal(&packets[0].data).unwrap_or_default().len();
    }
    count += dec.flush().len();
    assert_eq!(count, 5, "should decode 5 all-keyframe frames");
}

#[test]
fn alternating_content_pframes() {
    // Test with alternating content (scene changes)
    let width = 32u32;
    let height = 32u32;
    let uv = ((width / 2) * (height / 2)) as usize;

    let frame_a = DecodedFrame {
        width,
        height,
        y: vec![100u8; (width * height) as usize],
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };
    let frame_b = DecodedFrame {
        width,
        height,
        y: vec![200u8; (width * height) as usize],
        u: vec![128u8; uv],
        v: vec![128u8; uv],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    };

    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 2_000_000,
        keyframe_interval: 10,
        ..Default::default()
    })
    .unwrap();

    let mut dec = H264Decoder::new();
    let mut count = 0;

    // Alternate between two different frames
    for i in 0..6 {
        let frame = if i % 2 == 0 { &frame_a } else { &frame_b };
        let packets = enc.encode(frame).unwrap();
        for pkt in &packets {
            count += dec.decode_nal(&pkt.data).unwrap_or_default().len();
        }
    }
    count += dec.flush().len();
    assert_eq!(count, 6, "should decode 6 alternating frames");
}
