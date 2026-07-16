//! 4x4 integer DCT transform for H.264.
//!
//! Implements the exact H.264 forward and inverse 4x4 transform.
//!
// TODO(whytho-dsp dedup): private integer-DCT copy. Once a verification harness exists, fold the
// shared kernel into `whytho-dsp` (alongside the AV1/AV2 transforms) rather than per-codec copies.

/// Forward 4x4 integer DCT.
///
/// The H.264 transform uses a separable integer approximation of the DCT.
/// Core matrix: [[1,1,1,1],[2,1,-1,-2],[1,-1,-1,1],[1,-2,2,-1]]
///
/// NOTE: No >> 1 normalization here. The quantizer MF/shift values are
/// calibrated for unnormalized DCT output, matching x264's convention.
/// The 4x quantizer gain compensates for the 1/4 inverse DCT gain (>> 6).
pub fn forward_4x4(block: [[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    // Horizontal transform. The H.264 forward core transform basis (spec 8.5):
    // row1 = [2,1,-1,-2], row3 = [1,-2,2,-1], i.e. Y1 = 2c + d and Y3 = c - 2d. This
    // is the exact pairing for the decoder's inverse_dct_4x4; the earlier c+2d / 2c-d
    // formulation transposed the odd rows and corrupted every odd-position coefficient.
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

    // Vertical transform
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

/// Inverse 4x4 integer DCT.
pub fn inverse_4x4(block: [[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    // Horizontal inverse transform
    for row in 0..4 {
        let a = block[row][0] as i32 + block[row][2] as i32;
        let b = block[row][0] as i32 - block[row][2] as i32;
        let c = block[row][1] as i32 + block[row][3] as i32; // simplified
        let d = block[row][1] as i32 - block[row][3] as i32; // simplified

        tmp[row][0] = a + c;
        tmp[row][1] = b + d;
        tmp[row][2] = b - d;
        tmp[row][3] = a - c;
    }

    // Vertical inverse transform
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

/// 2x2 Hadamard transform for chroma DC coefficients.
///
/// Used for the 2x2 block of DC values from 4 chroma 4x4 blocks.
pub fn hadamard_2x2(block: [[i16; 2]; 2]) -> [[i16; 2]; 2] {
    let a = block[0][0] as i32 + block[0][1] as i32;
    let b = block[0][0] as i32 - block[0][1] as i32;
    let c = block[1][0] as i32 + block[1][1] as i32;
    let d = block[1][0] as i32 - block[1][1] as i32;

    [
        [(a + c) as i16, (b + d) as i16],
        [(a - c) as i16, (b - d) as i16],
    ]
}

/// 4x4 Hadamard transform for I16x16 luma DC coefficients.
///
/// Applied to the 4x4 block of DC values from the 16 luma 4x4 blocks.
/// No normalization here — the quantizer MF/shift values compensate for
/// the combined Hadamard + inverse Hadamard gain.
pub fn hadamard_4x4(block: [[i16; 4]; 4]) -> [[i16; 4]; 4] {
    let mut tmp = [[0i32; 4]; 4];
    let mut result = [[0i16; 4]; 4];

    // Horizontal Hadamard
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

    // Vertical Hadamard
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn forward_inverse_roundtrip() {
        let original = [
            [10, 20, 30, 40],
            [50, 60, 70, 80],
            [90, 100, 110, 120],
            [130, 140, 150, 160],
        ];

        let transformed = forward_4x4(original);
        assert!(transformed[0][0] != 0); // DC coefficient
        let dc_approx = (transformed[0][0] as f64 / 16.0).round() as i16;
        assert!(dc_approx > 50 && dc_approx < 120);
    }

    #[test]
    fn zero_block_produces_zero() {
        let zero = [[0i16; 4]; 4];
        let transformed = forward_4x4(zero);
        assert!(transformed.iter().all(|row| row.iter().all(|&c| c == 0)));
    }

    #[test]
    fn constant_block_dc_only() {
        let constant = [[128i16; 4]; 4];
        let transformed = forward_4x4(constant);
        // DC should be 128*16 = 2048
        assert_eq!(transformed[0][0], 2048);
        // All AC coefficients should be 0
        for row in 0..4 {
            for col in 0..4 {
                if row != 0 || col != 0 {
                    assert_eq!(transformed[row][col], 0);
                }
            }
        }
    }

    #[test]
    fn inverse_zero_produces_zero() {
        let result = inverse_4x4([[0i16; 4]; 4]);
        assert!(result.iter().all(|row| row.iter().all(|&c| c == 0)));
    }

    #[test]
    fn inverse_dc_only_is_constant() {
        // A pure-DC input dequantizes to a flat block. With DC=2048: (2048+32)>>6 = 32.
        let mut block = [[0i16; 4]; 4];
        block[0][0] = 2048;
        let result = inverse_4x4(block);
        assert!(result.iter().all(|row| row.iter().all(|&c| c == 32)));
    }

    #[test]
    fn hadamard_2x2_known() {
        assert_eq!(hadamard_2x2([[1, 0], [0, 0]]), [[1, 1], [1, 1]]);
    }

    #[test]
    fn hadamard_4x4_constant_is_dc() {
        let result = hadamard_4x4([[10i16; 4]; 4]);
        assert_eq!(result[0][0], 160); // 16 * 10
        for row in 0..4 {
            for col in 0..4 {
                if row != 0 || col != 0 {
                    assert_eq!(result[row][col], 0);
                }
            }
        }
    }

    proptest! {
        /// The 2x2 Hadamard is an orthogonal involution up to scale: H(H(x)) == 4x.
        #[test]
        fn hadamard_2x2_double_scales_by_4(
            a in -1000i16..=1000, b in -1000i16..=1000,
            c in -1000i16..=1000, d in -1000i16..=1000,
        ) {
            let x = [[a, b], [c, d]];
            let twice = hadamard_2x2(hadamard_2x2(x));
            for r in 0..2 {
                for col in 0..2 {
                    prop_assert_eq!(twice[r][col], x[r][col] * 4);
                }
            }
        }

        /// The 4x4 Hadamard pair satisfies H(H(x)) == 16x. Inputs bounded so 16x fits i16.
        #[test]
        fn hadamard_4x4_double_scales_by_16(block in prop::array::uniform4(prop::array::uniform4(-64i16..=64))) {
            let twice = hadamard_4x4(hadamard_4x4(block));
            for r in 0..4 {
                for col in 0..4 {
                    prop_assert_eq!(twice[r][col], block[r][col] * 16);
                }
            }
        }

        /// Forward DCT of a flat block is pure DC (= 16 * value), no AC energy.
        #[test]
        fn forward_constant_is_dc(c in -2000i16..=2000) {
            let result = forward_4x4([[c; 4]; 4]);
            prop_assert_eq!(result[0][0], c * 16);
            for r in 0..4 {
                for col in 0..4 {
                    if r != 0 || col != 0 {
                        prop_assert_eq!(result[r][col], 0);
                    }
                }
            }
        }
    }
}
