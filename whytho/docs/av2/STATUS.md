# Status

_Last updated: 2026-06-17. Keep this current — it is the first thing a new session reads._

_2026-07-03: the `av2-common` crate (referenced below) was merged into
`whytho-codec-av2` as its `common` module — no other crate depended on it
separately. Types/behavior unchanged, packaging only._

## TL;DR

A working skeleton exists: the workspace builds, **common codec types and frame buffers are
implemented**, **spec attachment tables are generated and committed**, the **entropy coder is
bit-exact-validated**, the **embedded-asm DSP path is proven** with a NEON kernel, and
`av2enc` emits a minimal 128x128 8-bit 4:2:0 still-picture `.obu` that native `avmdec`
decodes.

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check`, `cargo run -p xtask -- gen-tables --check`, and
`cargo run -p xtask -- validate` are green in the current worktree.

## Git history (repo root is `rs/`, branch `main`)

| Commit    | Summary |
|-----------|---------|
| `408711d` | Scaffold the 7-crate workspace, LICENSE/PATENTS, toolchain pin, CLAUDE.md |
| `cb454dd` | `av2-entropy`: bit-exact `od_ec` range coder, CDF model, symbol writer |
| `5f72221` | `av2-tables`: make `generated` module resolvable for fmt/clippy |
| `f7ac7cf` | `av2-dsp`: SAD dispatch with hand-written NEON `asm!` kernel |
| `28c722d` | `av2-common`: AVM-aligned enums, partition helpers, 4:2:0 frame buffers |
| `b99f8f4` | `av2-tables`: deterministic generated tables from 245 spec attachments |
| `d142026` | `av2-encoder`: minimal decodable 128x128 still-picture keyframe + CLI |

(Run `git log --oneline` for the live list; update this table as commits land.)

## Task board

| # | Task | State | Notes |
|---|------|-------|-------|
| 1 | Scaffold workspace + `git init` in `rs/` | **DONE** | `408711d` |
| 2 | `av2-common` enums & types | **DONE** | AVM-aligned enums, helpers, partition subsizes, and 4:2:0 buffers |
| 3 | `av2-entropy` range coder (real) | **DONE** | `cb454dd`; see [ENTROPY.md](ENTROPY.md) |
| 4 | `xtask gen-tables` + `av2-tables` | **DONE** | 245 individual headers generated; `all_tables.h` intentionally excluded |
| 5 | `av2-dsp` dispatch + scalar + first NEON asm | **DONE** | `f7ac7cf` (SAD); more kernels are future work |
| 6 | `av2-encoder` pipeline skeleton + bitstream | **DONE** | Constrained 128x128 8-bit 4:2:0 still-picture seed; see [BITSTREAM_NOTES.md](BITSTREAM_NOTES.md) |
| 7 | Validation harness (`build-refdec` + `validate`) | **DONE** | Native `avmdec` decode + MD5 gate; see [VALIDATION.md](VALIDATION.md) |

## What is implemented for real (not stubs)

- **`av2-common`** — AVM-aligned block/transform/mode/partition/frame enums, reference-table
  geometry helpers, supported partition-subsize lookup, and safe stride-padded 4:2:0 frame
  buffers. Partition trees and block sizes above 128×128 are intentionally deferred.
- **`av2-tables`** — 245 individual `../av2-spec/v1.0.0/attachments/*.h` files are parsed by
  a deterministic `xtask gen-tables` generator into committed Rust modules. `all_tables.h` is
  intentionally excluded as an aggregate. `gen-tables --check` is the idempotence gate.
- **`av2-entropy`** — full `od_ec` range encoder (`OdEcEnc`), CDF adaptation (`update_cdf`),
  and a `Writer`. Validated bit-exact by round-tripping against a faithful port of the
  reference range *decoder*. 7 passing tests. Details: [ENTROPY.md](ENTROPY.md).
- **`av2-dsp`** — `cpu.rs` runtime feature detection; the scalar/asm dispatch pattern;
  an 8×8 `u16` SAD kernel in AArch64 NEON (`core::arch::asm!`) validated bit-exact vs the
  scalar baseline over 8/10/12-bit blocks. Details: [DSP_ASM.md](DSP_ASM.md).
- **`av2-encoder` / `av2-cli`** — safe OBU framing primitives, fixed typed stages
  (`PARTITION_NONE`, `DC_PRED`, zero residual), and a CLI that reads Y4M/raw planar 4:2:0 and
  emits a raw `.obu`. Initial support is deliberately limited to 128×128 8-bit 4:2:0. The
  **sequence-header and key-frame-header payloads are now written by real Rust writers**
  (`bitstream::write_sequence_header_payload` and `write_frame_header_payload`): their leading
  fields are structured bit-writes — the seq header's `seq_header_id`/profile/single-picture/
  level-chroma/frame-size, and the key-frame header's `cur_mfh_id`/`seq_header_id` `uvlc`
  preamble (with `write_frame_size` emitting no bits on the still-picture non-override path).
  The remaining tool/quant/loop-filter/tile flags and the range-coded tile data are still
  emitted as validated bootstrap constant tails. Both writers bit-exactly reproduce the
  previous fixed byte arrays, so `xtask validate` is unchanged.
- **`xtask validate`** — builds/caches native AVM decoder tools under `target/refdec`, runs
  `av2enc` on a generated deterministic frame, decodes through `avmdec`, checks decoded size,
  and checks MD5 `58efe7d34c4f36aab183bbf18a3f1e6a`.

## What is currently a stub (compiles, not yet functional)

- **Production encoder stages** — RD search, non-zero transform/quant/token coding, arbitrary
  geometry, inter prediction, filters, rate control, multiframe output, and 10-bit output are
  still deferred.
- **Partition trees / 256×256 block support** — intentionally deferred until real consumers
  need them.

## Environment facts (verified this session)

- Host: **Apple Silicon / arm64**, macOS (Darwin 27). rustc/cargo **1.95.0**.
- `cmake` 4.3.2 and `clang` are present → the native reference `avmdec` is buildable.
- `emcc` is **absent** → validation uses the **native** decoder, not the WASM demo path.
- Consequence: NEON kernels run/test natively here; **x86 (SSE/AVX2) asm can be compiled
  but not executed on this host** — rely on scalar-equivalence tests + CI/QEMU for x86.

## Known gotchas already discovered

- **Bypass literals are ≤8 bits per call.** `OdEcEnc::encode_literal_bypass` must be called
  with `n_bits <= 8` (the `normalize` flush emits at most two bytes). Wider values are
  chunked MSB-first into ≤8-bit pieces by `Writer::literal`, exactly like `avm_write_literal`.
- **rustfmt ignores `#[cfg]`.** A `#[cfg(feature=…)] mod generated;` made `cargo fmt` fail
  because the file didn't exist. Fix: a committed empty `generated/mod.rs` placeholder.
- **Don't `cd` inside compound Bash commands** in this harness (permission prompt); use
  `--manifest-path` or `git -C`. The shell CWD persists across calls.
- **Raw `.obu` files use Annex-B-style framing.** Each OBU in the file is prefixed by ULEB128
  size of `header + payload`, then the AVM OBU header, then payload. The internal encoder
  section-5 helper in AVM uses a different order before conversion.

## Next realistic work

The initial roadmap unblocker is complete and the **sequence-header and uncompressed
key-frame-header payloads now have real Rust writers** (their preambles are structured;
the tool/quant/tile flags and range-coded tile data remain validated constant tails). Next is
extending the frame-header writer to structure the remaining uncompressed fields
(quantization including `base_q_idx`, loop-filter/CDEF/LR/CCSO-off flags, `tx_mode`, tile
info) up to the byte boundary, then writing the **tile payload** with `av2-entropy::Writer`,
before growing the single-superblock path into real non-zero residual coding ahead of
arbitrary partition trees or inter prediction.
