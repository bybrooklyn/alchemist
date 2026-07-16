use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use crate::error::WhyThoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkingMode {
    Disabled,
    Enabled,
    KeyframeAware,
}

impl Default for ChunkingMode {
    fn default() -> Self {
        Self::KeyframeAware
    }
}

impl fmt::Display for ChunkingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "disabled"),
            Self::Enabled => write!(f, "enabled"),
            Self::KeyframeAware => write!(f, "keyframe-aware"),
        }
    }
}

impl FromStr for ChunkingMode {
    type Err = WhyThoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disabled" => Ok(Self::Disabled),
            "enabled" => Ok(Self::Enabled),
            "keyframe-aware" => Ok(Self::KeyframeAware),
            _ => Err(WhyThoError::InvalidValue {
                field: "chunking".into(),
                value: s.into(),
            }),
        }
    }
}

/// A chunk of a media file for parallel processing.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub index: usize,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_pts: Duration,
    pub end_pts: Duration,
}

impl Chunk {
    pub fn frame_count(&self) -> u64 {
        self.end_frame - self.start_frame
    }
}

/// Plan how to split a video into chunks for parallel encoding.
#[derive(Debug, Clone)]
pub struct ChunkPlan {
    pub mode: ChunkingMode,
    pub total_frames: u64,
    pub chunks: Vec<Chunk>,
}

impl ChunkPlan {
    /// Create a plan that splits `total_frames` into `num_chunks` roughly equal pieces.
    pub fn fixed(total_frames: u64, num_chunks: usize, fps: f64) -> Self {
        if num_chunks <= 1 || total_frames <= 1 {
            return Self {
                mode: ChunkingMode::Enabled,
                total_frames,
                chunks: vec![Chunk {
                    index: 0,
                    start_frame: 0,
                    end_frame: total_frames,
                    start_pts: Duration::ZERO,
                    end_pts: Duration::from_secs_f64(total_frames as f64 / fps),
                }],
            };
        }

        let chunk_size = (total_frames + num_chunks as u64 - 1) / num_chunks as u64;
        let mut chunks = Vec::new();
        let mut start = 0u64;
        let mut idx = 0;

        while start < total_frames {
            let end = (start + chunk_size).min(total_frames);
            chunks.push(Chunk {
                index: idx,
                start_frame: start,
                end_frame: end,
                start_pts: Duration::from_secs_f64(start as f64 / fps),
                end_pts: Duration::from_secs_f64(end as f64 / fps),
            });
            start = end;
            idx += 1;
        }

        Self {
            mode: ChunkingMode::Enabled,
            total_frames,
            chunks,
        }
    }

    /// Create a keyframe-aware plan that aligns chunks with keyframe positions.
    pub fn keyframe_aligned(total_frames: u64, keyframes: &[u64], fps: f64) -> Self {
        if keyframes.is_empty() || total_frames <= 1 {
            return Self::fixed(total_frames, 1, fps);
        }

        let mut chunks = Vec::new();
        let mut chunk_start = 0u64;
        let mut idx = 0;

        for &kf in keyframes {
            if kf > chunk_start && kf < total_frames {
                chunks.push(Chunk {
                    index: idx,
                    start_frame: chunk_start,
                    end_frame: kf,
                    start_pts: Duration::from_secs_f64(chunk_start as f64 / fps),
                    end_pts: Duration::from_secs_f64(kf as f64 / fps),
                });
                chunk_start = kf;
                idx += 1;
            }
        }

        // Final chunk from last keyframe to end
        if chunk_start < total_frames {
            chunks.push(Chunk {
                index: idx,
                start_frame: chunk_start,
                end_frame: total_frames,
                start_pts: Duration::from_secs_f64(chunk_start as f64 / fps),
                end_pts: Duration::from_secs_f64(total_frames as f64 / fps),
            });
        }

        Self {
            mode: ChunkingMode::KeyframeAware,
            total_frames,
            chunks,
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl fmt::Display for ChunkPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Chunk plan ({}, {} chunks):",
            self.mode,
            self.chunks.len()
        )?;
        for chunk in &self.chunks {
            writeln!(
                f,
                "  chunk {}: frames {}-{} ({:.1}s-{:.1}s)",
                chunk.index,
                chunk.start_frame,
                chunk.end_frame,
                chunk.start_pts.as_secs_f64(),
                chunk.end_pts.as_secs_f64()
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_chunk_plan_even_split() {
        let plan = ChunkPlan::fixed(100, 4, 24.0);
        assert_eq!(plan.chunk_count(), 4);
        assert_eq!(plan.chunks[0].start_frame, 0);
        assert_eq!(plan.chunks[0].end_frame, 25);
        assert_eq!(plan.chunks[3].end_frame, 100);
    }

    #[test]
    fn fixed_chunk_plan_uneven() {
        let plan = ChunkPlan::fixed(10, 3, 24.0);
        assert_eq!(plan.chunk_count(), 3);
        assert_eq!(plan.chunks[2].end_frame, 10);
    }

    #[test]
    fn fixed_chunk_plan_single_chunk() {
        let plan = ChunkPlan::fixed(100, 1, 24.0);
        assert_eq!(plan.chunk_count(), 1);
        assert_eq!(plan.chunks[0].start_frame, 0);
        assert_eq!(plan.chunks[0].end_frame, 100);
    }

    #[test]
    fn keyframe_aligned_plan() {
        let kfs = vec![0, 30, 60, 90];
        let plan = ChunkPlan::keyframe_aligned(100, &kfs, 24.0);
        assert_eq!(plan.chunk_count(), 4);
        assert_eq!(plan.chunks[0].start_frame, 0);
        assert_eq!(plan.chunks[0].end_frame, 30);
        assert_eq!(plan.chunks[1].start_frame, 30);
        assert_eq!(plan.chunks[3].end_frame, 100);
    }

    #[test]
    fn keyframe_aligned_no_keyframes() {
        let plan = ChunkPlan::keyframe_aligned(100, &[], 24.0);
        assert_eq!(plan.chunk_count(), 1);
    }

    #[test]
    fn display_format() {
        let plan = ChunkPlan::fixed(100, 2, 24.0);
        let display = format!("{plan}");
        assert!(display.contains("2 chunks"));
    }
}
