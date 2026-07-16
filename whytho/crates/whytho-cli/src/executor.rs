use std::fs::File;
use std::io::BufWriter;
use std::time::{Duration, Instant};

use matroska_demuxer::{Frame as MkvFrame, MatroskaFile, TrackType};
use whytho_codecs::{Av1Encoder, H264Decoder, H264Encoder};
use whytho_codecs::{VideoEncoder, VideoEncoderConfig};
use whytho_mux::muxer::{AudioTrackConfig, MkvMuxer, MuxerConfig, TrackConfig, VideoTrackConfig};

use whytho_core::error::WhyThoError;
use whytho_core::media::VideoCodec;
use whytho_core::transcode::{TranscodeJob, TranscodeReport, TranscodeStatus};

pub fn execute(job: &TranscodeJob) -> Result<TranscodeReport, WhyThoError> {
    let start = Instant::now();

    let video_stream = job
        .probe
        .streams
        .iter()
        .find(|s| matches!(s.kind, whytho_core::probe::StreamKind::Video(_)))
        .ok_or_else(|| WhyThoError::Config("no video stream found".into()))?;

    let video_info = match &video_stream.kind {
        whytho_core::probe::StreamKind::Video(v) => v,
        _ => unreachable!(),
    };

    let fps = video_info.frame_rate.unwrap_or(24.0);

    let input_file = File::open(job.input.path()).map_err(|e| WhyThoError::ProbeFailed {
        path: job.input.path().to_path_buf(),
        source: Box::new(e),
    })?;
    let mut mkv = MatroskaFile::open(input_file).map_err(|e| WhyThoError::ProbeFailed {
        path: job.input.path().to_path_buf(),
        source: Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )),
    })?;

    let video_track_entry = mkv
        .tracks()
        .iter()
        .find(|t| t.track_type() == TrackType::Video)
        .ok_or_else(|| WhyThoError::Config("no video track in source".into()))?;
    let video_track_number = video_track_entry.track_number().get();

    let codec_private = video_track_entry.codec_private().map(|cp| cp.to_vec());

    let mut decoder = H264Decoder::with_fps(fps);
    if let Some(ref cp) = codec_private {
        decoder.init_avcc(cp).map_err(|e| WhyThoError::Config(e))?;
    } else {
        return Err(WhyThoError::Config(
            "H.264 stream has no CodecPrivate (AVCC) data".into(),
        ));
    }

    let enc_config = VideoEncoderConfig {
        width: video_info.width,
        height: video_info.height,
        fps,
        bitrate: job.config.default_video_codec_bitrate(),
        keyframe_interval: 240,
        speed_preset: 6,
    };

    let target_codec = job.config.default_video_codec;

    // Create encoder based on target codec
    let (mut encoder, codec_id, av1_seq_header): (Box<dyn VideoEncoder>, String, Option<Vec<u8>>) =
        match target_codec {
            VideoCodec::Av1 => {
                let mut enc = Av1Encoder::new();
                enc.configure(&enc_config).map_err(WhyThoError::Config)?;
                let seq = enc.sequence_header();
                (Box::new(enc), "V_AV1".into(), seq)
            }
            VideoCodec::H264 => {
                let mut enc = H264Encoder::new();
                enc.configure(&enc_config).map_err(WhyThoError::Config)?;
                (Box::new(enc), "V_MPEG4/ISO/AVC".into(), None)
            }
            _ => {
                return Err(WhyThoError::Config(format!(
                    "unsupported target codec: {target_codec}"
                )));
            }
        };

    let output_file = File::create(&job.output).map_err(|e| WhyThoError::ProbeFailed {
        path: job.output.clone(),
        source: Box::new(e),
    })?;
    let mut muxer =
        MkvMuxer::new(BufWriter::new(output_file), MuxerConfig::default()).map_err(|e| {
            WhyThoError::ProbeFailed {
                path: job.output.clone(),
                source: Box::new(e),
            }
        })?;

    let mut mux_track_number = 1u32;
    let video_track_idx = mux_track_number;

    muxer.add_track(TrackConfig {
        track_number: mux_track_number,
        codec_id,
        track_type: whytho_mux::muxer::TrackType::Video,
        name: None,
        language: None,
        default: true,
        video: Some(VideoTrackConfig {
            width: video_info.width,
            height: video_info.height,
            codec_private: av1_seq_header,
        }),
        audio: None,
    });

    let audio_tracks: Vec<_> = mkv
        .tracks()
        .iter()
        .filter(|t| t.track_type() == TrackType::Audio)
        .collect();

    let mut audio_track_map: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    for audio_track in &audio_tracks {
        mux_track_number += 1;
        audio_track_map.insert(audio_track.track_number().get(), mux_track_number);
        let audio = audio_track.audio();
        muxer.add_track(TrackConfig {
            track_number: mux_track_number,
            codec_id: audio_track.codec_id().to_string(),
            track_type: whytho_mux::muxer::TrackType::Audio,
            name: audio_track.name().map(|s| s.to_string()),
            language: audio_track.language().map(|s| s.to_string()),
            default: audio_track.flag_default(),
            video: None,
            audio: Some(AudioTrackConfig {
                sample_rate: audio.map(|a| a.sampling_frequency()).unwrap_or(48000.0),
                channels: audio.map(|a| a.channels().get() as u32).unwrap_or(2),
                bit_depth: audio.and_then(|a| a.bit_depth()).map(|d| d.get() as u32),
                codec_private: audio_track.codec_private().map(|cp| cp.to_vec()),
            }),
        });
    }

    muxer.write_tracks().map_err(|e| WhyThoError::ProbeFailed {
        path: job.output.clone(),
        source: Box::new(e),
    })?;

    let mut frames_encoded = 0u64;
    let mut last_timestamp = 0u64;
    let mut first_video_ts: Option<u64> = None;
    let mut frame = MkvFrame::default();

    while mkv
        .next_frame(&mut frame)
        .map_err(|e| WhyThoError::ProbeFailed {
            path: job.input.path().to_path_buf(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )),
        })?
    {
        if frame.track == video_track_number {
            if first_video_ts.is_none() {
                first_video_ts = Some(frame.timestamp);
            }
            let normalized_ts = frame.timestamp - first_video_ts.unwrap_or(0);
            last_timestamp = normalized_ts;

            let decoded = decoder
                .decode_sample(&frame.data)
                .map_err(|e| WhyThoError::Config(e))?;

            for dec_frame in decoded {
                let packets = encoder.encode(&dec_frame).map_err(WhyThoError::Config)?;

                for pkt in packets {
                    let tc = Duration::from_millis(normalized_ts);
                    muxer.write_packet(video_track_idx, &pkt, tc).map_err(|e| {
                        WhyThoError::ProbeFailed {
                            path: job.output.clone(),
                            source: Box::new(e),
                        }
                    })?;
                    frames_encoded += 1;
                }
            }
        } else if let Some(&mux_track) = audio_track_map.get(&frame.track) {
            let normalized_ts = frame.timestamp - first_video_ts.unwrap_or(0);
            muxer
                .write_simple_block(
                    mux_track,
                    &frame.data,
                    Duration::from_millis(normalized_ts),
                    false,
                )
                .map_err(|e| WhyThoError::ProbeFailed {
                    path: job.output.clone(),
                    source: Box::new(e),
                })?;
        }
    }

    let remaining = encoder.flush().map_err(WhyThoError::Config)?;
    for pkt in remaining {
        let tc = Duration::from_millis(last_timestamp);
        muxer
            .write_packet(video_track_idx, &pkt, tc)
            .map_err(|e| WhyThoError::ProbeFailed {
                path: job.output.clone(),
                source: Box::new(e),
            })?;
        frames_encoded += 1;
    }

    muxer.finish().map_err(|e| WhyThoError::ProbeFailed {
        path: job.output.clone(),
        source: Box::new(e),
    })?;

    let elapsed = start.elapsed();
    let avg_fps = if elapsed.as_secs_f64() > 0.0 {
        frames_encoded as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let output_size = std::fs::metadata(&job.output).map(|m| m.len()).unwrap_or(0);
    let input_size = std::fs::metadata(job.input.path())
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(TranscodeReport {
        input: job.input.path().to_path_buf(),
        output: job.output.clone(),
        frames_encoded,
        elapsed,
        avg_fps,
        output_size,
        input_size,
        status: TranscodeStatus::Complete,
        verification: None,
        chunk_plan: None,
    })
}
