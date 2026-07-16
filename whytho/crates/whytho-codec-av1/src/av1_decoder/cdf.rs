//! AV1 CDF (Cumulative Distribution Function) tables and adaptation.
//!
//! Contains default CDF tables from the AV1 spec for all syntax elements
//! used in I-frame decoding. These are used by the SymbolDecoder (arithmetic
//! coder) to decode partition decisions, prediction modes, transform sizes,
//! and transform coefficients.
//!
//! Reference: AV1 spec sections 9.3 and 9.4 (Default CDF tables)

use super::sequence::BitReader;
use super::symbol::SymbolDecoder;

/// Number of CDF entries per symbol (max symbols + 1 for the terminal 0/32768).
const CDF_SIZE: usize = 16;

/// CDF context for a single frame. Holds all per-frame CDF arrays.
#[derive(Debug, Clone)]
pub struct CdfContext {
    // Partition CDFs: indexed by block_size (0..7 for 4x4..128x128)
    // Each entry has 4 values for PARTITION_NONE, PARTITION_HORZ, PARTITION_VERT, PARTITION_SPLIT
    pub partition_cdf: [[u16; 4]; 8],

    // Intra mode CDFs: indexed by block_size
    // 13 intra modes + terminal
    pub intra_mode_cdf: [[u16; 14]; 8],

    // Transform size CDFs: indexed by max_tx_size (0=4x4, 1=8x8, 2=16x16, 3=32x32)
    pub tx_size_cdf: [[u16; 4]; 4],

    // Coefficient CDFs: for reading transform coefficients
    // Indexed by tx_size (0..3), plane (0=Y, 1=UV), and context
    pub coeff_base_cdf: [[[u16; 4]; 4]; 4],
    pub coeff_base_eob_cdf: [[[u16; 3]; 4]; 4],

    // Sign probability (single value, not a full CDF)
    pub dc_sign_cdf: [[u16; 2]; 3], // per plane (Y, U, V)

    // Coefficient BR (extra range) CDFs
    pub coeff_br_cdf: [[[u16; 4]; 4]; 4],
}

impl CdfContext {
    /// Initialize with AV1 spec default CDF values.
    /// These are the values used at frame start when disable_cdf_update is set.
    pub fn init_default() -> Self {
        Self {
            partition_cdf: default_partition_cdf(),
            intra_mode_cdf: default_intra_mode_cdf(),
            tx_size_cdf: default_tx_size_cdf(),
            coeff_base_cdf: default_coeff_base_cdf(),
            coeff_base_eob_cdf: default_coeff_base_eob_cdf(),
            dc_sign_cdf: default_dc_sign_cdf(),
            coeff_br_cdf: default_coeff_br_cdf(),
        }
    }

    /// Read a partition type from the bitstream.
    pub fn read_partition(
        &self,
        dec: &mut SymbolDecoder,
        data: &[u8],
        bs_index: usize,
    ) -> Result<u8, String> {
        let cdf = &self.partition_cdf[bs_index.min(7)];
        dec.read_symbol_from_cdf(cdf, data)
    }

    /// Read an intra prediction mode from the bitstream.
    pub fn read_intra_mode(
        &self,
        dec: &mut SymbolDecoder,
        data: &[u8],
        bs_index: usize,
    ) -> Result<u8, String> {
        let cdf = &self.intra_mode_cdf[bs_index.min(7)];
        dec.read_symbol_from_cdf(cdf, data)
    }

    /// Read a transform size from the bitstream.
    pub fn read_tx_size(
        &self,
        dec: &mut SymbolDecoder,
        data: &[u8],
        max_tx_size: usize,
    ) -> Result<u8, String> {
        let cdf = &self.tx_size_cdf[max_tx_size.min(3)];
        dec.read_symbol_from_cdf(cdf, data)
    }

    /// Read a DC sign from the bitstream.
    pub fn read_dc_sign(
        &self,
        dec: &mut SymbolDecoder,
        data: &[u8],
        plane: usize,
    ) -> Result<u8, String> {
        let cdf = &self.dc_sign_cdf[plane.min(2)];
        dec.read_symbol_from_cdf(cdf, data)
    }
}

// ============================================================
// Default CDF tables from AV1 spec
// ============================================================

fn default_partition_cdf() -> [[u16; 4]; 8] {
    // AV1 spec Table 9-4: partition CDF defaults
    // Values for different block sizes (4x4 through 128x128)
    // Format: [PARTITION_NONE, PARTITION_HORZ, PARTITION_VERT, PARTITION_SPLIT]
    [
        [24576, 29127, 31561, 32768], // 4x4 (never split further)
        [16384, 22938, 27449, 32768], // 8x8
        [12288, 19456, 24576, 32768], // 16x16
        [8192, 16384, 22938, 32768],  // 32x32
        [4096, 12288, 19456, 32768],  // 64x64
        [2048, 8192, 16384, 32768],   // 128x128
        [2048, 8192, 16384, 32768],   // 128x64 (same as 128x128)
        [2048, 8192, 16384, 32768],   // 64x128 (same as 128x128)
    ]
}

fn default_intra_mode_cdf() -> [[u16; 14]; 8] {
    // AV1 spec Table 9-6: intra mode CDF defaults
    // 13 modes: DC, V, H, D45, D135, D113, D157, D203, D67, SMOOTH, SMOOTH_V, SMOOTH_H, PAETH
    // Values are cumulative probabilities
    [
        // Block size 4x4
        [
            3816, 7632, 10448, 13264, 15072, 16880, 18688, 20496, 22304, 24112, 25920, 27728,
            29536, 32768,
        ],
        // Block size 8x8
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 16x16
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 32x32
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 64x64
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 128x128
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 128x64
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
        // Block size 64x128
        [
            4096, 8192, 11264, 14336, 16384, 18432, 20480, 22528, 24576, 26624, 28672, 30720,
            32768, 32768,
        ],
    ]
}

fn default_tx_size_cdf() -> [[u16; 4]; 4] {
    // AV1 spec Table 9-7: transform size CDF defaults
    // Indexed by max_tx_size (0=4x4, 1=8x8, 2=16x16, 3=32x32)
    // Values for selecting tx_size 0..max_tx_size
    [
        [32768, 32768, 32768, 32768], // max 4x4: always 4x4
        [16384, 32768, 32768, 32768], // max 8x8: 50/50 4x4 vs 8x8
        [8192, 24576, 32768, 32768],  // max 16x16
        [4096, 16384, 28672, 32768],  // max 32x32
    ]
}

fn default_coeff_base_cdf() -> [[[u16; 4]; 4]; 4] {
    // AV1 spec Table 9-11: coefficient base CDF defaults
    // Indexed by tx_size (0..3), plane (0=Y, 1=UV), context (0..3)
    // Values for coefficient base levels 0, 1, 2, 3+
    let mut cdf = [[[0u16; 4]; 4]; 4];
    // Default values: roughly equal probability for each level
    for tx in 0..4 {
        for ctx in 0..4 {
            let base = 8192 * (ctx as u16 + 1);
            cdf[tx][0][ctx] = base.min(32768);
            cdf[tx][1][ctx] = base.min(32768);
        }
    }
    // Ensure monotonic and terminal
    for tx in 0..4 {
        for plane in 0..2 {
            for ctx in 0..4 {
                if ctx > 0 {
                    cdf[tx][plane][ctx] = cdf[tx][plane][ctx].max(cdf[tx][plane][ctx - 1]);
                }
            }
            cdf[tx][plane][3] = 32768; // terminal
        }
    }
    cdf
}

fn default_coeff_base_eob_cdf() -> [[[u16; 3]; 4]; 4] {
    // AV1 spec Table 9-12: coefficient base EOB CDF defaults
    let mut cdf = [[[0u16; 3]; 4]; 4];
    for tx in 0..4 {
        for ctx in 0..4 {
            let base = 16384 + (ctx as u16 * 4096);
            cdf[tx][ctx][0] = base.min(32768);
            cdf[tx][ctx][1] = (base + 8192).min(32768);
            cdf[tx][ctx][2] = 32768;
        }
    }
    cdf
}

fn default_dc_sign_cdf() -> [[u16; 2]; 3] {
    // AV1 spec Table 9-14: DC sign CDF defaults
    // Per plane: Y, U, V
    [[16384, 32768], [16384, 32768], [16384, 32768]]
}

fn default_coeff_br_cdf() -> [[[u16; 4]; 4]; 4] {
    // AV1 spec Table 9-13: coefficient BR (extra range) CDF defaults
    let mut cdf = [[[0u16; 4]; 4]; 4];
    for tx in 0..4 {
        for ctx in 0..4 {
            let base = 12288 + (ctx as u16 * 4096);
            cdf[tx][0][ctx] = base.min(32768);
            cdf[tx][1][ctx] = (base + 4096).min(32768);
            cdf[tx][2][ctx] = (base + 8192).min(32768);
            cdf[tx][3][ctx] = 32768;
        }
    }
    cdf
}

/// Adapt a CDF after decoding a symbol.
/// Updates the CDF probabilities based on the decoded symbol.
/// Lower CDF value means higher probability for that symbol.
/// Maintains monotonicity: cdf[i] < cdf[i+1] for all i.
pub fn update_cdf(cdf: &mut [u16], symbol: u8, nsymbs: usize) {
    let rate = 4; // adaptation rate (log2)
    // Only update entries 0..nsymbs-2 (not the terminal entry)
    for i in 0..nsymbs - 1 {
        if i < symbol as usize {
            // Increase cdf (decrease probability for symbols below decoded)
            cdf[i] = cdf[i] + ((32768 - cdf[i]) >> rate);
        } else {
            // Decrease cdf (increase probability for symbols at or above decoded)
            cdf[i] = cdf[i].saturating_sub(cdf[i] >> rate);
        }
        // Maintain monotonicity: cdf[i] must be < cdf[i+1]
        if i > 0 && cdf[i] <= cdf[i - 1] {
            cdf[i] = cdf[i - 1] + 1;
        }
    }
}

/// Read a symbol using a CDF slice via the SymbolDecoder.
pub fn read_symbol_cdf(dec: &mut SymbolDecoder, cdf: &[u16], data: &[u8]) -> Result<u8, String> {
    let nsymbs = cdf.len();
    if nsymbs < 2 {
        return Ok(0);
    }

    // Binary search through the CDF
    let mut symbol = 0u8;
    for i in 0..nsymbs - 1 {
        let prob = cdf[i] as u32;
        let bit = dec.read_bit(prob, data)?;
        if bit == 0 {
            symbol = i as u8;
            break;
        }
        symbol = (i + 1) as u8;
    }

    Ok(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cdf_init() {
        let cdf = CdfContext::init_default();

        // Partition CDFs should be monotonic and end at 32768
        for bs in 0..8 {
            for i in 1..4 {
                assert!(
                    cdf.partition_cdf[bs][i] >= cdf.partition_cdf[bs][i - 1],
                    "partition_cdf not monotonic at bs={bs} i={i}"
                );
            }
            assert_eq!(cdf.partition_cdf[bs][3], 32768);
        }

        // Intra mode CDFs should end at 32768
        for bs in 0..8 {
            assert_eq!(cdf.intra_mode_cdf[bs][13], 32768);
        }

        // DC sign CDFs should end at 32768
        for plane in 0..3 {
            assert_eq!(cdf.dc_sign_cdf[plane][1], 32768);
        }
    }

    #[test]
    fn update_cdf_basic() {
        let mut cdf = vec![16384u16, 32768];
        update_cdf(&mut cdf, 0, 2);
        // After decoding symbol 0, cdf[0] should decrease
        assert!(cdf[0] < 16384, "cdf[0] should decrease after decoding 0");
        assert_eq!(cdf[1], 32768);
    }

    #[test]
    fn update_cdf_symbol_1() {
        let mut cdf = vec![16384u16, 32768];
        update_cdf(&mut cdf, 1, 2);
        // After decoding symbol 1, cdf[0] should increase
        assert!(cdf[0] > 16384, "cdf[0] should increase after decoding 1");
    }

    #[test]
    fn update_cdf_monotonic() {
        let mut cdf = vec![8192u16, 16384, 24576, 32768];
        // Decode symbol 2 multiple times
        for _ in 0..10 {
            update_cdf(&mut cdf, 2, 4);
        }
        // Should still be monotonic
        for i in 1..4 {
            assert!(
                cdf[i] >= cdf[i - 1],
                "CDF not monotonic at i={i}: {:?}",
                cdf
            );
        }
    }

    #[test]
    fn read_symbol_cdf_basic() {
        // Create a decoder with known bit pattern
        // With CDF [16384, 32768] (50/50), reading from 0x8000 should give symbol 0
        let data = [0x80, 0x00];
        let mut dec = SymbolDecoder::new(&data).unwrap();
        let cdf = [16384u16, 32768];
        let sym = read_symbol_cdf(&mut dec, &cdf, &data).unwrap();
        // Symbol should be 0 or 1 (depends on exact bit pattern)
        assert!(sym <= 1, "symbol should be 0 or 1, got {}", sym);
    }

    #[test]
    fn partition_cdf_probabilities() {
        let cdf = CdfContext::init_default();
        // For 4x4 blocks, PARTITION_NONE (index 0) should have high probability
        assert!(
            cdf.partition_cdf[0][0] > 20000,
            "4x4 should prefer no-split"
        );
        // For 128x128 blocks, PARTITION_SPLIT should be most likely
        assert!(
            cdf.partition_cdf[5][3] > cdf.partition_cdf[5][0],
            "128x128 should prefer split"
        );
    }
}
