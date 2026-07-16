//! Distortion metric: sum of absolute differences (SAD).
//!
//! Reference: `avm/avm_dsp/sad.c` and the arm/x86 SIMD variants. Public functions dispatch
//! to a hand-written asm kernel when one applies, else the scalar baseline.

pub mod scalar;

#[cfg(target_arch = "aarch64")]
pub mod neon;

/// Sum of absolute differences over a `w`x`h` block of `u16` samples.
///
/// `src_stride` / `ref_stride` are in elements. Dispatches to a NEON kernel for the sizes
/// that have one; otherwise uses the scalar baseline.
pub fn sad(
    src: &[u16],
    src_stride: usize,
    reference: &[u16],
    ref_stride: usize,
    w: usize,
    h: usize,
) -> u32 {
    #[cfg(target_arch = "aarch64")]
    if w == 8 && h == 8 && crate::cpu::has_neon() {
        debug_assert!(src.len() >= 7 * src_stride + 8);
        debug_assert!(reference.len() >= 7 * ref_stride + 8);
        // SAFETY: bounds for a full 8x8 block are asserted above; strides are in elements.
        return unsafe { neon::sad8x8(src.as_ptr(), src_stride, reference.as_ptr(), ref_stride) };
    }
    scalar::sad(src, src_stride, reference, ref_stride, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next_u32(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 32) as u32
        }
    }

    /// Random plane of `u16` samples masked to `bits` bit-depth, with the given stride.
    fn random_plane(rng: &mut Lcg, stride: usize, rows: usize, bits: u32) -> Vec<u16> {
        let mask = (1u32 << bits) - 1;
        (0..stride * rows)
            .map(|_| (rng.next_u32() & mask) as u16)
            .collect()
    }

    #[test]
    fn sad8x8_dispatch_matches_scalar() {
        // On aarch64 this exercises the NEON asm kernel; elsewhere it's scalar vs scalar.
        let mut rng = Lcg(0x5AD_5EED);
        for bits in [8u32, 10, 12] {
            for _ in 0..256 {
                let ss = 8 + (rng.next_u32() % 9) as usize; // strides 8..=16
                let rs = 8 + (rng.next_u32() % 9) as usize;
                let s = random_plane(&mut rng, ss, 8, bits);
                let r = random_plane(&mut rng, rs, 8, bits);
                let got = sad(&s, ss, &r, rs, 8, 8);
                let want = scalar::sad(&s, ss, &r, rs, 8, 8);
                assert_eq!(got, want, "bits={bits} ss={ss} rs={rs}");
            }
        }
    }

    #[test]
    fn sad8x8_known_values() {
        // All-zero ref: SAD == sum of src.
        let s: Vec<u16> = (0..64).map(|i| i as u16).collect();
        let r = vec![0u16; 64];
        let total: u32 = (0..64).sum();
        assert_eq!(sad(&s, 8, &r, 8, 8, 8), total);
        // Identical blocks: SAD == 0.
        assert_eq!(sad(&s, 8, &s, 8, 8, 8), 0);
    }

    #[test]
    fn scalar_generic_sizes() {
        // Constant difference of 3 over a 16x4 block -> 16*4*3.
        let s = vec![10u16; 16 * 4];
        let r = vec![7u16; 16 * 4];
        assert_eq!(scalar::sad(&s, 16, &r, 16, 16, 4), 16 * 4 * 3);
    }
}
