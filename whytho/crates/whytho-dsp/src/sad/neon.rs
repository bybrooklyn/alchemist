//! Hand-written AArch64 NEON SAD kernels (`core::arch::asm!`).
//!
//! These are the embedded-assembly fast paths. They are validated for bit-exactness
//! against [`super::scalar`] in the module tests. High-bitdepth (`u16`) samples.
#![cfg(target_arch = "aarch64")]

use core::arch::asm;

/// 8x8 `u16` SAD in NEON assembly.
///
/// `src_stride` / `ref_stride` are in elements. Each row of 8 `u16` is one Q register;
/// `uabd` computes per-lane |a-b| and `uadalp` accumulates pairwise into four 32-bit
/// lanes across all 8 rows, which `addv` finally reduces. The 8x8 max sum
/// (`64 * 65535`) fits in `u32`.
///
/// # Safety
/// `src` and `reference` must each point to a readable 8x8 block with the given strides
/// (i.e. at least `7 * stride + 8` elements available from the pointer).
#[target_feature(enable = "neon")]
pub unsafe fn sad8x8(
    src: *const u16,
    src_stride: usize,
    reference: *const u16,
    ref_stride: usize,
) -> u32 {
    let mut sp = src;
    let mut rp = reference;
    let src_byte_stride = src_stride * 2;
    let ref_byte_stride = ref_stride * 2;
    let sum: u32;
    unsafe {
        asm!(
            "movi v3.4s, #0",
            "mov {cnt:w}, #8",
            "2:",
            "ld1 {{v0.8h}}, [{s}]",
            "ld1 {{v1.8h}}, [{r}]",
            "add {s}, {s}, {ss}",
            "add {r}, {r}, {rs}",
            "uabd v2.8h, v0.8h, v1.8h",
            "uadalp v3.4s, v2.8h",
            "subs {cnt:w}, {cnt:w}, #1",
            "b.ne 2b",
            "addv s4, v3.4s",
            "fmov {sum:w}, s4",
            s = inout(reg) sp,
            r = inout(reg) rp,
            ss = in(reg) src_byte_stride,
            rs = in(reg) ref_byte_stride,
            cnt = out(reg) _,
            sum = out(reg) sum,
            out("v0") _,
            out("v1") _,
            out("v2") _,
            out("v3") _,
            out("v4") _,
            options(nostack, readonly),
        );
    }
    let _ = (sp, rp);
    sum
}
