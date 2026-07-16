# WhyTho Roadmap

## Near-term (next sessions)

### Native rewrites of absorbed engines
The three vendored encoders (rav1e, opus-rs, rust_h264) are absorbed as first-party crates.
The long-term goal is native whytho rewrites:
- **Opus encoder**: pure-Rust native replacement (opus-rs is functional but not ours)
- **H.264 decoder**: already native (`whytho-h264dec`); consider optimizing
- **AV1 encoder**: rav1e is the reference; native rewrite deferred indefinitely

### H.264 encoder — remaining gaps
I/P/B slices done (CAVLC only), I16x16+I4x4 mode selection by SATD cost. `me.rs`
(motion estimation) and `dpb.rs` (reference frame mgmt) are live, not staged.
I4x4 mode-prediction (`write_i4x4_mode`) checked against the decoder and confirmed
correct — not an open item (see `whytho/scratch.md`). Open:
- CABAC entropy coding: not present. An earlier non-functional scaffold
  (`cabac.rs`, wrong LPS table shape, unwired) was removed. Real CABAC needs the
  actual spec Table 9-44 (2D, state x range-class) + full context-init tables —
  substantial work, not a quick add. Reference: `whytho-h264dec`'s own working
  CABAC decoder/tables (`cabac.rs`, `cabac_tables.rs`, `decode_cabac.rs`).

### rav2e past 128x128
The AV2 encoder is a pre-alpha skeleton (128x128 still-keyframes). Needs:
- Non-zero residual coding
- Real geometry / partition trees
- Loop filters
- Rate control
- Multi-frame / tile support
- 10-bit output
- See `whytho/docs/av2/ROADMAP.md` for details

### H.264 rate control
Currently uses a minimal constant-QP-from-bitrate heuristic. Needs:
- Real RD-based rate control
- Adaptive QP per macroblock
- Buffer model / HRD compliance

## Medium-term

### Chunked + multithreaded transcode engine
The single biggest gap vs Spec.md (§13 Chunking, §14 Multithreading).
Currently only a 44-line `ChunkingMode` enum in `whytho-core/src/chunking.rs`.
Zero threading anywhere (no `std::thread`/`rayon`/`tokio`).
Needs:
- Keyframe-aware chunk boundaries
- Chunk scheduler in `whytho-core`
- Parallel chunk encode
- Boundary verification / seam handling

### End-to-end transcode (Spec MVP-3)
Real decode → plan → encode → mux → verify pipeline for H.264 → AV1.
Needs: pipeline orchestration, progress reporting, error recovery.

## Long-term

- AV2 decoder (in-house, for decode-side verification)
- Hardware-accelerated encode/decode backends
- Streaming / real-time transcode mode
- Container format support beyond MKV (MP4, WebM, HLS, DASH)
