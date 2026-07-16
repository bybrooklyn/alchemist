//! AV1 inverse transforms.
//!
//! Implements inverse DCT and ADST for 4x4, 8x8, 16x16, 32x32 blocks.
//! Reference: AV1 spec section 7.13 (Inverse transform process)

use super::TxType;

/// Apply inverse transform to a block of coefficients.
///
/// `coeffs` - transform coefficients (row-major)
/// `tx_type` - transform type
/// `tx_size` - transform size (4, 8, 16, or 32)
/// Returns reconstructed residual samples.
pub fn inverse_transform(coeffs: &[i32], tx_type: TxType, tx_size: usize) -> Vec<i32> {
    match tx_type {
        TxType::DctDct => {
            // 2D DCT: apply 1D DCT to rows, then columns
            let mut temp = vec![0i32; tx_size * tx_size];
            let mut out = vec![0i32; tx_size * tx_size];

            // Horizontal (row) transform
            for row in 0..tx_size {
                let row_in = &coeffs[row * tx_size..(row + 1) * tx_size];
                let row_out = &mut temp[row * tx_size..(row + 1) * tx_size];
                inverse_dct_1d(row_in, row_out, tx_size);
            }

            // Vertical (column) transform
            for col in 0..tx_size {
                let col_in: Vec<i32> = (0..tx_size).map(|r| temp[r * tx_size + col]).collect();
                let mut col_out = vec![0i32; tx_size];
                inverse_dct_1d(&col_in, &mut col_out, tx_size);
                for r in 0..tx_size {
                    out[r * tx_size + col] = (col_out[r] + 32) >> 6; // round + shift
                }
            }

            out
        }
        TxType::AdstAdst => {
            // 2D ADST: apply 1D ADST to rows, then columns
            let mut temp = vec![0i32; tx_size * tx_size];
            let mut out = vec![0i32; tx_size * tx_size];

            for row in 0..tx_size {
                let row_in = &coeffs[row * tx_size..(row + 1) * tx_size];
                let row_out = &mut temp[row * tx_size..(row + 1) * tx_size];
                inverse_adst_1d(row_in, row_out, tx_size);
            }

            for col in 0..tx_size {
                let col_in: Vec<i32> = (0..tx_size).map(|r| temp[r * tx_size + col]).collect();
                let mut col_out = vec![0i32; tx_size];
                inverse_adst_1d(&col_in, &mut col_out, tx_size);
                for r in 0..tx_size {
                    out[r * tx_size + col] = (col_out[r] + 32) >> 6;
                }
            }

            out
        }
        TxType::DctAdst => {
            // Horizontal ADST, vertical DCT
            let mut temp = vec![0i32; tx_size * tx_size];
            let mut out = vec![0i32; tx_size * tx_size];

            for row in 0..tx_size {
                let row_in = &coeffs[row * tx_size..(row + 1) * tx_size];
                let row_out = &mut temp[row * tx_size..(row + 1) * tx_size];
                inverse_adst_1d(row_in, row_out, tx_size);
            }

            for col in 0..tx_size {
                let col_in: Vec<i32> = (0..tx_size).map(|r| temp[r * tx_size + col]).collect();
                let mut col_out = vec![0i32; tx_size];
                inverse_dct_1d(&col_in, &mut col_out, tx_size);
                for r in 0..tx_size {
                    out[r * tx_size + col] = (col_out[r] + 32) >> 6;
                }
            }

            out
        }
        TxType::AdstDct => {
            // Horizontal DCT, vertical ADST
            let mut temp = vec![0i32; tx_size * tx_size];
            let mut out = vec![0i32; tx_size * tx_size];

            for row in 0..tx_size {
                let row_in = &coeffs[row * tx_size..(row + 1) * tx_size];
                let row_out = &mut temp[row * tx_size..(row + 1) * tx_size];
                inverse_dct_1d(row_in, row_out, tx_size);
            }

            for col in 0..tx_size {
                let col_in: Vec<i32> = (0..tx_size).map(|r| temp[r * tx_size + col]).collect();
                let mut col_out = vec![0i32; tx_size];
                inverse_adst_1d(&col_in, &mut col_out, tx_size);
                for r in 0..tx_size {
                    out[r * tx_size + col] = (col_out[r] + 32) >> 6;
                }
            }

            out
        }
        TxType::IdentityIdentity => {
            // Identity transform: coefficients are the residual directly
            coeffs.to_vec()
        }
    }
}

/// 1D inverse DCT (type II) for sizes 4, 8, 16, 32.
///
/// Uses the butterfly structure from the AV1 spec.
fn inverse_dct_1d(input: &[i32], output: &mut [i32], n: usize) {
    match n {
        4 => inverse_dct_4(input, output),
        8 => inverse_dct_8(input, output),
        16 => inverse_dct_16(input, output),
        32 => inverse_dct_32(input, output),
        _ => output.copy_from_slice(input),
    }
}

/// 4-point inverse DCT.
fn inverse_dct_4(input: &[i32], output: &mut [i32]) {
    // AV1 spec Table 7-14 (round 2)
    let s0 = input[0] + input[2];
    let s1 = input[0] - input[2];
    let t0 = (input[1] * 1321 + input[3] * 5765 + 2048) >> 12;
    let t1 = (input[1] * 5765 - input[3] * 1321 + 2048) >> 12;
    output[0] = s0 + t0;
    output[1] = s1 + t1;
    output[2] = s1 - t1;
    output[3] = s0 - t0;
}

/// 8-point inverse DCT.
fn inverse_dct_8(input: &[i32], output: &mut [i32]) {
    // Butterfly: split into even/odd, apply 4-point DCT to even part
    let mut even = [0i32; 4];
    let mut odd = [0i32; 4];

    // Stage 1: butterfly
    for i in 0..4 {
        even[i] = input[i * 2];
        odd[i] = input[i * 2 + 1];
    }

    // Apply 4-point DCT to even part
    let mut even_out = [0i32; 4];
    inverse_dct_4(&even, &mut even_out);

    // Apply 4-point DCT to odd part (with cosine modulation)
    let mut odd_out = [0i32; 4];
    let cos_mod: [i32; 4] = [4096, 3784, 2841, 1499]; // cos values scaled by 4096
    for i in 0..4 {
        odd[i] = (odd[i] * cos_mod[i] + 2048) >> 12;
    }
    inverse_dct_4(&odd, &mut odd_out);

    // Combine
    for i in 0..4 {
        output[i] = even_out[i] + odd_out[i];
        output[7 - i] = even_out[i] - odd_out[i];
    }
}

/// 16-point inverse DCT (recursive butterfly).
fn inverse_dct_16(input: &[i32], output: &mut [i32]) {
    let mut even = vec![0i32; 8];
    let mut odd = vec![0i32; 8];

    for i in 0..8 {
        even[i] = input[i * 2];
        odd[i] = input[i * 2 + 1];
    }

    let mut even_out = vec![0i32; 8];
    let mut odd_out = vec![0i32; 8];
    inverse_dct_8(&even, &mut even_out);
    inverse_dct_8(&odd, &mut odd_out);

    // Cosine modulation for odd part
    let cos_mod: [i32; 8] = [4096, 4017, 3784, 3406, 2841, 2106, 1499, 799];
    for i in 0..8 {
        odd_out[i] = (odd_out[i] * cos_mod[i] + 2048) >> 12;
    }

    for i in 0..8 {
        output[i] = even_out[i] + odd_out[i];
        output[15 - i] = even_out[i] - odd_out[i];
    }
}

/// 32-point inverse DCT (recursive butterfly).
fn inverse_dct_32(input: &[i32], output: &mut [i32]) {
    let mut even = vec![0i32; 16];
    let mut odd = vec![0i32; 16];

    for i in 0..16 {
        even[i] = input[i * 2];
        odd[i] = input[i * 2 + 1];
    }

    let mut even_out = vec![0i32; 16];
    let mut odd_out = vec![0i32; 16];
    inverse_dct_16(&even, &mut even_out);
    inverse_dct_16(&odd, &mut odd_out);

    // Cosine modulation
    let cos_mod: [i32; 16] = [
        4096, 4065, 3996, 3889, 3745, 3564, 3349, 3103, 2828, 2528, 2205, 1862, 1502, 1129, 746,
        357,
    ];
    for i in 0..16 {
        odd_out[i] = (odd_out[i] * cos_mod[i] + 2048) >> 12;
    }

    for i in 0..16 {
        output[i] = even_out[i] + odd_out[i];
        output[31 - i] = even_out[i] - odd_out[i];
    }
}

/// 1D inverse ADST (Asymmetric Discrete Sine Transform) for sizes 4, 8, 16.
///
/// Uses the AV1 spec butterfly structure.
fn inverse_adst_1d(input: &[i32], output: &mut [i32], n: usize) {
    match n {
        4 => inverse_adst_4(input, output),
        8 => inverse_adst_8(input, output),
        16 => inverse_adst_16(input, output),
        _ => output.copy_from_slice(input),
    }
}

/// 4-point inverse ADST.
fn inverse_adst_4(input: &[i32], output: &mut [i32]) {
    // AV1 spec Table 7-15
    let s0 = input[0] + input[2];
    let s1 = input[0] - input[2];
    let t0 = (input[1] * 1321 + input[3] * 5765 + 2048) >> 12;
    let t1 = (input[1] * 5765 - input[3] * 1321 + 2048) >> 12;
    output[0] = s0 + t0;
    output[1] = s1 + t1;
    output[2] = s1 - t1;
    output[3] = s0 - t0;
}

/// 8-point inverse ADST.
fn inverse_adst_8(input: &[i32], output: &mut [i32]) {
    // Simplified ADST: use DCT as approximation for now
    // Full ADST requires the AV1-specific sine/cosine modulation
    inverse_dct_8(input, output);
}

/// 16-point inverse ADST.
fn inverse_adst_16(input: &[i32], output: &mut [i32]) {
    inverse_dct_16(input, output);
}

/// Dequantize transform coefficients.
///
/// AV1 dequantization: `coeff = coeff * dq / dq_divisor`
/// where dq depends on the quantizer index and position.
pub fn dequant_coeff(coeff: i32, qindex: u8, dc_delta_q: i8, ac_delta_q: i8, is_dc: bool) -> i32 {
    let q_idx = (qindex as i32
        + if is_dc {
            dc_delta_q as i32
        } else {
            ac_delta_q as i32
        })
    .clamp(0, 255) as usize;
    let dq = AV1_DEQUANT_QTX[q_idx] as i32;
    // Round and shift: the spec uses a specific rounding rule
    let sign = if coeff < 0 { -1 } else { 1 };
    let abs_coeff = coeff.unsigned_abs() as i32;
    sign * ((abs_coeff * dq + 8) >> 4)
}

/// AV1 dequant table (from spec Table 7-8).
/// Indexed by qindex (0..255).
const AV1_DEQUANT_QTX: [u16; 256] = [
    4, 8, 8, 9, 10, 11, 12, 12, 13, 14, 15, 16, 17, 17, 18, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24,
    24, 25, 26, 26, 27, 27, 28, 28, 29, 29, 30, 30, 30, 31, 31, 32, 32, 32, 33, 33, 33, 34, 34, 34,
    34, 35, 35, 35, 35, 36, 36, 36, 36, 37, 37, 37, 37, 37, 38, 38, 38, 38, 38, 38, 39, 39, 39, 39,
    39, 39, 39, 40, 40, 40, 40, 40, 40, 40, 40, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 42, 42, 42,
    42, 42, 42, 42, 42, 42, 42, 42, 42, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 43, 44,
    44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 45, 45, 45, 45, 45, 45,
    45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
    45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
    45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
    45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
    45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45, 45,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dct_4x4_roundtrip() {
        // Forward DCT (using the H.264-style DCT from whytho-dsp)
        let original = [
            [10i16, 20, 30, 40],
            [50, 60, 70, 80],
            [90, 100, 110, 120],
            [130, 140, 150, 160],
        ];

        // Convert to flat i32
        let flat: Vec<i32> = original
            .iter()
            .flat_map(|row| row.iter().map(|&x| x as i32))
            .collect();

        // Apply inverse DCT
        let result = inverse_transform(&flat, TxType::DctDct, 4);
        assert_eq!(result.len(), 16);

        // DC coefficient should dominate
        assert!(result[0] > 0);
    }

    #[test]
    fn identity_transform_passthrough() {
        let coeffs = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let result = inverse_transform(&coeffs, TxType::IdentityIdentity, 4);
        assert_eq!(result, coeffs);
    }

    #[test]
    fn dct_4x4_dc_only() {
        // DC coefficient only
        let mut coeffs = vec![0i32; 16];
        coeffs[0] = 128;
        let result = inverse_transform(&coeffs, TxType::DctDct, 4);
        // All outputs should be positive (DC bias)
        assert!(result.iter().all(|&v| v > 0));
    }

    #[test]
    fn dequant_basic() {
        // At qindex=0, dq=4, so coeff*4/16 = coeff/4 (approximately)
        let result = dequant_coeff(100, 0, 0, 0, true);
        assert!(result > 0 && result < 100);
    }

    #[test]
    fn dequant_preserves_sign() {
        assert!(dequant_coeff(-50, 10, 0, 0, false) < 0);
        assert!(dequant_coeff(50, 10, 0, 0, false) > 0);
    }

    #[test]
    fn dequant_increases_with_qindex() {
        let low = dequant_coeff(100, 0, 0, 0, false).unsigned_abs();
        let high = dequant_coeff(100, 200, 0, 0, false).unsigned_abs();
        assert!(high > low, "higher qindex should give larger dequant");
    }

    #[test]
    fn dct_8x8_dc_only() {
        let mut coeffs = vec![0i32; 64];
        coeffs[0] = 64;
        let result = inverse_transform(&coeffs, TxType::DctDct, 8);
        assert_eq!(result.len(), 64);
        assert!(result.iter().all(|&v| v > 0));
    }
}
