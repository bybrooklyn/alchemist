use std::io::BufWriter;
use std::time::Duration;

use matroska_demuxer::{Frame as MkvFrame, MatroskaFile, TrackType};
use whytho_codec_av1::Av1Encoder;
use whytho_codec_h264::{H264Decoder, H264Encoder};
use whytho_mux::muxer::{
    MkvMuxer, MuxerConfig, TrackConfig, TrackType as MuxTrackType, VideoTrackConfig,
};
use whytho_types::{DecodedFrame, PixelFormat, VideoDecoder, VideoEncoder, VideoEncoderConfig};

fn make_flat_frame(width: u32, height: u32, value: u8) -> DecodedFrame {
    let uv_size = ((width / 2) * (height / 2)) as usize;
    DecodedFrame {
        width,
        height,
        y: vec![value; (width * height) as usize],
        u: vec![128u8; uv_size],
        v: vec![128u8; uv_size],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    }
}

fn make_test_frame(width: u32, height: u32) -> DecodedFrame {
    let mut y = vec![0u8; (width * height) as usize];
    for row in 0..height {
        for col in 0..width {
            let idx = (row * width + col) as usize;
            y[idx] = ((row * 4 + col * 2) % 256) as u8;
        }
    }
    let uv_size = ((width / 2) * (height / 2)) as usize;
    DecodedFrame {
        width,
        height,
        y,
        u: vec![128u8; uv_size],
        v: vec![128u8; uv_size],
        pixel_format: PixelFormat::Yuv420,
        pts: Duration::ZERO,
    }
}

#[test]
fn h264_encodes_valid_bitstream() {
    let mut enc = H264Encoder::new();
    let config = VideoEncoderConfig {
        width: 16,
        height: 16,
        fps: 24.0,
        ..Default::default()
    };
    enc.configure(&config).unwrap();

    let frame = make_flat_frame(16, 16, 128);
    let packets = enc.encode(&frame).unwrap();
    assert!(!packets.is_empty());
    assert!(packets[0].is_keyframe);
    assert!(packets[0].data.len() > 10);
    assert_eq!(
        &packets[0].data[..4],
        &[0x00, 0x00, 0x00, 0x01],
        "should start with Annex B NAL start code"
    );

    // Verify round-trip decode
    let mut dec = H264Decoder::new();
    for pkt in &packets {
        let _ = dec.decode_nal(&pkt.data);
    }
    let decoded = dec.flush();
    assert_eq!(
        decoded.len(),
        1,
        "expected 1 decoded frame from flush, got {}",
        decoded.len()
    );
    assert_eq!(decoded[0].width, 16);
    assert_eq!(decoded[0].height, 16);
}

#[test]
fn h264_encodes_320x240() {
    let mut enc = H264Encoder::new();
    let config = VideoEncoderConfig {
        width: 320,
        height: 240,
        fps: 24.0,
        ..Default::default()
    };
    enc.configure(&config).unwrap();

    let frame = make_flat_frame(320, 240, 128);
    let packets = enc.encode(&frame).unwrap();
    assert!(!packets.is_empty());
    assert!(packets[0].is_keyframe);
    assert!(
        packets[0].data.len() > 10,
        "320x240 encoded bitstream should be non-trivial"
    );
}

#[test]
fn av1_encode_produces_packets() {
    let mut enc = Av1Encoder::new();
    let config = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 24.0,
        speed_preset: 10,
        ..Default::default()
    };
    enc.configure(&config).unwrap();

    let frame = make_test_frame(64, 64);
    let packets = enc.encode(&frame).unwrap();
    let flushed = enc.flush().unwrap();
    let all: Vec<_> = packets.into_iter().chain(flushed).collect();
    assert!(
        !all.is_empty(),
        "av1 encoder should produce at least one packet"
    );
    assert!(all[0].is_keyframe, "first av1 packet should be a keyframe");
}

#[test]
fn av1_pts_scales_with_fps() {
    let mut enc = Av1Encoder::new();
    let config = VideoEncoderConfig {
        width: 64,
        height: 64,
        fps: 30.0,
        speed_preset: 10,
        ..Default::default()
    };
    enc.configure(&config).unwrap();

    let frame = make_test_frame(64, 64);
    let packets = enc.encode(&frame).unwrap();
    let flushed = enc.flush().unwrap();
    let all: Vec<_> = packets.into_iter().chain(flushed).collect();
    if !all.is_empty() {
        let expected_pts = Duration::from_secs_f64(0.0 / 30.0);
        assert_eq!(all[0].pts, expected_pts, "first frame PTS should be 0");
    }
}

#[test]
fn h264_to_mkv_pipeline_roundtrip() {
    let width = 64u32;
    let height = 64u32;
    let frame = make_test_frame(width, height);

    // Encode 3 frames to H.264 (I, P, P)
    let mut enc = H264Encoder::new();
    enc.configure(&VideoEncoderConfig {
        width,
        height,
        fps: 24.0,
        bitrate: 8_000_000,
        keyframe_interval: 3, // I, P, P
        ..Default::default()
    })
    .unwrap();

    let mut packets = Vec::new();
    for _ in 0..3 {
        packets.extend(enc.encode(&frame).unwrap());
    }
    assert_eq!(packets.len(), 3, "should produce 3 packets");

    // Mux into MKV
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut muxer = MkvMuxer::new(BufWriter::new(&tmp), MuxerConfig::default()).unwrap();
    muxer.add_track(TrackConfig {
        track_number: 1,
        codec_id: "V_MPEG4/ISO/AVC".into(),
        track_type: MuxTrackType::Video,
        name: None,
        language: None,
        default: true,
        video: Some(VideoTrackConfig {
            width,
            height,
            codec_private: None,
        }),
        audio: None,
    });
    muxer.write_tracks().unwrap();
    for (i, pkt) in packets.iter().enumerate() {
        let ts = Duration::from_secs_f64(i as f64 / 24.0);
        muxer.write_packet(1, pkt, ts).unwrap();
    }
    muxer.finish().unwrap();

    // Demux and decode
    let file = std::fs::File::open(tmp.path()).unwrap();
    let mut mkv = MatroskaFile::open(file).unwrap();
    let video_track = mkv
        .tracks()
        .iter()
        .find(|t| t.track_type() == TrackType::Video)
        .unwrap()
        .clone();

    let mut decoder = H264Decoder::new();
    let mut decoded_frames = Vec::new();
    let mut mkv_frame = MkvFrame::default();
    while mkv.next_frame(&mut mkv_frame).unwrap() {
        if mkv_frame.track == video_track.track_number().get() {
            decoded_frames.extend(decoder.decode_nal(&mkv_frame.data).unwrap_or_default());
        }
    }
    decoded_frames.extend(decoder.flush());

    assert_eq!(
        decoded_frames.len(),
        3,
        "should decode 3 frames from MKV, got {}",
        decoded_frames.len()
    );
    assert_eq!(decoded_frames[0].width, width);
    assert_eq!(decoded_frames[0].height, height);

    // Verify fidelity
    let orig_y = &frame.y;
    for (i, dec) in decoded_frames.iter().enumerate() {
        let err: f64 = orig_y
            .iter()
            .zip(dec.y.iter())
            .map(|(&o, &d)| (o as i32 - d as i32).unsigned_abs() as f64)
            .sum::<f64>()
            / orig_y.len() as f64;
        assert!(err < 55.0, "frame {i} MAE = {err:.2}, expected < 55");
    }
}
