# Entropy coder (`av2-entropy`)

The single most important substrate: the AV2 multi-symbol arithmetic (range) coder. Every
byte the encoder emits in the tile payload goes through it, so it must be **bit-exact** with
the reference. It is a direct port of the Daala/AV1 `od_ec` coder.

Reference files: `avm/avm_dsp/entenc.c` (encoder), `entdec.{c,h}` (decoder, used in our
tests), `entcode.{c,h}` (`av2_prob_inc_tbl`, `od_ec_window`), `prob.h` (constants,
`od_ec_prob_scale`, `update_cdf`), `bitwriter.h` (`avm_write_*`).

## Constants (from `prob.h` / `entcode.h`)

| Name | Value | Meaning |
|------|-------|---------|
| `CDF_PROB_BITS` | 15 | probabilities are Q15 |
| `CDF_PROB_TOP` | `1<<15` = 32768 | CDF top |
| `CDF_SHIFT` | `15 - CDF_PROB_BITS` = 0 | (0 for AV2) |
| `EC_PROB_SHIFT` | 7 | range-scaling shift |
| `OD_ICDF(x)` | `CDF_PROB_TOP - x` | inverse-CDF representation (its own inverse) |
| `od_ec_window` | `u64` | the `low`/`dif` accumulator type |

`od_ec_prob_scale(p, r, n, nsym)` (the quantized-multiply at the heart of every symbol):
```
rr = r >> 8
pp = ((p >> 7) << 4) + PROB_INC_TBL[nsym-2][n]
return ((rr * pp) >> 7) << 3        # shift = 7 - EC_PROB_SHIFT - CDF_SHIFT + 1 + 6 = 7
```
`PROB_INC_TBL` is the `15×16` `av2_prob_inc_tbl` from `entcode.c`. Its `-1` sentinels are
never indexed for a valid symbol (`n` only ranges over `0..nsym`, all non-negative there),
so we store it as `i16` and it is still bit-exact for all valid inputs.

## Encoder state (`OdEcEnc`, port of `od_ec_enc`)

- `precarry: Vec<u16>` — buffered output bytes (with carry room); the reference `offs` is
  just `precarry.len()`. Carry propagation happens in `done()`.
- `low: u64`, `rng: u16` (init `0x8000`), `cnt: i16` (init `-9`), `error: bool`.

Key methods (all ports; see source for the line-by-line mapping):
- `encode_cdf_q15(s, icdf, nsyms)` → `encode_q15` → `normalize`.
- `encode_bool_q15(val, f)` — binary symbol with explicit Q15 prob.
- `encode_literal_bypass(val, n_bits)` — **`n_bits` must be 1..=8** (see gotcha below).
- `done() -> Vec<u8>` — finalize with carry propagation; operates on a clone of `precarry`
  so the encoder may keep coding afterward (matches the reference semantics).
- `tell()`, `patch_initial_bits(val, nbits)`, `checkpoint()`/`rollback(cp)` (for trial RD).

`normalize(low, rng, n_bypass)`: `d = (rng as u16).leading_zeros()` implements
`16 - OD_ILOG_NZ(rng)`. It flushes at most **two** bytes per call.

## CDF adaptation (`cdf.rs`)

`update_cdf(cdf, val, nsyms)` ports `prob.h`'s `update_cdf`. The adaptive CDF array layout is
`CDF_SIZE(n) = n + 4` u16 slots: `n` inverse-CDF entries (last is 0), a counter at `[n]`, and
**three PARA rate offsets** at `[n+1..=n+3]` (time intervals 0/1/2). The rate is
`2 + cdf[n+1+time_interval]`; `time_interval` is chosen by the counter (`>31`→2, `>15`→1,
else 0). The last inverse-CDF entry is never modified; the counter saturates at 32.

> The PARA values live in the default CDF tables (the `AVM_PARAn` macros). Until `av2-tables`
> is generated, adaptive coding can be tested with hand-picked PARA slots (the tests do this);
> real default CDFs come from §4 of the ROADMAP.

## Writer (`writer.rs`, port of `bitwriter.h`)

`Writer` wraps `OdEcEnc`:
- `symbol(s, cdf, nsyms)` — adaptive: codes then `update_cdf` (if `allow_update_cdf`).
- `symbol_fixed(s, icdf, nsyms)` — non-adaptive.
- `bool_prob(val, f)` — explicit-probability bit.
- `literal(val, nbits)` — **chunks into ≤8-bit pieces MSB-first**, exactly like
  `avm_write_literal`, because the bypass primitive caps at 8 bits.
- `bit(val)` — single bypass bit.
- `finish() -> Vec<u8>`.

## GOTCHA: bypass literals are ≤8 bits per call

`avm_write_literal` loops `n = min(bits, 8)` calling `od_ec_encode_literal_bypass(data >>
(bits-n), n)`. The encoder `normalize` only flushes two bytes, so a single call with >8 bits
corrupts the stream. Our `encode_literal_bypass` asserts `n_bits <= 8`; `Writer::literal`
does the chunking. (This was found via a round-trip test failure during the port.)

## Tests (the bit-exactness gate)

`entenc.rs` contains a **faithful port of the reference decoder** (`OdEcDec`, from
`entdec.{c,h}`) used only in tests. Tests:
- `roundtrip_mixed_random_ops` — 64 seeds × hundreds of mixed symbol/bool/literal ops;
  encode → `done()` → decode → assert every symbol recovered.
- `writer_wide_literals_roundtrip` — `Writer::literal` for widths 1..=32 (forces chunking).
- `checkpoint_rollback_restores_state`, `tell_is_monotonic_nondecreasing`,
  `empty_stream_is_decodable`.
- `cdf.rs`: `update_preserves_invariants`, `favored_symbol_shifts_mass`.

When the validation harness (§7) lands, add a test that compares encoder bytes against a
trace captured from the actual C reference for a fixed symbol script (ultimate ground truth).
