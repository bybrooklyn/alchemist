# Bitstream notes (for the keyframe writer, ROADMAP §6)

Facts gathered from exploring `avm/av2/encoder/bitstream.c`, `avm/av2/common/obu_util.{h,c}`,
`avm/avm/avm_codec.h`, and the spec. These guide `av2-encoder/src/bitstream.rs`.

## OBU framing

Raw `.obu` files consumed by AVM use Annex-B-style framing:

```
ULEB128(header_size + payload_size)
OBU header
payload
```

AVM's internal section-5 writer first constructs `header + payload_size + payload`, then
`av2_convert_sect5obus_to_annexb` converts that representation before raw `.obu` output.
`av2-rs` writes the raw-file form directly.

OBU header byte (MSB→LSB), per `av2_write_obu_header`:
- `obu_extension_flag` (1 bit)
- `obu_type` (5 bits)
- temporal layer id (2 bits)
- optional extension byte when `obu_extension_flag=1`: embedded layer id

OBU types (`avm/avm/avm_codec.h`): `OBU_SEQUENCE_HEADER=1`, `OBU_TEMPORAL_DELIMITER=2`,
`OBU_CLOSED_LOOP_KEY=4`, `OBU_OPEN_LOOP_KEY=5`, `OBU_REGULAR_TILE_GROUP=7`, … (AV2 has its
own enum — use these values, which differ from AV1).

Sizes are ULEB128 (`uleb`). Implement a `write_uleb128(buf, value)`.

## Minimal decodable keyframe sequence

```
OBU_TEMPORAL_DELIMITER (2)   payload empty
OBU_SEQUENCE_HEADER    (1)   sequence header payload
<key-frame OBU>              uncompressed frame header + range-coded tile data
```
The key-frame OBU is either `OBU_CLOSED_LOOP_KEY (4)` carrying header+tile, or a frame
header + `OBU_REGULAR_TILE_GROUP (7)` split. Start with whichever the reference decoder
accepts most simply; cross-check by decoding with `avmdec`.

Current seed stream:
- TD: size `01`, header `08`, empty payload.
- Sequence header: size `0d`, header `04`, payload
  `82 0a 66 ff fc 70 e7 77 91 b8 08 80` — emitted by `bitstream::write_sequence_header_payload`
  (structured leading fields + a validated 50-bit tool-flag constant + trailing bits).
- Closed-loop key: size `08`, header `10`, payload `e2 40 0f 00 2f 2f 5c` — emitted by
  `bitstream::write_frame_header_payload`. The `cur_mfh_id`/`seq_header_id` `uvlc` preamble is
  structured (`single_picture_header_flag = 1`, so `write_frame_size` emits no bits); the
  remaining 54 bits (frame-header tool/quant/tile flags + byte-aligned range-coded tile data)
  are a validated constant `0x0022_400f_002f_2f5c` until each field group gets a Rust writer.
- Total size: 25 bytes; decoded MD5:
  `58efe7d34c4f36aab183bbf18a3f1e6a  img-128x128-0001.i420`.

## Two writers needed

1. **Uncompressed bit-buffer** (MSB-first into bytes) for the sequence header and the
   uncompressed frame header. Mirror `struct avm_write_bit_buffer` (`aom_wb_write_bit`,
   `aom_wb_write_literal`, `aom_wb_write_uvlc`). This is NOT the arithmetic coder.
2. **Range coder** (`av2-entropy::Writer`) for the tile payload (modes, partitions,
   coefficients), using default CDFs from `av2-tables`.

## Sequence header fields (see `write_sequence_header`, bitstream.c ≈ 4894)

profile, `seq_level`, `bit_depth` (8/10), `monochrome=0`, color config (primaries/transfer/
matrix or "unspecified"), `subsampling_x=subsampling_y=1` (4:2:0), max frame width/height
(minus 1, with N-bit fields), `use_128x128_superblock`, and a series of **tool-enable
flags** — set these conservatively (advanced tools OFF) so the first bitstream uses the
smallest feature set the decoder will accept. Reference frame / order-hint info as required.

## Frame header fields (KEY_FRAME) (see `write_uncompressed_header`, bitstream.c ≈ 5190)

`frame_type=KEY_FRAME`, `show_frame=1`, `frame_size_override` (+ width/height if set),
render size, `base_q_idx` (the QIndex), Y/U/V dc/ac delta-q (0 initially), segmentation OFF,
delta-q/delta-lf OFF, loop filter levels 0, CDEF OFF, loop restoration OFF, CCSO OFF,
`tx_mode`, single tile (`tile_cols=tile_rows=1`, so no tile-size signaling beyond the
defaults). KEY frames need no reference handling.

## Quantization

`base_q_idx` plus dc/ac deltas index dequant LUTs (`av2_dc_quant_QTX`/`av2_ac_quant_QTX`,
`avm/av2/common/quant_common.h`). Port those LUTs into `av2-tables`. The default CDF set for
coefficient coding is chosen by Q-context (`get_q_ctx`, see [ENTROPY.md](ENTROPY.md)).

## Strategy

Get the OBU framing + sequence header + frame header byte-correct **first** (decoder parses
them before any tile data), with a trivial tile payload (all-skip / DC / zero coefficients).
Iterate against `avmdec` until it decodes without error. Then improve the tile content.
Use `avm/tools/dump_obu.cc` / the analyzer to inspect framing while debugging.
