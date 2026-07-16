# Reference material map

These sibling folders are **read-only aids**, never runtime dependencies of the encoder.
Paths are relative to the parent of `rs/` (i.e. `/Users/brooklyn/data/av2-rs/`).

## `../avm` — the official AV2 reference codec (C, "AVM")

The ground truth for algorithms, table values, and the decoder we validate against.

Key files we have already mined (and will keep returning to):

| Area | Files |
|------|-------|
| Range coder | `avm_dsp/entenc.c`, `entdec.{c,h}`, `entcode.{c,h}`, `prob.h`, `bitwriter.h` |
| Bitstream/OBU | `av2/encoder/bitstream.c` (`write_sequence_header`≈4894, `write_uncompressed_header`≈5190, `av2_write_obu_header`≈5888), `av2/common/obu_util.{h,c}` |
| Encoder pipeline | `av2/encoder/{encoder,encode_strategy,encodeframe,partition_search,intra_mode_search,rdopt,mcomp}.c` |
| Transforms/quant | `av2/encoder/{av2_fwd_txfm2d,hybrid_fwd_txfm,av2_quantize,trellis_quant}.c`; `av2/common/quant_common.h` |
| Tokenize/coeffs | `av2/encoder/{tokenize,encodetxb}.c` |
| In-loop filters | `av2/encoder/{picklpf,pickcdef,pickrst,pickccso}.c`; `av2/common/{cdef,restoration}.c` |
| Enums/types | `av2/common/{enums.h,blockd.h,entropy.c,entropymode.c,entropymv.c}`, `av2/encoder/{block.h,context_tree.h,enc_enums.h}` |
| CDF init tables | `av2/common/{entropy_inits_coeffs.h,entropy_inits_modes.h,entropy_inits_mv.h}` |
| SIMD inventory | `avm_dsp/avm_dsp_rtcd_defs.pl`, `av2/common/av2_rtcd_defs.pl` (Perl DSL listing every DSP fn + its sse2/avx2/neon specializations) |
| Decoder (validation) | `apps/avmdec.c`, `examples/{simple_decoder,decode_to_md5}.c`, public API `avm/avm_decoder.h`, `avm/avmdx.h` |
| Build | `CMakeLists.txt`, `cmake/{avm_configure,avm_optimization,rtcd}.cmake` |
| Inspection tools | `tools/{dump_obu.cc,obu_parser.cc,avm_analyzer/}` |

How SIMD is organized in AVM (for porting guidance): a Perl "RTCD" (run-time CPU detect)
system. `*_rtcd_defs.pl` declares each function and `specialize qw/name sse2 avx2 neon/`;
at build time `rtcd.pl` generates function-pointer dispatch. Our Rust equivalent is the
`cpu.rs` + per-family dispatch pattern (see [DSP_ASM.md](DSP_ASM.md)).

## `../av2-spec` — the AV2 specification

- `v1.0.0/20260528_38f28e7_AV2_Spec_v1.0.0.pdf` — the normative spec (3.9 MB). Use `Read`
  with a `pages` range for exact syntax tables.
- `v1.0.0/syntax_browser.html` / `index.html` — interactive syntax viewer.
- **`v1.0.0/attachments/` — 246 C-array header files** = the porting goldmine for
  `av2-tables`: ~180 `default_*_cdf.h`, ~11 `*_kernel*.h` (dct/adst/ddtx/fdst), `all_tables.h`
  (1.7 MB consolidated), scan orders (`stx_scan_map.h`), size LUTs, `quantizer_matrix.h`,
  `prob_inc.h` (already transcribed into `av2-entropy`), `mode_to_*`, `warped_filters.h`, etc.
  Format: bare initializers like `Name[d0][d1] = {…};` — mechanically convertible to Rust.
- `v13-public/` — an older draft; prefer `v1.0.0`.

## `../av2_demo` — WASM demo (reference only)

A separate project (`av2codec`/`wav2c`) that encodes an image to AV2 in Rust→WASM and
decodes with the reference decoder in the browser. We do **not** base our design on it, but
it is a useful example of: RGB→YUV420 conversion (`src/lib.rs`), and a minimal C decode
harness (`decoder_wasm/avmdec_wasm.c`) showing the `avm_codec_dec_init` →
`avm_codec_decode` → `avm_codec_get_frame` flow.

## How to look things up efficiently

- Spec syntax for a field → open the PDF at the relevant page, or `syntax_browser.html`.
- Exact algorithm/encoding → read the AVM C file named in the module's top-of-file comment.
- A table's values → the matching `attachments/*.h`, cross-checked against AVM's
  `entropy_inits_*.h` where applicable.
