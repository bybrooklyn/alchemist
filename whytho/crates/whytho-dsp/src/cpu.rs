//! Runtime CPU feature detection for SIMD dispatch.
//!
//! Detection is performed once and cached. The flags gate selection of the
//! hand-written asm kernels over the scalar baselines.

use std::sync::OnceLock;

/// Detected SIMD capabilities of the host CPU.
#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    /// aarch64 Advanced SIMD (NEON). Always present on aarch64, detected anyway.
    pub neon: bool,
    /// x86_64 SSE4.1.
    pub sse4_1: bool,
    /// x86_64 AVX2.
    pub avx2: bool,
}

fn detect() -> Features {
    #[allow(unused_mut)]
    let mut f = Features::default();
    #[cfg(target_arch = "aarch64")]
    {
        f.neon = std::arch::is_aarch64_feature_detected!("neon");
    }
    #[cfg(target_arch = "x86_64")]
    {
        f.sse4_1 = std::arch::is_x86_feature_detected!("sse4.1");
        f.avx2 = std::arch::is_x86_feature_detected!("avx2");
    }
    f
}

/// Returns the cached host CPU features.
pub fn features() -> Features {
    static CACHE: OnceLock<Features> = OnceLock::new();
    *CACHE.get_or_init(detect)
}

/// True if NEON is available (aarch64).
#[inline]
pub fn has_neon() -> bool {
    features().neon
}

/// True if SSE4.1 is available (x86_64).
#[inline]
pub fn has_sse4_1() -> bool {
    features().sse4_1
}

/// True if AVX2 is available (x86_64).
#[inline]
pub fn has_avx2() -> bool {
    features().avx2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_is_cached_and_consistent() {
        let a = features();
        let b = features();
        assert_eq!(a.neon, b.neon);
        assert_eq!(a.avx2, b.avx2);
        // On aarch64 hosts NEON must be present.
        #[cfg(target_arch = "aarch64")]
        assert!(a.neon, "aarch64 must report NEON");
    }
}
