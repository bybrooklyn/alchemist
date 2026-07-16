# Benchmark: rust_h264 vs FFmpeg

## Test Setup

- **Platform:** Apple Silicon (ARM64), macOS
- **Source:** `testsrc2` (animated test pattern with motion, text, color bars)
- **Streams:**
  - 720p: 1280x720, 300 frames, x264 `--preset medium --no-deblock`, CABAC
    - P-only: `bframes=0 ref=1`
    - B-frames: `bframes=3 ref=4`
  - 1080p: 1920x1080, 100 frames, x264 `--preset medium --no-deblock`, CABAC, `bframes=3 ref=2`
- **FFmpeg:** Single-threaded (`-threads 1`), software decode, compiled with `-O3` + NEON assembly
- **rust_h264:** `cargo build --release`, pure Rust + NEON `half_pel_h` intrinsics

### Stream generation

```bash
# 1080p, 100 frames
ffmpeg -f lavfi -i "testsrc2=s=1920x1080:rate=30:duration=3.33" -frames:v 100 \
  -c:v libx264 -preset medium -crf 23 \
  -x264opts "bframes=3:ref=2:no-deblock:keyint=250:min-keyint=25" \
  -f h264 bench_1080p_100f_complex.h264

# 720p P-only, 300 frames
ffmpeg -f lavfi -i "testsrc2=s=1280x720:rate=30:duration=10" -frames:v 300 \
  -c:v libx264 -preset medium -crf 23 \
  -x264opts "bframes=0:ref=1:no-deblock:keyint=250:min-keyint=25" \
  -f h264 bench_720p_300f_ponly_complex.h264

# 720p B-frames, 300 frames
ffmpeg -f lavfi -i "testsrc2=s=1280x720:rate=30:duration=10" -frames:v 300 \
  -c:v libx264 -preset medium -crf 23 \
  -x264opts "bframes=3:ref=4:no-deblock:keyint=250:min-keyint=25" \
  -f h264 bench_720p_300f_bframes_complex.h264
```

## Results

### 720p (1280x720, 300 frames)

| Decoder | Stream | Time (user) | FPS | Memory |
|---------|--------|-------------|-----|--------|
| FFmpeg | P-only | 0.30s | 1000 | — |
| FFmpeg | B-frames | 0.31s | 968 | — |
| rust_h264 | P-only | 0.77s | 390 | 12 MB |
| rust_h264 | B-frames | 1.18s | 254 | 12 MB |

### 1080p (1920x1080, 100 frames)

| Decoder | Stream | Time (user) | FPS | vs 30fps | vs 60fps |
|---------|--------|-------------|-----|----------|----------|
| FFmpeg | B-frames | 0.22s | 454 | 15.1x | 7.6x |
| rust_h264 | B-frames | 0.89s | 112 | 3.7x | 1.9x |

**FFmpeg is 2.6-4.0x faster.** FFmpeg uses hand-tuned NEON/SSE assembly for all
MC filters, IDCT, and deblocking. rust_h264 uses NEON only for `half_pel_h`.

**1080p @ 60fps target achieved** — 112 fps (1.9x realtime at 60fps).
720p B-frames at 254 fps (8.5x realtime at 30fps).

### Note on synthetic sources

Earlier benchmarks used `mandelbrot` as the video source, which produces
near-static content with mostly skip MBs. This inflated rust_h264 FPS numbers
(67 fps at 1080p) and exaggerated the FFmpeg ratio (reported as 50-110x).
The `testsrc2` source has realistic motion and texture, giving more
representative numbers.

## Profile Breakdown

Sampled with macOS `sample` command on the 720p P-only decode:

| Component | % Time | Description |
|-----------|--------|-------------|
| **Luma MC (half-pel filters)** | **42%** | 6-tap FIR filter for sub-pixel interpolation |
| **CABAC decode overhead** | **25%** | Loop/store overhead (18.7%), arithmetic engine (2.8%), residual/cbp/mvd (3.5%) |
| **Chroma MC** | **18%** | Bilinear interpolation at 1/8-pel precision |
| **P_Skip MC** | **10%** | Combined luma+chroma MC for skip MBs |
| Inverse DCT | 3% | 4x4 integer IDCT |
| finalize_mb_info | 2% | Per-MB metadata copy for deblocking |
| Other | 1% | MV prediction, reconstruction, etc. |

### B-frame profile (720p, bframes=3 ref=4, CABAC)

Sampled with macOS `sample` on 300-frame 720p B-frame decode (1.40s user):

| Component | % Time | Description |
|-----------|--------|-------------|
| **Luma MC** | **42%** | 6-tap FIR half-pel filters (25% in B_Skip, 16% in other inter) |
| **Chroma MC** | **19%** | Bilinear 1/8-pel (13% B_Skip, 6% other inter) |
| **Spatial direct MV** | **9%** | `derive_spatial_direct_blk` per-4x4-block derivation |
| **Bi-pred averaging** | **7%** | L0+L1 pixel averaging in B_Skip |
| **CABAC decode** | **5%** | Residual (2%), syntax elements (2%), neighbor/dequant (1%) |
| Deblock/frame mgmt | 4% | Deblocking filter + DPB management |
| Reconstruct | 2% | Luma/chroma reconstruction from residual |
| Inverse DCT | 2% | 4x4 integer IDCT |
| Other | 10% | MV prediction, malloc, unaccounted |

**Key difference from P-only:** B_Skip dominates (55% of total), with spatial
direct MV derivation (9%) as a new significant cost. CABAC overhead dropped
from 25% to 5% after the `OFFSET_TO_BLOCK` optimization — the reverse lookups
were a major B-slice bottleneck since each B MB required dual-list neighbor
queries.

### 1080p profile (1920x1080, bframes=3 ref=2, CABAC)

Sampled with macOS `sample` on 100-frame 1080p B-frame decode (2.10s user):

| Component | % Time | Description |
|-----------|--------|-------------|
| **Luma MC** | **~55%** | `luma_mc` + `half_pel_h`/`half_pel_v`/`half_pel_hv` |
| **Chroma MC** | **~13%** | Bilinear 1/8-pel interpolation |
| **Spatial direct MV** | **~12%** | `derive_spatial_direct_blk` per-4x4-block |
| **CABAC residual** | **~10%** | `decode_residual_cabac` coefficient parsing |
| **CABAC engine** | **~4%** | `get_cabac` arithmetic decode |
| Other | ~6% | MV prediction, bi-pred, dequant, malloc |

**Detailed leaf-level breakdown** (non-overlapping):

| Function | % Time | Description |
|----------|--------|-------------|
| **`luma_mc` overhead** | **21.5%** | Per-pixel loop, `luma_interp` dispatch, `ref_luma` bounds clamping |
| **`half_pel_h`** | **19.4%** | 6-tap horizontal FIR filter |
| **`chroma_mc`** | **13.8%** | Bilinear 1/8-pel with per-pixel clamping |
| **`decode_residual_cabac`** | **11.3%** | Significance map + coefficient level decode |
| **`half_pel_v`** | **10.5%** | 6-tap vertical FIR filter |
| **`half_pel_hv`** | **8.9%** | 2-pass 6-tap diagonal filter |
| `derive_spatial_direct_blk` | 6.0% | Neighbor MV lookup + co-located check |
| `get_cabac` | 4.6% | Arithmetic decode engine |
| Other | 4.1% | DCT, dequant, weight, mvd, predict_mv |

**Key insight:** `luma_mc` overhead (21.5%) is as expensive as `half_pel_h` (19.4%).
This is the per-pixel `luma_interp` dispatch and `ref_luma` boundary clamping — not
the filter math itself. A row-based approach that processes entire rows with a single
bounds check would cut this significantly even without SIMD.

## Optimization Opportunities

### 1. Luma MC (60% total) — High impact

Luma MC has two bottlenecks: the filter math (39%) and the per-pixel overhead (21%).

**`luma_mc` overhead (21.5%):** The current code calls `luma_interp` -> `half_pel_*`
-> `ref_luma` per pixel. Each `ref_luma` call does bounds clamping. Restructuring to
process entire rows with a single bounds check (is the entire row within bounds?)
would eliminate most of the overhead.

**`half_pel_h`/`half_pel_v`/`half_pel_hv` (38.8%):** The 6-tap FIR filter does
6 multiplications + additions + clipping per pixel.

**Approaches:**
- **Row-based processing:** Process entire rows with one bounds check instead of
  per-pixel clamping. Enables compiler auto-vectorization. Medium effort, no SIMD
  dependency. Expected: **~15-20% overall improvement** (eliminates 21.5% overhead).
- **SIMD (NEON):** Process 8 pixels per instruction with `vmull`/`vmlal`.
  `std::arch::aarch64` intrinsics or `std::simd` (nightly). Expected: **3-5x
  speedup for filter math -> ~1.5-2x overall**.

### 2. Chroma MC (14%) — Medium impact

Same per-pixel overhead pattern as luma: `ref_chroma` boundary clamping per pixel.
Row-based + NEON bilinear would help.

### 3. CABAC decode (16%) — Low impact, hard to optimize

`decode_residual_cabac` (11.3%) and `get_cabac` (4.6%) are bit-serial.
The actual bottleneck is the surrounding code in `decode_cabac_mb`.

**Approaches:**
- ~~**`BLOCK_INDEX_TO_OFFSET` lookup table:**~~ Done — see optimization #6 below.
- **Inline `cabac_neighbor_*` functions:** The neighbor context lookups involve
  multiple function calls with many parameters. `#[inline(always)]` or manual
  inlining would reduce call overhead.
- **Reduce array stores:** MV/ref/MVD stores write to every 4x4 block (16 writes
  for a 16x16 partition). For uniform partitions, a single `memset`-style fill
  would be faster.

Expected improvement: **~10-20% of CABAC time -> ~3-5% overall**

### 3. Chroma MC (18%) — Medium impact, low effort

Chroma MC uses bilinear interpolation at 1/8-pel. Simpler than luma but still
per-pixel with multiplications.

**Approaches:**
- **SIMD:** Same NEON approach as luma MC.
- ~~**Strength reduction:** For full-pel chroma (frac=0), skip interpolation entirely
  and use `copy_from_slice`.~~ Done — see optimization #7 below.

Expected improvement: **2-4x for chroma MC -> ~5-10% overall** (SIMD only; full-pel
fast path already implemented)

### 4. Inverse DCT (3%) — Low impact

Already fast. SIMD could help for 8x8 IDCT in High profile but the 4x4 IDCT
is simple enough that scalar code is nearly optimal.

### 5. Memory allocation (done)

Replaced `Vec` heap allocations with stack arrays in hot paths:
- MC prediction buffers: `vec![0u8; w*h]` -> `[0u8; 256]`
- `b_sub_parts`: `Vec<BSubPart>` -> `[BSubPart; 16]`
- `BSubLayout`: `Vec<BSubLayout>` -> `[BSubLayout; 4]`
- Sub-partition offsets: `vec![...]` -> `&[...]` static slices

**Result: ~4% improvement** (1.70s -> 1.63s)

### 6. OFFSET_TO_BLOCK reverse lookup table (done)

Replaced ~46 O(16) linear scans (`BLOCK_INDEX_TO_OFFSET.iter().position()`) with
O(1) `OFFSET_TO_BLOCK[row][col]` table lookups across `neighbor.rs`,
`decode_cabac.rs`, `decode_cavlc.rs`, and `mv_pred.rs`. These reverse lookups
convert (row, col) grid coordinates to block indices and were called dozens of
times per MB for neighbor context (amvd, ref_idx, coded_block_flag) and MV
prediction.

**Result: ~11% improvement on B-frames** (1.58s -> 1.40s), P-only within noise
(1.60s -> 1.65s). The B-frame gain is larger because B-slices exercise the
reverse lookup much more heavily: dual-list neighbor lookups, direct mode checks,
and spatial/temporal MV derivation.

### 7. Full-pel MC fast path (done)

Added early-exit fast paths in `luma_mc` and `chroma_mc`: when the fractional MV
is zero (integer-pel position), skip the 6-tap FIR / bilinear interpolation and
`copy_from_slice` directly from the reference buffer. Inner-bounds check avoids
per-pixel clamping for blocks fully within the picture.

**Result: ~2% improvement** (P-only 1.65s -> 1.61s, B-frames 1.40s -> 1.37s).
Modest because x264 `--preset medium` (subme=7) produces mostly sub-pel MVs.
Streams with simpler motion estimation or static content would see larger gains.

### 8. Spatial direct MV dedup + inlining (done)

When `direct_8x8_inference_flag` is set, all 4 blocks within each 8x8 group
derive the same spatial/temporal direct MVs. Reduced from 16 derivation calls
per MB to 4 (one per 8x8 group), filling sub-blocks by copy. Also added
`#[inline(always)]` to hot neighbor functions (`cabac_amvd`, `cabac_neighbor_ref`,
`get_mv_neighbor_left/above/above_right/above_left`).

**Result: negligible** (~0.5% at 1080p). The per-call cost was already low after
the `OFFSET_TO_BLOCK` optimization, and LLVM was already inlining the neighbor
functions in release mode.

### 9. Row-based luma MC restructure (done)

Restructured `luma_mc` to dispatch on `(frac_x, frac_y)` once per block instead
of per pixel. In-bounds blocks use direct buffer slicing (`&ref_y[off..off+len]`)
with row-based filter functions (`row_half_pel_h`, `row_half_pel_v`,
`row_half_pel_hv`), eliminating per-pixel `ref_luma` clamping and `luma_interp`
dispatch. Boundary blocks fall back to the original per-pixel path.

**Result: negligible in release** (LLVM was already inlining and optimizing the
per-pixel path to equivalent code at `-O3`). **29% faster in debug mode**
(test suite: 6.38s -> 4.52s), confirming the structural improvement. The new
row-based functions (`row_half_pel_h` etc.) are natural NEON SIMD targets.

### 10. NEON half_pel_h (done)

Replaced the scalar `row_half_pel_h` with NEON intrinsics (`std::arch::aarch64`).
Processes 8 pixels per iteration using `vld1_u8` (6 overlapping loads), `vaddl_u8`
(widen to u16), `vmlaq_n_s16`/`vmlsq_n_s16` (multiply-accumulate with coefficients
20 and -5), `vshrq_n_s16` (right shift by 5), and `vqmovun_s16` (saturating narrow
to u8). Scalar tail handles remaining 0-7 pixels per row.

Key insight: `#[inline(never)]` on the NEON function is critical — without it,
LLVM's inliner absorbs the intrinsics into the enormous caller function and
scalarizes them back. With `#[inline(never)]`, vector instructions are preserved.

**Result: 28-37% improvement** on mandelbrot-source streams.
Now measured at **112 fps (1080p)** and **254-390 fps (720p)** on realistic
`testsrc2` content.

## Realistic Performance Target

**Target: 1080p @ 60 fps — ACHIEVED** (112 fps on testsrc2, 1.9x realtime at 60fps).

### Completed optimizations

1. ~~BLOCK_INDEX_TO_OFFSET lookup table~~ — ~11% B-frame improvement
2. ~~Full-pel MC fast path~~ — ~2% (content-dependent)
3. ~~Spatial direct MV dedup~~ — negligible (per-call cost already low)
4. ~~`#[inline(always)]` on neighbor functions~~ — negligible (LLVM already inlining)
5. ~~Row-based MC processing~~ — no release improvement, but structured for SIMD
6. ~~NEON `half_pel_h`~~ — **28-37% improvement** on simple content

### Further SIMD opportunities

7. **NEON `half_pel_v`** — Same 6-tap filter but vertical. 10.5% of pre-NEON
   1080p time. Needs column gather from 6 rows.

8. **NEON `half_pel_hv`** — 2-pass diagonal filter. 8.9% of pre-NEON time.

9. **NEON chroma MC** — 8-wide bilinear. ~14% of pre-NEON time.

10. **NEON bi-pred averaging** — `vrhadd_u8` does `(a+b+1)>>1` in one instruction.

**720p** is **8.5x realtime** at 30fps (254 fps with B-frames).
**1080p** is **3.7x realtime** at 30fps (112 fps), **1.9x at 60fps**.
Items 7-9 would increase 1080p headroom to ~150+ fps.
