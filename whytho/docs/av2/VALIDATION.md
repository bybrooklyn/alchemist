# Validation

Spec compliance is **not** self-asserted: we prove it by decoding our encoder's output with
the official **AVM reference decoder**, built natively from `../avm`. (The WASM path in
`../av2_demo` is unavailable here because `emcc` is not installed.)

## Two layers of validation

1. **Unit-level bit-exactness** (already in place / extend as we go):
   - Entropy: round-trip vs a faithful port of the reference decoder (see [ENTROPY.md]).
   - DSP: every asm kernel vs its scalar baseline (see [DSP_ASM.md]).
   - Tables: spot-checks vs known spec values.
2. **End-to-end spec compliance** (ROADMAP §7): encode → decode with `avmdec` → compare.

## `xtask build-refdec`

Out-of-tree CMake build of the reference decoder:
```
cmake -S ../avm -B target/refdec -DCMAKE_BUILD_TYPE=Release \
      -DCONFIG_AV2_DECODER=1 -DCONFIG_AV2_ENCODER=0 \
      -DENABLE_APPS=1 -DENABLE_EXAMPLES=1 -DENABLE_TESTS=0
cmake --build target/refdec --target avmdec decode_to_md5 -j 8
```
Cache lives under `rs/target/refdec/`. Tooling present on this host: `cmake` 4.3.2, `clang`.
Use `cargo run -p xtask -- build-refdec --force` to force reconfiguration/rebuild.

## `xtask validate`

```
cargo run -p xtask -- validate
```
Current gate:
- Generates `target/validation/gray128.y4m`.
- Builds `target/debug/av2enc`.
- Encodes `target/validation/gray128.obu`.
- Runs `target/refdec/avmdec --rawvideo`.
- Asserts decoded output length is 24,576 bytes for 128×128 I420.
- Runs `target/refdec/examples/decode_to_md5` and checks
  `58efe7d34c4f36aab183bbf18a3f1e6a`.

This proves the current skeleton stream decodes with native AVM. It does not prove visual
quality or general encoder correctness.

## Debugging framing

While getting OBU/headers byte-correct, inspect with the reference tools:
- `avm/tools/dump_obu.cc` (build via the AVM CMake) — dumps OBU structure.
- `avm/tools/avm_analyzer/` — a Rust+web bitstream analyzer.

## Test fixtures

Fixtures are generated under `target/validation/`; no binary Y4M/YUV/OBU artifacts are
committed.
