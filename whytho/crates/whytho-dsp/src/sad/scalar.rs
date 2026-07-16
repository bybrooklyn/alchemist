//! Safe-Rust scalar SAD baseline — the correctness source of truth.

/// Sum of absolute differences over a `w`x`h` block of `u16` (high-bitdepth) samples.
///
/// `src_stride` / `ref_stride` are in elements (samples), not bytes.
pub fn sad(
    src: &[u16],
    src_stride: usize,
    reference: &[u16],
    ref_stride: usize,
    w: usize,
    h: usize,
) -> u32 {
    let mut sum = 0u32;
    for y in 0..h {
        let s = &src[y * src_stride..][..w];
        let r = &reference[y * ref_stride..][..w];
        for x in 0..w {
            sum += (s[x] as i32 - r[x] as i32).unsigned_abs();
        }
    }
    sum
}
