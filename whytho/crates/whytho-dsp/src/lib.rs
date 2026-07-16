//! AV2 DSP kernels.
//!
//! Each kernel family exposes a safe public function that dispatches at runtime to a
//! hand-written `core::arch::asm!` implementation when the CPU supports it, falling back
//! to a safe-Rust `scalar` implementation that is the correctness source of truth.
//!
//! `unsafe` lives only in the per-arch asm kernels (added in the whytho-dsp task); the
//! scalar baselines and the dispatch layer are safe.
//!
//! Reference for the kernel inventory: `avm/avm_dsp/avm_dsp_rtcd_defs.pl` and
//! `avm/av2/common/av2_rtcd_defs.pl`.

pub mod cpu;

// Kernel families. Each becomes a `{mod,scalar,neon,avx2}` directory as it is implemented.
pub mod cdef;
pub mod convolve;
pub mod intrapred;
pub mod quantize;
pub mod sad;
pub mod transform;
