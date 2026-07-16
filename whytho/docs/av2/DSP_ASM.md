# DSP and embedded assembly (`av2-dsp`)

The headline requirement: **100% Rust with embedded ARM/x86 assembly.** The approach
(decided with the user):

> Every kernel ships a **safe-Rust scalar implementation that is the correctness source of
> truth**. Hot paths additionally get **hand-written `core::arch::asm!` kernels** for
> `aarch64` (NEON) and `x86_64` (SSE/AVX2), selected at **runtime**. Each asm kernel is
> tested for **bit-exactness against its scalar sibling**.

`unsafe` is confined to this crate (the asm kernels). The dispatch layer and scalar
baselines are safe.

## CPU detection (`cpu.rs`)

```rust
pub struct Features { pub neon: bool, pub sse4_1: bool, pub avx2: bool }
pub fn features() -> Features            // cached in a OnceLock
pub fn has_neon() -> bool                // aarch64
pub fn has_sse4_1() -> bool / has_avx2() // x86_64
```
Uses `std::arch::is_aarch64_feature_detected!` / `is_x86_feature_detected!`, gated by
`#[cfg(target_arch=…)]`. On aarch64 hosts NEON is always present (the test asserts it).

## Kernel family layout (the template)

Each family is a directory `<family>/` with:
- `mod.rs` — the **public dispatch function** + tests comparing dispatch vs scalar.
- `scalar.rs` — the safe baseline (always correct, always compiled).
- `neon.rs` — `#![cfg(target_arch = "aarch64")]`, `#[target_feature(enable="neon")]`
  `unsafe fn`s built from `core::arch::asm!`.
- `avx2.rs` — `#![cfg(target_arch = "x86_64")]` analog (TODO; can't run on this arm64 host).

Dispatch shape (from `sad/mod.rs`):
```rust
pub fn sad(src, src_stride, reference, ref_stride, w, h) -> u32 {
    #[cfg(target_arch = "aarch64")]
    if w == 8 && h == 8 && crate::cpu::has_neon() {
        // SAFETY: full 8x8 block bounds asserted; strides in elements.
        return unsafe { neon::sad8x8(src.as_ptr(), src_stride, reference.as_ptr(), ref_stride) };
    }
    scalar::sad(src, src_stride, reference, ref_stride, w, h)
}
```

## The reference kernel: 8×8 SAD in NEON (`sad/neon.rs`)

High-bitdepth (`u16`) samples. One row of 8 `u16` = one Q register.
```
movi v3.4s, #0            ; 4 x u32 accumulators
loop 8 rows:
  ld1 {v0.8h},[s] ; ld1 {v1.8h},[r]
  add s,s,ss ; add r,r,rs        ; strides passed in BYTES (elements*2)
  uabd v2.8h, v0.8h, v1.8h       ; |a-b| per u16 lane
  uadalp v3.4s, v2.8h            ; pairwise-add-accumulate into the 4 u32 lanes
addv s4, v3.4s ; fmov w,s4       ; horizontal reduce -> u32
```
Max sum `64 * 65535` fits `u32`. Operands: `inout(reg)` pointers, `in(reg)` byte strides,
clobber `v0..v4` via `out("vN") _`, `options(nostack, readonly)` (reads memory, writes none).

Validated by `sad8x8_dispatch_matches_scalar` over 256 random 8/10/12-bit blocks with
strides 8..=16, plus known-value and generic-size tests.

## How to add a new asm kernel (checklist)

1. Write `scalar.rs` first; it defines the exact semantics (rounding, clamping, types).
2. Add a `mod.rs` dispatch function with the scalar call as the default/fallback.
3. Write `neon.rs` with `#[target_feature(enable="neon")]` + `core::arch::asm!`; document a
   `// SAFETY:` precondition and assert bounds in the dispatcher before the `unsafe` call.
4. Add a test in `mod.rs` comparing the dispatched result to `scalar::…` over randomized
   inputs across all supported bit depths/sizes — this is the bit-exactness gate.
5. `cargo test -p av2-dsp`, then `cargo build --release -p av2-dsp` (asm must compile under
   `-O`), then `cargo clippy -p av2-dsp --all-targets`.

## x86 (SSE/AVX2) reality check

This host is **arm64**, so x86 asm **compiles but cannot execute here**. Plan:
- Write `avx2.rs` kernels guarded by `#[cfg(target_arch="x86_64")]` + `is_x86_feature_detected!`.
- Validate them via the scalar-equivalence tests under **CI or QEMU** (`x86_64-unknown-linux-gnu`),
  not on the dev machine. Keep scalar as the always-correct fallback so functionality never
  depends on x86 asm being exercised locally.

## Next kernels to implement (priority for the pipeline)

1. Forward transforms (DCT/ADST 4..64) — port kernels from spec `*_kernel*.h` into
   `av2-tables`, scalar `transform/scalar.rs`, then NEON. Needed by `av2-encoder::transform`.
2. Quantize (highbd) — `av2_quantize.c`; needed by `av2-encoder::quantize`.
3. Intra prediction (DC/V/H first) — `avm_dsp/intrapred.c`; needed by `av2-encoder::intra`.
4. SSE/variance — distortion for RD; extend the `sad` family pattern.
