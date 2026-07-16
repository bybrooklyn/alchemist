use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use crate::error::WhyThoError;
use crate::media::ContainerFormat;

use super::{
    AttachmentInfo, AudioStreamInfo, ChapterInfo, DetectedCodec, DetectedContainer, ProbeResult,
    StreamInfo, StreamKind, SubtitleStreamInfo, VideoStreamInfo,
};

pub fn probe_mkv(path: &Path) -> Result<ProbeResult, WhyThoError> {
    let file = File::open(path).map_err(|e| WhyThoError::ProbeFailed {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    let mkv = matroska::Matroska::open(file).map_err(|e| WhyThoError::ProbeFailed {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let format = if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("webm"))
        .unwrap_or(false)
    {
        ContainerFormat::WebM
    } else {
        ContainerFormat::Mkv
    };

    let container = DetectedContainer {
        format,
        matroska_version: None,
        writing_app: Some(mkv.info.writing_app.clone()).filter(|s| !s.is_empty()),
        muxing_app: Some(mkv.info.muxing_app.clone()).filter(|s| !s.is_empty()),
        duration: mkv.info.duration,
    };

    let mut tags = HashMap::new();
    if let Some(ref title) = mkv.info.title {
        tags.insert("title".into(), title.clone());
    }
    for tag in &mkv.tags {
        for simple in &tag.simple {
            if let Some(matroska::TagValue::String(ref val)) = simple.value {
                tags.insert(simple.name.clone(), val.clone());
            }
        }
    }

    let mut streams = Vec::new();
    for (idx, track) in mkv.tracks.iter().enumerate() {
        let codec = DetectedCodec::from_codec_id(&track.codec_id);
        let language = track.language.as_ref().map(|l| l.to_string());

        let kind = if track.is_video() {
            if let matroska::Settings::Video(ref video) = track.settings {
                let frame_rate = track
                    .default_duration
                    .filter(|d| !d.is_zero())
                    .map(|d| 1_000_000_000.0 / d.as_nanos() as f64);

                let (profile, level, pixel_format, bit_depth, hdr) = track
                    .codec_private
                    .as_deref()
                    .map(|cp| super::h264::parse_codec_private(cp, &track.codec_id))
                    .transpose()?
                    .unwrap_or((None, None, None, None, None));

                StreamKind::Video(VideoStreamInfo {
                    width: video.pixel_width as u32,
                    height: video.pixel_height as u32,
                    display_width: video.display_width.map(|v| v as u32),
                    display_height: video.display_height.map(|v| v as u32),
                    pixel_format,
                    bit_depth,
                    frame_rate,
                    bitrate: None,
                    profile,
                    level,
                    color_space: None,
                    color_range: None,
                    hdr,
                })
            } else {
                StreamKind::Video(VideoStreamInfo {
                    width: 0,
                    height: 0,
                    display_width: None,
                    display_height: None,
                    pixel_format: None,
                    bit_depth: None,
                    frame_rate: None,
                    bitrate: None,
                    profile: None,
                    level: None,
                    color_space: None,
                    color_range: None,
                    hdr: None,
                })
            }
        } else if track.is_audio() {
            if let matroska::Settings::Audio(ref audio) = track.settings {
                let channel_layout = match audio.channels {
                    1 => Some("mono".into()),
                    2 => Some("stereo".into()),
                    6 => Some("5.1".into()),
                    8 => Some("7.1".into()),
                    _ => None,
                };

                StreamKind::Audio(AudioStreamInfo {
                    sample_rate: Some(audio.sample_rate).filter(|&r| r > 0.0),
                    channels: Some(audio.channels).filter(|&c| c > 0),
                    channel_layout,
                    bit_depth: audio.bit_depth.map(|d| d as u8),
                    bitrate: None,
                })
            } else {
                StreamKind::Audio(AudioStreamInfo {
                    sample_rate: None,
                    channels: None,
                    channel_layout: None,
                    bit_depth: None,
                    bitrate: None,
                })
            }
        } else {
            StreamKind::Subtitle(SubtitleStreamInfo {
                codec_id: track.codec_id.clone(),
            })
        };

        streams.push(StreamInfo {
            index: idx as u32,
            track_number: track.number,
            kind,
            codec_id: track.codec_id.clone(),
            codec,
            language,
            name: track.name.clone(),
            default: track.default,
            forced: track.forced,
            enabled: track.enabled,
            duration: None,
        });
    }

    let chapters: Vec<ChapterInfo> = mkv
        .chapters
        .iter()
        .flat_map(|edition| &edition.chapters)
        .map(|ch| {
            let title = ch
                .display
                .first()
                .map(|d| d.string.clone())
                .unwrap_or_default();

            ChapterInfo {
                uid: ch.uid,
                start: ch.time_start,
                end: ch.time_end.unwrap_or(ch.time_start),
                title,
            }
        })
        .collect();

    let attachments: Vec<AttachmentInfo> = mkv
        .attachments
        .iter()
        .map(|a| AttachmentInfo {
            filename: a.name.clone(),
            mime_type: a.mime_type.clone(),
            size: a.data.len() as u64,
        })
        .collect();

    Ok(ProbeResult {
        path: path.to_path_buf(),
        container,
        size_bytes,
        streams,
        chapters,
        attachments,
        tags,
    })
}
