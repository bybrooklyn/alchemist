//! Adapter implementing the `whytho-types` [`VideoEncoder`] contract over the AV2 [`Encoder`].
//!
//! This is the seam that lets the codec facade and CLI drive AV2 through the same trait as every
//! other codec. The underlying AV2 pipeline is a 128x128 8-bit 4:2:0 still-image bootstrap, so
//! `configure` rejects other geometries deterministically (via [`Encoder::new`]).

use crate::common::image::{Frame, Plane};
use whytho_types::{DecodedFrame, EncodedPacket, VideoCodec, VideoEncoder, VideoEncoderConfig};

use crate::encoder::{Config, Encoder};

/// Default `base_q_idx` used until a rate-control mapping exists (AV2 ratectrl is fixed-QP).
const DEFAULT_BASE_QINDEX: u8 = 128;

/// Drives the AV2 [`Encoder`] through the `whytho.` [`VideoEncoder`] trait.
#[derive(Default)]
pub struct Av2Encoder {
    encoder: Option<Encoder>,
}

impl Av2Encoder {
    pub fn new() -> Self {
        Self { encoder: None }
    }
}

impl VideoEncoder for Av2Encoder {
    fn name(&self) -> &str {
        "av2"
    }

    fn codec(&self) -> VideoCodec {
        VideoCodec::Av2
    }

    fn configure(&mut self, config: &VideoEncoderConfig) -> Result<(), String> {
        let cfg = Config::new(config.width, config.height, 8, DEFAULT_BASE_QINDEX);
        // `Encoder::new` validates the geometry/bit-depth the skeleton supports and fails fast.
        self.encoder = Some(Encoder::new(cfg).map_err(|e| e.to_string())?);
        Ok(())
    }

    fn encode(&mut self, frame: &DecodedFrame) -> Result<Vec<EncodedPacket>, String> {
        let encoder = self
            .encoder
            .as_ref()
            .ok_or("av2 encoder used before configure()")?;

        let av2_frame = to_av2_frame(frame);
        let data = encoder
            .encode_frame(&av2_frame)
            .map_err(|e| e.to_string())?;

        Ok(vec![EncodedPacket {
            data,
            pts: frame.pts,
            is_keyframe: true,
        }])
    }

    fn flush(&mut self) -> Result<Vec<EncodedPacket>, String> {
        // The still-image bootstrap encodes each frame eagerly; nothing is buffered.
        Ok(Vec::new())
    }
}

/// Convert a `whytho.` planar 8-bit frame into an AV2 `u16`-backed 4:2:0 frame.
fn to_av2_frame(frame: &DecodedFrame) -> Frame {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let mut out = Frame::new_420(width, height, 8);

    let (yw, yh) = (out.y.width(), out.y.height());
    copy_plane(&mut out.y, &frame.y, yw, yh);
    let (cw, ch) = (out.u.width(), out.u.height());
    copy_plane(&mut out.u, &frame.u, cw, ch);
    copy_plane(&mut out.v, &frame.v, cw, ch);
    out
}

/// Copy a tightly packed 8-bit source plane into a stride-padded `u16` AV2 plane.
fn copy_plane(plane: &mut Plane, src: &[u8], width: usize, height: usize) {
    for y in 0..height {
        let Some(row) = plane.row_mut(y) else { break };
        let base = y * width;
        for x in 0..width {
            if let Some(&sample) = src.get(base + x) {
                row[x] = u16::from(sample);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use whytho_types::PixelFormat;

    fn solid_frame(side: usize) -> DecodedFrame {
        let chroma = side / 2;
        DecodedFrame {
            width: side as u32,
            height: side as u32,
            y: vec![16; side * side],
            u: vec![128; chroma * chroma],
            v: vec![128; chroma * chroma],
            pixel_format: PixelFormat::Yuv420,
            pts: Duration::ZERO,
        }
    }

    #[test]
    fn configure_rejects_non_bootstrap_geometry() {
        let mut enc = Av2Encoder::new();
        let cfg = VideoEncoderConfig {
            width: 1920,
            height: 1080,
            ..Default::default()
        };
        assert!(enc.configure(&cfg).is_err());
    }

    #[test]
    fn encodes_a_bootstrap_frame_through_the_trait() {
        let mut enc = Av2Encoder::new();
        let cfg = VideoEncoderConfig {
            width: 128,
            height: 128,
            ..Default::default()
        };
        enc.configure(&cfg).expect("128x128 8-bit is supported");

        let packets = enc.encode(&solid_frame(128)).expect("encode succeeds");
        assert_eq!(packets.len(), 1);
        assert!(packets[0].is_keyframe);
        assert!(!packets[0].data.is_empty());
        assert!(enc.flush().expect("flush ok").is_empty());
    }
}
