# Roadmap

The approved strategy: **build a full pipeline skeleton, then fill stages in**, validating
each against the native reference decoder. The initial skeleton now exists: entropy, tables,
DSP dispatch, common types, a constrained encoder/CLI, and AVM validation are implemented.

Completed execution details follow for the initial roadmap. Future production work is listed
in "Beyond the skeleton".

## §2 — `av2-common` enums & types  (DONE)

_Packaging note (2026-07-03): the `av2-common` crate described below was merged into
`whytho-codec-av2` as its `common` module — nothing else depended on it as a separate
crate. The types and behavior described here are unchanged._

Port from `avm/av2/common/enums.h`, `avm/av2/common/blockd.h`, `avm/av2/encoder/enc_enums.h`.

- `BlockSize` — `BLOCK_4X4 … BLOCK_128X128` incl. rectangular (4x8, 8x4, …) and the 1:4/4:1
  shapes AV2 allows. Provide `width()`, `height()`, `width_log2()`, `height_log2()` helpers.
- `TxSize` — `TX_4X4 … TX_64X64` + rectangular. Helpers for w/h and square-up.
- `TxType` — DCT_DCT, ADST_*, FLIPADST_*, IDTX, V_/H_ variants (the separable combos).
- `PredictionMode` — DC, V, H, D45/D135/D113/D157/D203/D67, SMOOTH/SMOOTH_V/SMOOTH_H, PAETH;
  leave AV2-specific IBP/DIP as named variants with TODOs.
- `PartitionType` — NONE, SPLIT, HORZ, VERT, HORZ_A, HORZ_B, VERT_A, VERT_B, HORZ3, VERT3.
- `FrameType` — KEY_FRAME, INTER_FRAME, INTRA_ONLY, S_FRAME (only KEY needed first).
- `image.rs` — a `Plane { data: Vec<u16>, stride, width, height }` and `Frame { y,u,v,
  bit_depth, subsampling_x/y }`. Allocation should pad to superblock/stride alignment.
- SB size is **128×128** (`MAX_SB_SIZE`).

Acceptance: done in `28c722d`; enums round-trip, helpers are table-backed, image buffers are
safe and `unsafe`-free. Partition trees and block sizes above 128×128 remain deferred.

## §4 — `xtask gen-tables` + `av2-tables` (DONE)

The spec attachments are bare C initializers like `Dct_Kernel4[4][4] = {{64,64,64,64},…}`
(no type/`static`). Conversion is mechanical because C nested `{}` ≡ Rust nested `[]`.

Implement `xtask gen-tables`:
1. Read each `../av2-spec/v1.0.0/attachments/*.h`.
2. Parse the LHS `Name[d0][d1]…` to get dimensions; pick a Rust name (UPPER_SNAKE).
3. Infer element type: CDF tables → `u16`; transform kernels → `i32` (verify range, some
   fit `i16`); scan/size LUTs → `u8`/`u16`. Keep a small per-file override map for oddities.
4. Replace `{`→`[`, `}`→`]`, strip the trailing `;` artifacts, and emit
   `pub static NAME: [[T; d1]; d0] = […];` into `crates/av2-tables/src/generated/<name>.rs`.
5. Regenerate `generated/mod.rs` with `pub mod <name>;` lines + a header banner.
6. Cross-check a few against AVM's `entropy_inits_*.h` to confirm equality.

Counts to expect: ~180 `default_*_cdf.h`, ~11 `*_kernel*.h`, plus scan/size/quant LUTs.
The default CDFs are Q-context indexed (`get_q_ctx`: q≤90→0, ≤140→1, ≤190→2, else 3) — port
`av2_default_coef_probs`'s selection logic into `av2-tables` (or `av2-encoder`).
Note: `all_tables.h` (1.7 MB) is the consolidated set; prefer the individual files for
reviewability, or use `all_tables.h` as a cross-check.

Acceptance: done in `b99f8f4`; `cargo run -p xtask -- gen-tables --check` is idempotent,
committed `generated/` builds, and tests spot-check DCT, CDF, symbolic tables, and
coefficient Q-context boundaries.

## §6 — `av2-encoder` pipeline skeleton + real bitstream (DONE, CONSTRAINED)

Implement `bitstream.rs` **for real** first (it gates validation), then wire stubs.

Minimal decodable keyframe OBU sequence (see [BITSTREAM_NOTES.md](BITSTREAM_NOTES.md)):
`OBU_TEMPORAL_DELIMITER (2)` → `OBU_SEQUENCE_HEADER (1)` → `OBU_CLOSED_LOOP_KEY (4)`.
Raw `.obu` files use ULEB size of `header + payload`, then header, then payload.

- A non-arithmetic bit-buffer writer for the **uncompressed** header fields (the sequence
  header and the uncompressed frame header are written MSB-first into a byte buffer, not the
  range coder) — mirror `struct avm_write_bit_buffer` in `avm/av2/encoder/bitstream.c`.
- Sequence header fields (see `write_sequence_header` ≈ bitstream.c:4894): profile, level,
  `bit_depth` (8/10), `monochrome=0`, 4:2:0 subsampling, max frame size, `use_128x128_sb`,
  tool-enable flags (set conservative: advanced tools OFF).
- Frame header (`write_uncompressed_header` ≈ bitstream.c:5190): KEY_FRAME, `show_frame=1`,
  frame/render size, `base_q_idx`, Y/U/V dc/ac deltas, segmentation OFF, loop filter / CDEF /
  loop restoration / CCSO OFF, `tx_mode`, single tile (`tile_cols=tile_rows=1`).
- Tile data is range-coded with `av2-entropy::Writer` using default CDFs from `av2-tables`.

Then wire the pipeline (`partition` → `intra` → `transform` → `quantize` → `tokenize` →
`encodetxb`) starting trivially: PARTITION_NONE, DC_PRED, all-skip / all-zero coefficients,
so the first artifact is a decodable (if low quality) keyframe. Grow from there.

Acceptance: done in `d142026`; `av2enc in.y4m out.obu` produces a 25-byte raw `.obu` for
128×128 8-bit 4:2:0 input that native `avmdec` decodes. The current payload is a fixed
deterministic bootstrap seed; replacing it with real Rust sequence/frame/tile payload writers
is the next encoder task.

## §7 — Validation harness (`xtask`) (DONE)

- `build-refdec`: out-of-tree `cmake` build of `../avm` (Release) producing `avmdec`
  (+ `examples/decode_to_md5`). Cache under `rs/target/refdec/`. See [VALIDATION.md](VALIDATION.md).
- `validate`: run `av2enc` on a tiny test frame → wrap to `.ivf`/`.obu` → `avmdec` decode →
  assert decoded I420 size and MD5 `58efe7d34c4f36aab183bbf18a3f1e6a`.

## Beyond the skeleton (later)

- Replace the fixed AVM-derived bootstrap payload with real Rust writers for sequence header
  (**done** — `write_sequence_header_payload`), uncompressed frame header (**preamble done** —
  `write_frame_header_payload`; quant/loop-filter/tile fields still in the constant tail), tile
  header, partition/mode syntax, and all-zero coefficient entropy syntax.
- Add non-zero residual coding for the single 128×128 path before expanding geometry.
- More `av2-dsp` kernels with NEON + AVX2 paths: forward transforms (DCT/ADST 4..64,
  port kernels from spec), quantize, intra-prediction, variance/SSE, convolution, CDEF.
- Real RD: partition search, intra mode search, transform type/size search, trellis quant.
- Inter prediction + motion search; reference frame management; GOP structure.
- In-loop filters (deblock, CDEF, loop restoration, CCSO) — encoder-side param search.
- Rate control; multi-tile; multi-threading; film grain.
- x86 asm paths exercised under CI/QEMU (can't run natively on the arm64 host).
