use std::num::NonZero;

use crate::{
    base::VInt64,
    lacer::Lacer,
    leaf::SimpleBlock,
    master::{BlockGroup, Cluster},
    *,
};

/// Frame data, either a single frame or multiple frames (in case of lacing)
/// See `Frame` for more details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameData<'a> {
    /// single frame data
    Single(&'a [u8]),
    /// multiple frame data (in case of lacing)
    Multiple(Vec<&'a [u8]>),
}

impl<'a> FrameData<'a> {
    fn single(data: &'a [u8]) -> Self {
        FrameData::Single(data)
    }
    fn multiple(data: Vec<&'a [u8]>) -> Self {
        FrameData::Multiple(data)
    }
}
/// A Matroska Frame, representing a block(SimpleBlock/BlockGroup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    /// frame data, either a single frame or multiple frames (in case of lacing)
    pub data: FrameData<'a>,
    /// whether the frame is a keyframe
    pub is_keyframe: bool,
    /// whether the frame is invisible (mostly for subtitle tracks)
    pub is_invisible: bool,
    /// whether the frame is discardable (for video tracks, e.g. non-reference frames)
    pub is_discardable: bool,
    /// track number the frame belongs to
    pub track_number: u64,
    /// timestamp of the frame, in the same timescale as the Cluster timestamp
    pub timestamp: i64,
    /// duration of the frame, in the same timescale as the Cluster timestamp
    pub duration: Option<NonZero<u64>>,
}

/// A block in a Cluster, either a SimpleBlock or a BlockGroup.
///
/// This is a convenience enum to allow handling both types of blocks uniformly.
/// * when reading: often we just want to iterate over all blocks in a cluster, regardless of type.
/// * when writing: we may want to write a list of blocks of mixed types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterBlock {
    /// A SimpleBlock
    Simple(SimpleBlock),
    /// A BlockGroup
    Group(BlockGroup),
}
impl ClusterBlock {
    fn block_ref(&self) -> BlockRef<'_> {
        match self {
            ClusterBlock::Simple(b) => BlockRef::Simple(b),
            ClusterBlock::Group(b) => BlockRef::Group(b),
        }
    }
}
impl From<SimpleBlock> for ClusterBlock {
    fn from(b: SimpleBlock) -> Self {
        ClusterBlock::Simple(b)
    }
}
impl From<BlockGroup> for ClusterBlock {
    fn from(b: BlockGroup) -> Self {
        ClusterBlock::Group(b)
    }
}

impl Encode for ClusterBlock {
    fn encode<B: BufMut>(&self, buf: &mut B) -> crate::Result<()> {
        match self {
            ClusterBlock::Simple(b) => b.encode(buf),
            ClusterBlock::Group(b) => b.encode(buf),
        }
    }
}

enum BlockRef<'a> {
    Simple(&'a crate::leaf::SimpleBlock),
    Group(&'a crate::master::BlockGroup),
}

impl<'a> BlockRef<'a> {
    /// Converts the block into a single frame, placing delaced frames into a FrameData::Multiple.
    fn into_frame(self, cluster_ts: u64) -> crate::Result<Frame<'a>> {
        match self {
            BlockRef::Simple(block) => {
                let body_buf = &mut &block[..];
                let track_number = VInt64::decode(body_buf)?;
                let relative_timestamp = body_buf.try_get_i16()?;
                let flag = body_buf.try_get_u8()?;
                let data = *body_buf;
                let lacing = (flag >> 1) & 0x03;
                Ok(Frame {
                    data: match lacing {
                        0 => FrameData::single(data),
                        0b01 => FrameData::multiple(Lacer::Xiph.delace(data)?),
                        0b11 => FrameData::multiple(Lacer::Ebml.delace(data)?),
                        _ => FrameData::multiple(Lacer::FixedSize.delace(data)?),
                    },
                    is_keyframe: (flag & 0x80) != 0,
                    is_invisible: (flag & 0x08) != 0,
                    is_discardable: (flag & 0x01) != 0,
                    track_number: *track_number,
                    timestamp: cluster_ts as i64 + relative_timestamp as i64,
                    duration: None,
                })
            }
            BlockRef::Group(g) => {
                let block = &g.block;
                let body_buf = &mut &block[..];
                let track_number = VInt64::decode(body_buf)?;
                let relative_timestamp = body_buf.try_get_i16()?;
                let flag = body_buf.try_get_u8()?;
                let data = *body_buf;
                let lacing = (flag >> 1) & 0x03;

                Ok(Frame {
                    data: match lacing {
                        0 => FrameData::single(data),
                        0b01 => FrameData::multiple(Lacer::Xiph.delace(data)?),
                        0b11 => FrameData::multiple(Lacer::Ebml.delace(data)?),
                        _ => FrameData::multiple(Lacer::FixedSize.delace(data)?),
                    },
                    is_keyframe: g.reference_block.is_empty(),
                    is_invisible: flag & 0x08 != 0,
                    is_discardable: false,
                    track_number: *track_number,
                    timestamp: cluster_ts as i64 + relative_timestamp as i64,
                    duration: g.block_duration.and_then(|d| NonZero::new(*d)),
                })
            }
        }
    }
}

impl<'a> From<&'a crate::leaf::SimpleBlock> for BlockRef<'a> {
    fn from(b: &'a crate::leaf::SimpleBlock) -> Self {
        BlockRef::Simple(b)
    }
}
impl<'a> From<&'a crate::master::BlockGroup> for BlockRef<'a> {
    fn from(b: &'a crate::master::BlockGroup) -> Self {
        BlockRef::Group(b)
    }
}

impl Cluster {
    /// frames in the cluster.
    pub fn frames(&self) -> impl Iterator<Item = crate::Result<Frame<'_>>> + '_ {
        self.blocks
            .iter()
            .map(|b| b.block_ref().into_frame(*self.timestamp))
    }
}
