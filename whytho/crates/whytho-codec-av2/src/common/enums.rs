//! Codec enumerations (block/transform sizes, prediction and partition modes, frame types).
//!
//! References: `avm/av2/common/{enums.h,common_data.h,blockd.h}`.

use core::fmt;

/// AV2 superblock dimension in luma samples.
pub const MAX_SB_SIZE: usize = 128;

/// Error returned when an integer is not a supported value for a codec enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidEnumValue {
    kind: &'static str,
    value: usize,
}

impl InvalidEnumValue {
    /// Name of the enum that rejected the value.
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    /// Rejected integer value.
    pub const fn value(self) -> usize {
        self.value
    }
}

impl fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} value {} is unsupported", self.kind, self.value)
    }
}

impl std::error::Error for InvalidEnumValue {}

macro_rules! codec_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident = $value:expr),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[repr(u8)]
        pub enum $name {
            $($variant = $value),+
        }

        impl $name {
            /// All supported variants in reference order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Return the reference codec's integer representation.
            pub const fn as_usize(self) -> usize {
                self as usize
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.as_usize()
            }
        }

        impl TryFrom<usize> for $name {
            type Error = InvalidEnumValue;

            fn try_from(value: usize) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(InvalidEnumValue {
                        kind: stringify!($name),
                        value,
                    }),
                }
            }
        }
    };
}

codec_enum! {
    /// Block sizes supported by the initial 128x128-superblock encoder.
    ///
    /// Discriminants preserve AVM's `BLOCK_SIZE` values. The 256-sized entries at values
    /// 16..=18 are intentionally unsupported, so the thin rectangular tail starts at 19.
    pub enum BlockSize {
        BLOCK_4X4 = 0,
        BLOCK_4X8 = 1,
        BLOCK_8X4 = 2,
        BLOCK_8X8 = 3,
        BLOCK_8X16 = 4,
        BLOCK_16X8 = 5,
        BLOCK_16X16 = 6,
        BLOCK_16X32 = 7,
        BLOCK_32X16 = 8,
        BLOCK_32X32 = 9,
        BLOCK_32X64 = 10,
        BLOCK_64X32 = 11,
        BLOCK_64X64 = 12,
        BLOCK_64X128 = 13,
        BLOCK_128X64 = 14,
        BLOCK_128X128 = 15,
        BLOCK_4X16 = 19,
        BLOCK_16X4 = 20,
        BLOCK_8X32 = 21,
        BLOCK_32X8 = 22,
        BLOCK_16X64 = 23,
        BLOCK_64X16 = 24,
        BLOCK_4X32 = 25,
        BLOCK_32X4 = 26,
        BLOCK_8X64 = 27,
        BLOCK_64X8 = 28,
    }
}

const BLOCK_TABLE_LEN: usize = 29;

const BLOCK_WIDTH: [u16; BLOCK_TABLE_LEN] = [
    4, 4, 8, 8, 8, 16, 16, 16, 32, 32, 32, 64, 64, 64, 128, 128, 0, 0, 0, 4, 16, 8, 32, 16, 64, 4,
    32, 8, 64,
];
const BLOCK_HEIGHT: [u16; BLOCK_TABLE_LEN] = [
    4, 8, 4, 8, 16, 8, 16, 32, 16, 32, 64, 32, 64, 128, 64, 128, 0, 0, 0, 16, 4, 32, 8, 64, 16, 32,
    4, 64, 8,
];
const BLOCK_WIDTH_LOG2: [u8; BLOCK_TABLE_LEN] = [
    2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 0, 0, 0, 2, 4, 3, 5, 4, 6, 2, 5, 3, 6,
];
const BLOCK_HEIGHT_LOG2: [u8; BLOCK_TABLE_LEN] = [
    2, 3, 2, 3, 4, 3, 4, 5, 4, 5, 6, 5, 6, 7, 6, 7, 0, 0, 0, 4, 2, 5, 3, 6, 4, 5, 2, 6, 3,
];

impl BlockSize {
    /// Width in samples.
    pub const fn width(self) -> usize {
        BLOCK_WIDTH[self.as_usize()] as usize
    }

    /// Height in samples.
    pub const fn height(self) -> usize {
        BLOCK_HEIGHT[self.as_usize()] as usize
    }

    /// Base-2 logarithm of the width in samples.
    pub const fn width_log2(self) -> u8 {
        BLOCK_WIDTH_LOG2[self.as_usize()]
    }

    /// Base-2 logarithm of the height in samples.
    pub const fn height_log2(self) -> u8 {
        BLOCK_HEIGHT_LOG2[self.as_usize()]
    }

    /// Whether width and height are equal.
    pub const fn is_square(self) -> bool {
        self.width() == self.height()
    }
}

codec_enum! {
    /// Transform block sizes.
    pub enum TxSize {
        TX_4X4 = 0,
        TX_8X8 = 1,
        TX_16X16 = 2,
        TX_32X32 = 3,
        TX_64X64 = 4,
        TX_4X8 = 5,
        TX_8X4 = 6,
        TX_8X16 = 7,
        TX_16X8 = 8,
        TX_16X32 = 9,
        TX_32X16 = 10,
        TX_32X64 = 11,
        TX_64X32 = 12,
        TX_4X16 = 13,
        TX_16X4 = 14,
        TX_8X32 = 15,
        TX_32X8 = 16,
        TX_16X64 = 17,
        TX_64X16 = 18,
        TX_4X32 = 19,
        TX_32X4 = 20,
        TX_8X64 = 21,
        TX_64X8 = 22,
        TX_4X64 = 23,
        TX_64X4 = 24,
    }
}

const TX_WIDTH: [u8; 25] = [
    4, 8, 16, 32, 64, 4, 8, 8, 16, 16, 32, 32, 64, 4, 16, 8, 32, 16, 64, 4, 32, 8, 64, 4, 64,
];
const TX_HEIGHT: [u8; 25] = [
    4, 8, 16, 32, 64, 8, 4, 16, 8, 32, 16, 64, 32, 16, 4, 32, 8, 64, 16, 32, 4, 64, 8, 64, 4,
];
const TX_WIDTH_LOG2: [u8; 25] = [
    2, 3, 4, 5, 6, 2, 3, 3, 4, 4, 5, 5, 6, 2, 4, 3, 5, 4, 6, 2, 5, 3, 6, 2, 6,
];
const TX_HEIGHT_LOG2: [u8; 25] = [
    2, 3, 4, 5, 6, 3, 2, 4, 3, 5, 4, 6, 5, 4, 2, 5, 3, 6, 4, 5, 2, 6, 3, 6, 2,
];
const TX_SQUARE_UP: [TxSize; 25] = [
    TxSize::TX_4X4,
    TxSize::TX_8X8,
    TxSize::TX_16X16,
    TxSize::TX_32X32,
    TxSize::TX_64X64,
    TxSize::TX_8X8,
    TxSize::TX_8X8,
    TxSize::TX_16X16,
    TxSize::TX_16X16,
    TxSize::TX_32X32,
    TxSize::TX_32X32,
    TxSize::TX_64X64,
    TxSize::TX_64X64,
    TxSize::TX_16X16,
    TxSize::TX_16X16,
    TxSize::TX_32X32,
    TxSize::TX_32X32,
    TxSize::TX_64X64,
    TxSize::TX_64X64,
    TxSize::TX_32X32,
    TxSize::TX_32X32,
    TxSize::TX_64X64,
    TxSize::TX_64X64,
    TxSize::TX_64X64,
    TxSize::TX_64X64,
];

impl TxSize {
    /// Width in samples.
    pub const fn width(self) -> usize {
        TX_WIDTH[self.as_usize()] as usize
    }

    /// Height in samples.
    pub const fn height(self) -> usize {
        TX_HEIGHT[self.as_usize()] as usize
    }

    /// Base-2 logarithm of the width in samples.
    pub const fn width_log2(self) -> u8 {
        TX_WIDTH_LOG2[self.as_usize()]
    }

    /// Base-2 logarithm of the height in samples.
    pub const fn height_log2(self) -> u8 {
        TX_HEIGHT_LOG2[self.as_usize()]
    }

    /// Smallest square transform containing this transform.
    pub const fn square_up(self) -> Self {
        TX_SQUARE_UP[self.as_usize()]
    }
}

codec_enum! {
    /// Separable two-dimensional transform types.
    pub enum TxType {
        DCT_DCT = 0,
        ADST_DCT = 1,
        DCT_ADST = 2,
        ADST_ADST = 3,
        FLIPADST_DCT = 4,
        DCT_FLIPADST = 5,
        FLIPADST_FLIPADST = 6,
        ADST_FLIPADST = 7,
        FLIPADST_ADST = 8,
        IDTX = 9,
        V_DCT = 10,
        H_DCT = 11,
        V_ADST = 12,
        H_ADST = 13,
        V_FLIPADST = 14,
        H_FLIPADST = 15,
    }
}

codec_enum! {
    /// Intra prediction modes.
    pub enum PredictionMode {
        DC_PRED = 0,
        V_PRED = 1,
        H_PRED = 2,
        D45_PRED = 3,
        D135_PRED = 4,
        D113_PRED = 5,
        D157_PRED = 6,
        D203_PRED = 7,
        D67_PRED = 8,
        SMOOTH_PRED = 9,
        SMOOTH_V_PRED = 10,
        SMOOTH_H_PRED = 11,
        PAETH_PRED = 12,
    }
}

codec_enum! {
    /// Recursive block partition types.
    pub enum PartitionType {
        PARTITION_NONE = 0,
        PARTITION_HORZ = 1,
        PARTITION_VERT = 2,
        PARTITION_HORZ_3 = 3,
        PARTITION_VERT_3 = 4,
        PARTITION_HORZ_4A = 5,
        PARTITION_HORZ_4B = 6,
        PARTITION_VERT_4A = 7,
        PARTITION_VERT_4B = 8,
        PARTITION_SPLIT = 9,
    }
}

const N: Option<BlockSize> = None;

#[rustfmt::skip]
const PARTITION_SUBSIZE: [[Option<BlockSize>; BLOCK_TABLE_LEN]; 10] = [
    [
        Some(BlockSize::BLOCK_4X4), Some(BlockSize::BLOCK_4X8), Some(BlockSize::BLOCK_8X4),
        Some(BlockSize::BLOCK_8X8), Some(BlockSize::BLOCK_8X16), Some(BlockSize::BLOCK_16X8),
        Some(BlockSize::BLOCK_16X16), Some(BlockSize::BLOCK_16X32), Some(BlockSize::BLOCK_32X16),
        Some(BlockSize::BLOCK_32X32), Some(BlockSize::BLOCK_32X64), Some(BlockSize::BLOCK_64X32),
        Some(BlockSize::BLOCK_64X64), Some(BlockSize::BLOCK_64X128), Some(BlockSize::BLOCK_128X64),
        Some(BlockSize::BLOCK_128X128), N, N, N,
        Some(BlockSize::BLOCK_4X16), Some(BlockSize::BLOCK_16X4), Some(BlockSize::BLOCK_8X32),
        Some(BlockSize::BLOCK_32X8), Some(BlockSize::BLOCK_16X64), Some(BlockSize::BLOCK_64X16),
        Some(BlockSize::BLOCK_4X32), Some(BlockSize::BLOCK_32X4), Some(BlockSize::BLOCK_8X64),
        Some(BlockSize::BLOCK_64X8),
    ],
    [
        N, Some(BlockSize::BLOCK_4X4), N, Some(BlockSize::BLOCK_8X4),
        Some(BlockSize::BLOCK_8X8), Some(BlockSize::BLOCK_16X4), Some(BlockSize::BLOCK_16X8),
        Some(BlockSize::BLOCK_16X16), Some(BlockSize::BLOCK_32X8), Some(BlockSize::BLOCK_32X16),
        Some(BlockSize::BLOCK_32X32), Some(BlockSize::BLOCK_64X16), Some(BlockSize::BLOCK_64X32),
        Some(BlockSize::BLOCK_64X64), N, Some(BlockSize::BLOCK_128X64), N, N, N,
        Some(BlockSize::BLOCK_4X8), N, Some(BlockSize::BLOCK_8X16), Some(BlockSize::BLOCK_32X4),
        Some(BlockSize::BLOCK_16X32), Some(BlockSize::BLOCK_64X8), Some(BlockSize::BLOCK_4X16),
        N, Some(BlockSize::BLOCK_8X32), N,
    ],
    [
        N, N, Some(BlockSize::BLOCK_4X4), Some(BlockSize::BLOCK_4X8),
        Some(BlockSize::BLOCK_4X16), Some(BlockSize::BLOCK_8X8), Some(BlockSize::BLOCK_8X16),
        Some(BlockSize::BLOCK_8X32), Some(BlockSize::BLOCK_16X16), Some(BlockSize::BLOCK_16X32),
        Some(BlockSize::BLOCK_16X64), Some(BlockSize::BLOCK_32X32), Some(BlockSize::BLOCK_32X64),
        N, Some(BlockSize::BLOCK_64X64), Some(BlockSize::BLOCK_64X128), N, N, N,
        N, Some(BlockSize::BLOCK_8X4), Some(BlockSize::BLOCK_4X32), Some(BlockSize::BLOCK_16X8),
        Some(BlockSize::BLOCK_8X64), Some(BlockSize::BLOCK_32X16), N, Some(BlockSize::BLOCK_16X4),
        N, Some(BlockSize::BLOCK_32X8),
    ],
    [
        N, N, N, N, Some(BlockSize::BLOCK_8X4), N, Some(BlockSize::BLOCK_16X4),
        Some(BlockSize::BLOCK_16X8), Some(BlockSize::BLOCK_32X4), Some(BlockSize::BLOCK_32X8),
        Some(BlockSize::BLOCK_32X16), Some(BlockSize::BLOCK_64X8), Some(BlockSize::BLOCK_64X16),
        N, N, N, N, N, N, N, N, Some(BlockSize::BLOCK_8X8), N,
        Some(BlockSize::BLOCK_16X16), N, N, N, N, N,
    ],
    [
        N, N, N, N, N, Some(BlockSize::BLOCK_4X8), Some(BlockSize::BLOCK_4X16),
        Some(BlockSize::BLOCK_4X32), Some(BlockSize::BLOCK_8X16), Some(BlockSize::BLOCK_8X32),
        Some(BlockSize::BLOCK_8X64), Some(BlockSize::BLOCK_16X32), Some(BlockSize::BLOCK_16X64),
        N, N, N, N, N, N, N, N, N, Some(BlockSize::BLOCK_8X8), N,
        Some(BlockSize::BLOCK_16X16), N, N, N, N,
    ],
    [
        N, N, N, N, N, N, N, Some(BlockSize::BLOCK_16X4), N,
        Some(BlockSize::BLOCK_32X4), Some(BlockSize::BLOCK_32X8), N,
        Some(BlockSize::BLOCK_64X8), N, N, N, N, N, N, N, N,
        Some(BlockSize::BLOCK_8X4), N, Some(BlockSize::BLOCK_16X8), N, N, N, N, N,
    ],
    [
        N, N, N, N, N, N, N, Some(BlockSize::BLOCK_16X4), N,
        Some(BlockSize::BLOCK_32X4), Some(BlockSize::BLOCK_32X8), N,
        Some(BlockSize::BLOCK_64X8), N, N, N, N, N, N, N, N,
        Some(BlockSize::BLOCK_8X4), N, Some(BlockSize::BLOCK_16X8), N, N, N, N, N,
    ],
    [
        N, N, N, N, N, N, N, N, Some(BlockSize::BLOCK_4X16),
        Some(BlockSize::BLOCK_4X32), N, Some(BlockSize::BLOCK_8X32),
        Some(BlockSize::BLOCK_8X64), N, N, N, N, N, N, N, N, N,
        Some(BlockSize::BLOCK_4X8), N, Some(BlockSize::BLOCK_8X16), N, N, N, N,
    ],
    [
        N, N, N, N, N, N, N, N, Some(BlockSize::BLOCK_4X16),
        Some(BlockSize::BLOCK_4X32), N, Some(BlockSize::BLOCK_8X32),
        Some(BlockSize::BLOCK_8X64), N, N, N, N, N, N, N, N, N,
        Some(BlockSize::BLOCK_4X8), N, Some(BlockSize::BLOCK_8X16), N, N, N, N,
    ],
    [
        N, N, N, Some(BlockSize::BLOCK_4X4), N, N, Some(BlockSize::BLOCK_8X8),
        N, N, Some(BlockSize::BLOCK_16X16), N, N, Some(BlockSize::BLOCK_32X32),
        N, N, Some(BlockSize::BLOCK_64X64), N, N, N, N, N, N, N, N, N, N, N, N, N,
    ],
];

impl PartitionType {
    /// Size of the first child block produced by this partition.
    ///
    /// Returns `None` when the partition is invalid for the supplied block or would require
    /// a block size outside this crate's 128x128-supported subset.
    pub const fn subsize(self, block: BlockSize) -> Option<BlockSize> {
        PARTITION_SUBSIZE[self.as_usize()][block.as_usize()]
    }
}

codec_enum! {
    /// Coded frame type.
    pub enum FrameType {
        KEY_FRAME = 0,
        INTER_FRAME = 1,
        INTRA_ONLY_FRAME = 2,
        S_FRAME = 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_roundtrip<T>(values: &[T])
    where
        T: Copy + Into<usize> + TryFrom<usize, Error = InvalidEnumValue> + PartialEq + fmt::Debug,
    {
        for &value in values {
            let raw = value.into();
            assert_eq!(T::try_from(raw), Ok(value));
        }
    }

    #[test]
    fn enums_roundtrip_supported_values() {
        assert_roundtrip(BlockSize::ALL);
        assert_roundtrip(TxSize::ALL);
        assert_roundtrip(TxType::ALL);
        assert_roundtrip(PredictionMode::ALL);
        assert_roundtrip(PartitionType::ALL);
        assert_roundtrip(FrameType::ALL);
    }

    #[test]
    fn unsupported_values_are_rejected() {
        for value in 16..=18 {
            assert!(BlockSize::try_from(value).is_err());
        }
        assert!(BlockSize::try_from(255).is_err());
        assert!(TxSize::try_from(25).is_err());
        assert!(PredictionMode::try_from(13).is_err());
    }

    #[test]
    fn block_size_helpers_match_reference_tables() {
        assert_eq!(
            (
                BlockSize::BLOCK_4X4.width(),
                BlockSize::BLOCK_4X4.height(),
                BlockSize::BLOCK_4X4.width_log2(),
                BlockSize::BLOCK_4X4.height_log2(),
            ),
            (4, 4, 2, 2)
        );
        assert_eq!(
            (
                BlockSize::BLOCK_64X128.width(),
                BlockSize::BLOCK_64X128.height(),
            ),
            (64, 128)
        );
        assert_eq!(
            (
                BlockSize::BLOCK_4X32.width(),
                BlockSize::BLOCK_4X32.height(),
            ),
            (4, 32)
        );
        assert!(BlockSize::BLOCK_128X128.is_square());
        assert!(!BlockSize::BLOCK_64X128.is_square());
    }

    #[test]
    fn tx_size_helpers_match_reference_tables() {
        assert_eq!((TxSize::TX_4X4.width(), TxSize::TX_4X4.height()), (4, 4));
        assert_eq!(
            (TxSize::TX_32X64.width(), TxSize::TX_32X64.height()),
            (32, 64)
        );
        assert_eq!(
            (
                TxSize::TX_64X4.width(),
                TxSize::TX_64X4.height(),
                TxSize::TX_64X4.width_log2(),
                TxSize::TX_64X4.height_log2(),
            ),
            (64, 4, 6, 2)
        );
        assert_eq!(TxSize::TX_4X8.square_up(), TxSize::TX_8X8);
        assert_eq!(TxSize::TX_4X64.square_up(), TxSize::TX_64X64);
    }

    #[test]
    fn partition_subsizes_match_reference_table() {
        assert_eq!(
            PartitionType::PARTITION_NONE.subsize(BlockSize::BLOCK_32X64),
            Some(BlockSize::BLOCK_32X64)
        );
        assert_eq!(
            PartitionType::PARTITION_SPLIT.subsize(BlockSize::BLOCK_128X128),
            Some(BlockSize::BLOCK_64X64)
        );
        assert_eq!(
            PartitionType::PARTITION_HORZ_4A.subsize(BlockSize::BLOCK_64X64),
            Some(BlockSize::BLOCK_64X8)
        );
        assert_eq!(
            PartitionType::PARTITION_SPLIT.subsize(BlockSize::BLOCK_32X64),
            None
        );
    }
}
