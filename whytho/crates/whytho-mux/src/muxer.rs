use std::io::{self, Write};
use std::time::Duration;

use whytho_types::EncodedPacket;

pub struct MuxerConfig {
    pub timecode_scale: u64,
    pub writing_app: String,
    pub duration: Option<Duration>,
}

impl Default for MuxerConfig {
    fn default() -> Self {
        Self {
            timecode_scale: 1_000_000,
            writing_app: "whytho".into(),
            duration: None,
        }
    }
}

pub struct TrackConfig {
    pub track_number: u32,
    pub codec_id: String,
    pub track_type: TrackType,
    pub name: Option<String>,
    pub language: Option<String>,
    pub default: bool,
    pub video: Option<VideoTrackConfig>,
    pub audio: Option<AudioTrackConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Video = 1,
    Audio = 2,
    Subtitle = 17,
}

pub struct VideoTrackConfig {
    pub width: u32,
    pub height: u32,
    pub codec_private: Option<Vec<u8>>,
}

pub struct AudioTrackConfig {
    pub sample_rate: f64,
    pub channels: u32,
    pub bit_depth: Option<u32>,
    pub codec_private: Option<Vec<u8>>,
}

pub struct MkvMuxer<W: Write> {
    writer: W,
    config: MuxerConfig,
    tracks: Vec<TrackConfig>,
    current_cluster_timecode: u64,
    cluster_start: bool,
}

impl<W: Write> MkvMuxer<W> {
    pub fn new(mut writer: W, config: MuxerConfig) -> io::Result<Self> {
        write_ebml_header(&mut writer)?;
        write_segment_start(&mut writer)?;

        let mut muxer = Self {
            writer,
            config,
            tracks: Vec::new(),
            current_cluster_timecode: 0,
            cluster_start: true,
        };

        muxer.write_segment_info()?;
        Ok(muxer)
    }

    pub fn add_track(&mut self, track: TrackConfig) {
        self.tracks.push(track);
    }

    pub fn write_tracks(&mut self) -> io::Result<()> {
        let tracks_data = encode_tracks(&self.tracks);
        let tracks_element = encode_master(0x1654AE6B, &tracks_data);
        self.writer.write_all(&tracks_element)?;
        Ok(())
    }

    pub fn write_packet(
        &mut self,
        track_number: u32,
        packet: &EncodedPacket,
        timecode: Duration,
    ) -> io::Result<()> {
        let tc_ms = timecode.as_millis() as u64;
        let relative_tc = tc_ms.saturating_sub(self.current_cluster_timecode);

        if self.cluster_start || relative_tc > 30000 {
            self.start_cluster(tc_ms)?;
        }

        let mut block_data = Vec::new();
        encode_variable_length(&mut block_data, track_number as u64)?;
        let tc_i16 = relative_tc as i16;
        block_data.extend_from_slice(&tc_i16.to_be_bytes());
        let flags: u8 = if packet.is_keyframe { 0x80 } else { 0x00 };
        block_data.push(flags);
        block_data.extend_from_slice(&packet.data);

        let block = encode_master(0xA3, &block_data);
        self.writer.write_all(&block)?;

        Ok(())
    }

    pub fn write_simple_block(
        &mut self,
        track_number: u32,
        data: &[u8],
        timecode: Duration,
        is_keyframe: bool,
    ) -> io::Result<()> {
        let tc_ms = timecode.as_millis() as u64;
        let relative_tc = tc_ms.saturating_sub(self.current_cluster_timecode);

        if self.cluster_start || relative_tc > 30000 {
            self.start_cluster(tc_ms)?;
        }

        let mut block_data = Vec::new();
        encode_variable_length(&mut block_data, track_number as u64)?;
        let tc_i16 = relative_tc as i16;
        block_data.extend_from_slice(&tc_i16.to_be_bytes());
        let flags: u8 = if is_keyframe { 0x80 } else { 0x00 };
        block_data.push(flags);
        block_data.extend_from_slice(data);

        let block = encode_master(0xA3, &block_data);
        self.writer.write_all(&block)?;

        Ok(())
    }

    fn start_cluster(&mut self, timecode_ms: u64) -> io::Result<()> {
        self.current_cluster_timecode = timecode_ms;
        self.cluster_start = false;

        // Write Cluster element ID
        let id = 0x1F43B675u32;
        self.writer.write_all(&id.to_be_bytes())?;
        // Write unknown size (8 bytes: 0x01 + 7x 0xFF)
        self.writer
            .write_all(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])?;

        // Write Timecode element inside the cluster
        let tc_data = encode_uint(0xE7, timecode_ms);
        self.writer.write_all(&tc_data)?;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn write_ebml_header(w: &mut impl Write) -> io::Result<()> {
    let mut header = Vec::new();
    header.extend_from_slice(&encode_uint(0x4286, 1)); // EBMLVersion
    header.extend_from_slice(&encode_uint(0x42F7, 1)); // EBMLReadVersion
    header.extend_from_slice(&encode_uint(0x42F2, 4)); // EBMLMaxIDLength
    header.extend_from_slice(&encode_uint(0x42F3, 8)); // EBMLMaxSizeLength
    header.extend_from_slice(&encode_string(0x4282, "matroska")); // DocType
    header.extend_from_slice(&encode_uint(0x4287, 4)); // DocTypeVersion
    header.extend_from_slice(&encode_uint(0x4285, 2)); // DocTypeReadVersion
    let ebml = encode_master(0x1A45DFA3, &header);
    w.write_all(&ebml)
}

fn write_segment_start(w: &mut impl Write) -> io::Result<()> {
    let id = [0x18, 0x53, 0x80, 0x67]; // Segment
    w.write_all(&id)?;
    let size_bytes = encode_unknown_size();
    w.write_all(&size_bytes)?;
    Ok(())
}

fn encode_unknown_size() -> Vec<u8> {
    // 8-byte unknown size: 0x01 followed by 7 bytes of 0xFF
    // In EBML variable-length integers:
    //   0x01 FF FF FF FF FF FF FF = 8-byte form with all value bits set = unknown
    vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
}

impl<W: Write> MkvMuxer<W> {
    fn write_segment_info(&mut self) -> io::Result<()> {
        let mut info = Vec::new();
        info.extend_from_slice(&encode_uint(0x2AD7B1, self.config.timecode_scale));
        if !self.config.writing_app.is_empty() {
            info.extend_from_slice(&encode_string(0x4D80, &self.config.writing_app));
        }
        info.extend_from_slice(&encode_string(0x5741, "whytho muxer"));
        if let Some(dur) = self.config.duration {
            let dur_f = dur.as_secs_f64() * 1000.0 / self.config.timecode_scale as f64;
            info.extend_from_slice(&encode_float(0x4489, dur_f));
        }
        let segment_info = encode_master(0x1549A966, &info);
        self.writer.write_all(&segment_info)?;
        Ok(())
    }
}

fn encode_tracks(tracks: &[TrackConfig]) -> Vec<u8> {
    let mut data = Vec::new();
    for track in tracks {
        data.extend_from_slice(&encode_track_entry(track));
    }
    data
}

fn encode_track_entry(track: &TrackConfig) -> Vec<u8> {
    let mut entry = Vec::new();
    entry.extend_from_slice(&encode_uint(0xD7, track.track_number as u64));
    entry.extend_from_slice(&encode_uint(0x73C5, track.track_number as u64)); // TrackUID
    entry.extend_from_slice(&encode_uint(0x83, track.track_type as u64));
    entry.extend_from_slice(&encode_string(0x86, &track.codec_id));
    entry.extend_from_slice(&encode_uint(0x88, if track.default { 1 } else { 0 }));

    if let Some(ref name) = track.name {
        entry.extend_from_slice(&encode_string(0x536E, name));
    }
    if let Some(ref lang) = track.language {
        entry.extend_from_slice(&encode_string(0x22B59C, lang));
    }

    if let Some(ref video) = track.video {
        let mut vid = Vec::new();
        vid.extend_from_slice(&encode_uint(0xB0, video.width as u64));
        vid.extend_from_slice(&encode_uint(0xBA, video.height as u64));
        entry.extend_from_slice(&encode_master(0xE0, &vid));

        if let Some(ref cp) = video.codec_private {
            entry.extend_from_slice(&encode_binary(0x63A2, cp));
        }
    }

    if let Some(ref audio) = track.audio {
        let mut aud = Vec::new();
        aud.extend_from_slice(&encode_float(0xB5, audio.sample_rate));
        aud.extend_from_slice(&encode_uint(0x9F, audio.channels as u64));
        if let Some(depth) = audio.bit_depth {
            aud.extend_from_slice(&encode_uint(0x6264, depth as u64));
        }
        entry.extend_from_slice(&encode_master(0xE1, &aud));

        if let Some(ref cp) = audio.codec_private {
            entry.extend_from_slice(&encode_binary(0x63A2, cp));
        }
    }

    encode_master(0xAE, &entry)
}

fn encode_master(id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_element_id(&mut out, id);
    write_element_size(&mut out, body.len() as u64);
    out.extend_from_slice(body);
    out
}

fn encode_uint(id: u32, value: u64) -> Vec<u8> {
    let bytes = uint_to_bytes(value);
    let mut out = Vec::new();
    write_element_id(&mut out, id);
    write_element_size(&mut out, bytes.len() as u64);
    out.extend_from_slice(&bytes);
    out
}

fn encode_float(id: u32, value: f64) -> Vec<u8> {
    let mut out = Vec::new();
    write_element_id(&mut out, id);
    write_element_size(&mut out, 8);
    out.extend_from_slice(&value.to_be_bytes());
    out
}

fn encode_string(id: u32, value: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_element_id(&mut out, id);
    write_element_size(&mut out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
    out
}

fn encode_binary(id: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_element_id(&mut out, id);
    write_element_size(&mut out, data.len() as u64);
    out.extend_from_slice(data);
    out
}

fn write_element_id(out: &mut Vec<u8>, id: u32) {
    if id <= 0xFF {
        out.push(id as u8);
    } else if id <= 0xFFFF {
        out.extend_from_slice(&(id as u16).to_be_bytes());
    } else if id <= 0xFFFFFF {
        out.push((id >> 16) as u8);
        out.push((id >> 8) as u8);
        out.push(id as u8);
    } else {
        out.extend_from_slice(&id.to_be_bytes());
    }
}

fn write_element_size(out: &mut Vec<u8>, size: u64) {
    if size < 0x7F {
        out.push(size as u8 | 0x80);
    } else if size < 0x3FFF {
        out.extend_from_slice(&(size as u16 | 0x4000).to_be_bytes());
    } else if size < 0x1F_FFFF {
        out.push((size >> 16) as u8 | 0x20);
        out.push((size >> 8) as u8);
        out.push(size as u8);
    } else {
        out.push((size >> 24) as u8 | 0x10);
        out.push((size >> 16) as u8);
        out.push((size >> 8) as u8);
        out.push(size as u8);
    }
}

fn uint_to_bytes(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0];
    }
    let mut bytes = Vec::new();
    let mut v = value;
    while v > 0 {
        bytes.push((v & 0xFF) as u8);
        v >>= 8;
    }
    bytes.reverse();
    bytes
}

fn encode_variable_length(out: &mut Vec<u8>, value: u64) -> io::Result<()> {
    if value < 0x80 {
        out.push(value as u8 | 0x80);
    } else if value < 0x4000 {
        out.extend_from_slice(&(value as u16 | 0x4000).to_be_bytes());
    } else if value < 0x200000 {
        out.push((value >> 16) as u8 | 0x20);
        out.push((value >> 8) as u8);
        out.push(value as u8);
    } else {
        out.push((value >> 24) as u8 | 0x10);
        out.push((value >> 16) as u8);
        out.push((value >> 8) as u8);
        out.push(value as u8);
    }
    Ok(())
}
