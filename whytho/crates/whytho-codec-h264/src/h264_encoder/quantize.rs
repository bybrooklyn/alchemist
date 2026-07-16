//! Quantization for H.264 encoder.

/// Quantize a 4x4 transform block.
///
/// H.264 quantization uses a scaling matrix with 6 levels (rem = qp % 6)
/// and a right shift (div = qp / 6).
///
/// MF values are calibrated for unnormalized DCT output (no >> 1 in forward DCT).
/// The 4x quantizer gain compensates for the 1/4 inverse DCT gain (>> 6).
/// Position category: (row & 1) + (col & 1) matches the decoder's position_category().
pub fn quantize_4x4(block: [[i16; 4]; 4], qp: i8) -> [[i16; 4]; 4] {
    let rem = (qp % 6) as usize;
    let div = (qp / 6) as i32;

    const MF: [[i32; 6]; 3] = [
        [13107, 11916, 10082, 9362, 8192, 7282], // pc=0: even-even
        [8066, 7490, 6554, 5825, 5243, 4559],    // pc=1: mixed
        [5243, 4660, 4194, 3647, 3355, 2893],    // pc=2: odd-odd
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

/// Quantize I16x16 luma DC coefficients after 4x4 Hadamard transform.
///
/// Uses only the MF[0][rem] scaling factor (DC position) per H.264 spec 8.5.5.
/// The DC quantizer uses shift = 17 + qp/6 (2 more bits than AC's 15 + qp/6)
/// to compensate for the Hadamard's 4x gain on constant inputs.
pub fn quantize_luma_dc_i16x16(block: [[i16; 4]; 4], qp: i8) -> [[i16; 4]; 4] {
    let rem = (qp % 6) as usize;
    let div = (qp / 6) as i32;

    const MF_DC: [i32; 6] = [13107, 11916, 10082, 9362, 8192, 7282];
    let mf = MF_DC[rem];

    let shift = 17 + div;
    let offset = 1i32 << (shift - 1);

    let mut result = [[0i16; 4]; 4];

    for row in 0..4 {
        for col in 0..4 {
            let level = block[row][col] as i32;
            let mag = (level.unsigned_abs() as i32 * mf + offset) >> shift;
            result[row][col] = if level < 0 { -mag } else { mag } as i16;
        }
    }

    result
}

/// Dequantize a 4x4 transform block.
pub fn dequantize_4x4(block: [[i16; 4]; 4], qp: i8) -> [[i16; 4]; 4] {
    let rem = (qp % 6) as usize;
    let div = (qp / 6) as i32;

    // H.264 dequantization scaling factors for 4x4 block
    const V: [[i32; 6]; 4] = [
        [10, 13, 16, 18, 20, 23],
        [13, 14, 18, 20, 23, 25],
        [16, 18, 20, 23, 25, 28],
        [10, 13, 16, 18, 20, 23],
    ];

    let mut result = [[0i16; 4]; 4];

    for row in 0..4 {
        for col in 0..4 {
            let level = block[row][col] as i32;
            let idx = (row & 1) + (col & 1);
            let v = V[idx][rem];

            result[row][col] = ((level * v) << div) as i16;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn quantize_zero_block() {
        let zero = [[0i16; 4]; 4];
        let quantized = quantize_4x4(zero, 26);
        assert!(quantized.iter().all(|row| row.iter().all(|&c| c == 0)));
    }

    #[test]
    fn quantize_high_qp_more_aggressive() {
        let block = [[100i16; 4]; 4];
        let q_low = quantize_4x4(block, 10);
        let q_high = quantize_4x4(block, 40);
        // Higher QP should produce smaller coefficients
        assert!(q_high[0][0].abs() <= q_low[0][0].abs());
    }

    #[test]
    fn dequantize_zero_block() {
        let result = dequantize_4x4([[0i16; 4]; 4], 26);
        assert!(result.iter().all(|row| row.iter().all(|&c| c == 0)));
    }

    #[test]
    fn dequantize_scales_up_with_qp() {
        // The reconstruction step grows with QP. Small level keeps us well inside i16.
        let block = [[2i16; 4]; 4];
        let low = dequantize_4x4(block, 6);
        let high = dequantize_4x4(block, 18);
        assert!(high[0][0].abs() > low[0][0].abs());
    }

    proptest! {
        /// Quantization never flips a coefficient's sign: each output is 0 or sign(input).
        #[test]
        fn quantize_sign_preserved(v in -512i16..=512, qp in 0i8..=51) {
            let q = quantize_4x4([[v; 4]; 4], qp);
            for row in &q {
                for &coeff in row {
                    prop_assert!(coeff == 0 || coeff.signum() == v.signum());
                }
            }
        }
    }
}
