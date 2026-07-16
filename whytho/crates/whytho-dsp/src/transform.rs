//! Forward and inverse transforms (DCT/Hadamard, sizes 2..4).
//!
//! These are the shared integer transform kernels used by H.264, AV1, and AV2 codecs.
//! The scalar implementations are the correctness source of truth; SIMD variants
//! (NEON, AVX2) will be added as optimization paths.
//!
//! Reference: H.264 spec sections 8.5.1 (4x4 DCT), 8.5.5 (Hadamard DC).

/// Forward 4x4 integer DCT (H.264 style).
///
/// Core matrix: [[1,1,1,1],[2,1,-1,-2],[1,-1,-1,1],[1,-2,2,-1]]
/// This is the non-normalized version (no >> 1). The quantizer MF values
/// are calibrated for this output.
pub fn forward_dct_4x4(block: &[[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    for row in 0..4 {
        let a = block[row][0] as i32 + block[row][3] as i32;
        let b = block[row][1] as i32 + block[row][2] as i32;
        let c = block[row][0] as i32 - block[row][3] as i32;
        let d = block[row][1] as i32 - block[row][2] as i32;
        tmp[row][0] = a + b;
        tmp[row][1] = 2 * c + d;
        tmp[row][2] = a - b;
        tmp[row][3] = c - 2 * d;
    }

    for col in 0..4 {
        let a = tmp[0][col] + tmp[3][col];
        let b = tmp[1][col] + tmp[2][col];
        let c = tmp[0][col] - tmp[3][col];
        let d = tmp[1][col] - tmp[2][col];
        result[0][col] = (a + b) as i16;
        result[1][col] = (2 * c + d) as i16;
        result[2][col] = (a - b) as i16;
        result[3][col] = (c - 2 * d) as i16;
    }

    result
}

/// Inverse 4x4 integer DCT (H.264 style).
///
/// Applies >> 6 normalization (>> 1 per dimension) to match the forward transform's gain.
pub fn inverse_dct_4x4(block: &[[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    for row in 0..4 {
        let a = block[row][0] as i32 + block[row][2] as i32;
        let b = block[row][0] as i32 - block[row][2] as i32;
        let c = block[row][1] as i32 + block[row][3] as i32;
        let d = block[row][1] as i32 - block[row][3] as i32;
        tmp[row][0] = a + c;
        tmp[row][1] = b + d;
        tmp[row][2] = b - d;
        tmp[row][3] = a - c;
    }

    for col in 0..4 {
        let a = tmp[0][col] + tmp[2][col];
        let b = tmp[0][col] - tmp[2][col];
        let c = tmp[1][col] + tmp[3][col];
        let d = tmp[1][col] - tmp[3][col];
        result[0][col] = ((a + c + 32) >> 6) as i16;
        result[1][col] = ((b + d + 32) >> 6) as i16;
        result[2][col] = ((b - d + 32) >> 6) as i16;
        result[3][col] = ((a - c + 32) >> 6) as i16;
    }

    result
}

/// Forward 4x4 Hadamard transform.
///
/// Used for I16x16 luma DC coefficients (H.264 spec 8.5.5).
/// No normalization — the quantizer compensates for the 16x gain.
pub fn forward_hadamard_4x4(block: &[[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    for row in 0..4 {
        let a = block[row][0] as i32 + block[row][3] as i32;
        let b = block[row][1] as i32 + block[row][2] as i32;
        let c = block[row][0] as i32 - block[row][3] as i32;
        let d = block[row][1] as i32 - block[row][2] as i32;
        tmp[row][0] = a + b;
        tmp[row][1] = c + d;
        tmp[row][2] = a - b;
        tmp[row][3] = c - d;
    }

    for col in 0..4 {
        let a = tmp[0][col] + tmp[3][col];
        let b = tmp[1][col] + tmp[2][col];
        let c = tmp[0][col] - tmp[3][col];
        let d = tmp[1][col] - tmp[2][col];
        result[0][col] = (a + b) as i16;
        result[1][col] = (c + d) as i16;
        result[2][col] = (a - b) as i16;
        result[3][col] = (c - d) as i16;
    }

    result
}

/// Forward 2x2 Hadamard transform.
///
/// Used for chroma DC coefficients (H.264 spec 8.5.6).
pub fn forward_hadamard_2x2(block: &[[i16; 2]; 2]) -> [[i16; 2]; 2] {
    let a = block[0][0] as i32 + block[0][1] as i32;
    let b = block[0][0] as i32 - block[0][1] as i32;
    let c = block[1][0] as i32 + block[1][1] as i32;
    let d = block[1][0] as i32 - block[1][1] as i32;
    [
        [(a + c) as i16, (b + d) as i16],
        [(a - c) as i16, (b - d) as i16],
    ]
}

/// Quantize a 4x4 block using H.264-style scaling.
///
/// MF values are the quantizer scaling factors indexed by position category
/// (pc = (row & 1) + (col & 1)) and qp % 6.
///
/// Returns the quantized coefficients.
pub fn quantize_4x4(block: &[[i16; 4]; 4], qp: i8) -> [[i16; 4]; 4] {
    let rem = (qp % 6) as usize;
    let div = (qp / 6) as i32;

    const MF: [[i32; 6]; 3] = [
        [13107, 11916, 10082, 9362, 8192, 7282],
        [8066, 7490, 6554, 5825, 5243, 4559],
        [5243, 4660, 4194, 3647, 3355, 2893],
    ];

    let mut result = [[0i16; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            let level = block[row][col] as i32;
            let pc = (row & 1) + (col & 1);
            let mf = MF[pc][rem];
            if div == 0 {
                result[row][col] = ((level * mf + (1 << 14)) >> 15) as i16;
            } else {
                result[row][col] = ((level * mf + (1 << (14 + div))) >> (15 + div)) as i16;
            }
        }
    }
    result
}

/// Dequantize a 4x4 block using H.264-style scaling.
///
/// V values are the dequantizer scaling factors indexed by position category
/// and qp % 6.
pub fn dequantize_4x4(block: &[[i16; 4]; 4], qp: i8) -> [[i16; 4]; 4] {
    let rem = (qp % 6) as usize;
    let div = (qp / 6) as i32;

    const V: [[i32; 6]; 3] = [
        [10, 13, 16, 18, 20, 23],
        [13, 14, 18, 20, 23, 25],
        [16, 18, 20, 23, 25, 28],
    ];

    let mut result = [[0i16; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            let level = block[row][col] as i32;
            let pc = (row & 1) + (col & 1);
            result[row][col] = ((level * V[pc][rem]) << div) as i16;
        }
    }
    result
}

/// Zigzag scan order for 4x4 blocks (H.264 spec Figure 8-5).
pub const ZIGZAG_4X4: [(usize, usize); 16] = [
    (0, 0),
    (0, 1),
    (1, 0),
    (2, 0),
    (1, 1),
    (0, 2),
    (0, 3),
    (1, 2),
    (2, 1),
    (3, 0),
    (3, 1),
    (2, 2),
    (1, 3),
    (2, 3),
    (3, 2),
    (3, 3),
];

/// Apply zigzag scan to a 4x4 block (raster → zigzag order).
pub fn zigzag_4x4(block: &[[i16; 4]; 4]) -> [i16; 16] {
    let mut result = [0i16; 16];
    for (i, &(r, c)) in ZIGZAG_4X4.iter().enumerate() {
        result[i] = block[r][c];
    }
    result
}

/// Inverse zigzag scan (zigzag order → raster).
pub fn unzigzag_4x4(coeffs: &[i16; 16]) -> [[i16; 4]; 4] {
    let mut result = [[0i16; 4]; 4];
    for (i, &(r, c)) in ZIGZAG_4X4.iter().enumerate() {
        result[r][c] = coeffs[i];
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dct_zero_block() {
        let zero = [[0i16; 4]; 4];
        assert_eq!(forward_dct_4x4(&zero), zero);
    }

    #[test]
    fn dct_constant_is_dc() {
        let c = [[128i16; 4]; 4];
        let result = forward_dct_4x4(&c);
        assert_eq!(result[0][0], 128 * 16);
        for r in 0..4 {
            for col in 0..4 {
                if r != 0 || col != 0 {
                    assert_eq!(result[r][col], 0);
                }
            }
        }
    }

    #[test]
    fn dct_inverse_roundtrip() {
        // Forward DCT has no normalization, inverse has >> 6.
        // Full roundtrip: forward → inverse gives gain = 16/64 = 1/4 for DC.
        // With quantize/dequantize in between, the gains cancel.
        let original = [[128i16; 4]; 4];
        let transformed = forward_dct_4x4(&original);
        let reconstructed = inverse_dct_4x4(&transformed);
        // For a constant block: forward DC = 128*16 = 2048, inverse = (2048+32)>>6 = 32
        // The 4x loss is expected without quantizer compensation.
        assert_eq!(transformed[0][0], 2048);
        assert_eq!(reconstructed[0][0], 32);
    }

    #[test]
    fn dct_quantize_dequantize_roundtrip() {
        // Full pipeline: forward DCT → quantize → dequantize → inverse DCT
        let original = [[128i16; 4]; 4];
        let dct = forward_dct_4x4(&original);
        let quantized = quantize_4x4(&dct, 0);
        let dequantized = dequantize_4x4(&quantized, 0);
        let reconstructed = inverse_dct_4x4(&dequantized);
        // For a constant block at qp=0, the roundtrip should be nearly lossless
        for r in 0..4 {
            for c in 0..4 {
                let err = (original[r][c] as i32 - reconstructed[r][c] as i32).unsigned_abs();
                assert!(err <= 2, "error at ({r},{c}): {err}");
            }
        }
    }

    #[test]
    fn hadamard_4x4_constant() {
        let c = [[10i16; 4]; 4];
        let result = forward_hadamard_4x4(&c);
        assert_eq!(result[0][0], 160); // 16 * 10
        for r in 0..4 {
            for col in 0..4 {
                if r != 0 || col != 0 {
                    assert_eq!(result[r][col], 0);
                }
            }
        }
    }

    #[test]
    fn hadamard_2x2_known() {
        assert_eq!(forward_hadamard_2x2(&[[1, 0], [0, 0]]), [[1, 1], [1, 1]]);
    }

    #[test]
    fn quantize_dequantize_roundtrip_low_qp() {
        // Full pipeline test: DCT → quantize → dequantize → inverse DCT.
        // The quantizer alone has 4x gain (MF*V/2^15 ≈ 4), compensated by
        // the DCT/inverse DCT pair (16x forward / 64x inverse = 1/4).
        let pixel_val = 128i16;
        let original = [[pixel_val; 4]; 4];
        let dct = forward_dct_4x4(&original);
        let quantized = quantize_4x4(&dct, 0);
        let dequantized = dequantize_4x4(&quantized, 0);
        let reconstructed = inverse_dct_4x4(&dequantized);
        // At qp=0, the full pipeline should be nearly lossless
        for r in 0..4 {
            for c in 0..4 {
                let err = (original[r][c] as i32 - reconstructed[r][c] as i32).unsigned_abs();
                assert!(err <= 2, "error at ({r},{c}): {err}");
            }
        }
    }

    #[test]
    fn zigzag_unzigzag_roundtrip() {
        let block = [
            [1, 2, 3, 4],
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ];
        let scanned = zigzag_4x4(&block);
        let restored = unzigzag_4x4(&scanned);
        assert_eq!(restored, block);
    }

    #[test]
    fn zigzag_order() {
        let block = [
            [0i16, 1, 5, 6],
            [2, 4, 7, 12],
            [3, 8, 11, 13],
            [9, 10, 14, 15],
        ];
        let scanned = zigzag_4x4(&block);
        for (i, &val) in scanned.iter().enumerate() {
            assert_eq!(val, i as i16);
        }
    }
}
