use rav1e::prelude::*;

use whytho_types::VideoCodec;

use super::{DecodedFrame, EncodedPacket, VideoEncoder, VideoEncoderConfig};

pub struct Av1Encoder {
    ctx: Option<rav1e::Context<u8>>,
    config: Option<VideoEncoderConfig>,
    frame_count: u64,
}

impl Av1Encoder {
    pub fn new() -> Self {
        Self {
            ctx: None,
            config: None,
            frame_count: 0,
        }
    }

    pub fn sequence_header(&self) -> Option<Vec<u8>> {
        self.ctx.as_ref().map(|c| c.container_sequence_header())
    }
}

impl VideoEncoder for Av1Encoder {
    fn name(&self) -> &str {
        "rav1e"
    }

    fn codec(&self) -> VideoCodec {
        VideoCodec::Av1
    }

    fn configure(&mut self, config: &VideoEncoderConfig) -> Result<(), String> {
        let mut enc = EncoderConfig::default();
        enc.width = config.width as usize;
        enc.height = config.height as usize;
        enc.bitrate = config.bitrate as i32;
        enc.min_key_frame_interval = config.keyframe_interval / 2;
        enc.max_key_frame_interval = config.keyframe_interval;
        enc.speed_settings = SpeedSettings::from_preset(config.speed_preset);
        enc.low_latency = true;
        enc.chroma_sampling = ChromaSampling::Cs420;

        let fps_num = (config.fps * 1000.0) as u64;
        enc.time_base = Rational {
            num: 1000,
            den: fps_num,
        };

        let ctx: Context<u8> = Config::new()
            .with_encoder_config(enc)
            .new_context()
            .map_err(|e| format!("rav1e context error: {e:?}"))?;

        self.ctx = Some(ctx);
        self.config = Some(config.clone());
        self.frame_count = 0;
        Ok(())
    }

    fn encode(&mut self, frame: &DecodedFrame) -> Result<Vec<EncodedPacket>, String> {
        let ctx = self.ctx.as_mut().ok_or("encoder not configured")?;

        let mut f = ctx.new_frame();

        copy_plane(&mut f.planes[0], &frame.y, frame.width as usize);
        let uv_w = (frame.width / 2) as usize;
        copy_plane(&mut f.planes[1], &frame.u, uv_w);
        copy_plane(&mut f.planes[2], &frame.v, uv_w);

        ctx.send_frame(f)
            .map_err(|e| format!("rav1e send_frame error: {e:?}"))?;
        self.frame_count += 1;
        let fps = self.config.as_ref().map(|c| c.fps).unwrap_or(24.0);
        drain_packets(ctx, fps)
    }

    fn flush(&mut self) -> Result<Vec<EncodedPacket>, String> {
        let ctx = match self.ctx.as_mut() {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };
        ctx.flush();
        let fps = self.config.as_ref().map(|c| c.fps).unwrap_or(24.0);
        drain_packets(ctx, fps)
    }
}

fn copy_plane(plane: &mut v_frame::plane::Plane<u8>, src: &[u8], width: usize) {
    let stride = plane.cfg.stride;
    for (dst_row, src_row) in plane
        .data_origin_mut()
        .chunks_mut(stride)
        .zip(src.chunks(width))
    {
        let n = src_row.len().min(width);
        dst_row[..n].copy_from_slice(&src_row[..n]);
    }
}

fn drain_packets(ctx: &mut rav1e::Context<u8>, fps: f64) -> Result<Vec<EncodedPacket>, String> {
    let mut packets = Vec::new();
    loop {
        match ctx.receive_packet() {
            Ok(pkt) => {
                packets.push(EncodedPacket {
                    data: pkt.data,
                    pts: std::time::Duration::from_secs_f64(pkt.input_frameno as f64 / fps),
                    is_keyframe: pkt.frame_type == FrameType::KEY,
                });
            }
            Err(EncoderStatus::Encoded) => continue,
            Err(EncoderStatus::NeedMoreData) | Err(EncoderStatus::LimitReached) => break,
            Err(e) => return Err(format!("rav1e error: {e:?}")),
        }
    }
    Ok(packets)
}
