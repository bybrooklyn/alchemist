# WhyTho?

`whytho.` is a standalone Rust workspace for a future media workflow CLI and
library. It follows the WhyTho? architecture charter and is intentionally
separate from the AGPLv3 Alchemist application in the repository root.

## License Boundary

- First-party `whytho-*` crates are licensed under Apache-2.0. See
  [`LICENSE`](LICENSE).
- `whytho-codec-av2` is a from-scratch Rust port of AVM reference *behavior*
  (module layout mirrors `avm/av2/encoder/*`), not verbatim AVM source — no BSD
  header or license file was ever present in it, and its `Cargo.toml` already
  inherits the workspace's Apache-2.0 license. It's the author's own code and
  is licensed Apache-2.0 like the rest of the first-party crates. (It absorbed
  the former `whytho-codec-av2-common` crate — see `docs/av2/ROADMAP.md`.)
- The other AV2-support crates (`whytho-dsp`, `whytho-entropy`, `whytho-tables`)
  are also ported (not verbatim) from the AVM reference; see each crate's
  `Cargo.toml` for current license status.
- The existing Alchemist application remains licensed under AGPL-3.0-or-later at
  the repository root.
- This workspace is not a member of the root Alchemist Cargo package. Use
  `--manifest-path whytho/Cargo.toml` when running Cargo commands for WhyTho.

### Absorbed vendored components

Three components were absorbed as hard forks — vendored in full, then developed
in place as part of this workspace rather than tracked against their upstream.
None of them carry their own nested Git history or upstream CI config anymore;
each is developed and versioned as part of this workspace going forward (see
[`docs/ROADMAP.md`](docs/ROADMAP.md) for the native-rewrite plan). Their original
licenses are preserved as-is, not relicensed — see [`NOTICE`](NOTICE) for the
short-form list, and each crate's own `README.md` for a fork notice pointing back
here.

- **`whytho-h264dec`** (H.264 decoder, crate name `rust_h264`) — forked from
  [roticv/rust_h264](https://github.com/roticv/rust_h264) at commit
  `b25c3e6a7d97994cd9f64a4f8da8833bdeefb220`, MIT OR Apache-2.0,
  Copyright (c) 2025 roticv. Both original license texts ship unmodified in
  the crate directory (`LICENSE-MIT`, `LICENSE-APACHE`).
- **`whytho-rav1e`** (AV1 encoder, crate/lib name `rav1e`) — forked from
  [xiph/rav1e](https://github.com/xiph/rav1e) at commit
  `564ae3b0007ae2b06893fd7166bf88c5a84c5b63`, BSD-2-Clause,
  Copyright (c) 2017-2023 the rav1e contributors. Also carries an Alliance for
  Open Media Patent License 1.0 grant (`PATENTS`), same as the AV2 components
  above. Original license text ships unmodified (`LICENSE`, `PATENTS`).
- **`whytho-opus`** (Opus audio codec, crate/lib name `opus_rs`) — forked from
  [restsend/opus-rs](https://github.com/restsend/opus-rs) at commit
  `11806b476361ccdd611b62d2a00e9c4fba05b4ec`, BSD-3-Clause. Copyright 2026
  restsend.com, plus the original IETF reference-implementation copyright it
  was ported from (Xiph.Org, Skype Limited, Octasic, Jean-Marc Valin, Timothy
  B. Terriberry, CSIRO, Gregory Maxwell, Mark Borgerding, Erik de Castro Lopo,
  Mozilla, Amazon — see `COPYING`, which also lists the royalty-free Opus
  patent grants from Xiph.Org, Microsoft, and Broadcom). Original license text
  ships unmodified as `COPYING` (this crate uses that filename instead of
  `LICENSE`).

## Crates

The workspace is layered bottom-up: a small contract crate, codec-agnostic shared
kernels, one crate per codec, a thin facade, then app policy and the CLI. Codec
crates depend only on the contract, the shared layers, and their backend — never on
app policy — so the dependency graph points one way.

**Contract & app**

- `whytho-types` - the codec contract: frame/packet types, encoder/decoder
  traits, codec/container enums, capability records. Tiny and dependency-light;
  every codec crate depends on it.
- `whytho-core` - app policy: media models, config primitives, planning
  vocabulary, verification, quality, scheduling, reporting, file-operation types.
- `whytho-cli` - user-facing `whytho` command shell.
- `whytho-backends` - backend traits and backend capability placeholders.

**Shared codec layers** — seeded from the AV2 work. `unsafe`/SIMD is confined to
`whytho-dsp`; every other crate is `#![forbid(unsafe_code)]`.

- `whytho-dsp` - DSP kernels (transforms, quantization, SAD, intra prediction,
  CDEF). The only crate that carries `unsafe`.
- `whytho-entropy` - entropy-coding substrate (od_ec range coder, bit writer,
  adaptive CDF model).
- `whytho-tables` - constant tables (CDFs, transform/scan/quant LUTs).

**Per-codec crates** — each depends only on `whytho-types` + the shared layers +
its backend.

- `whytho-codec-h264` - H.264 encoder and decoder.
- `whytho-codec-av1` - AV1 decoder and `rav1e`-backed encoder.
- `whytho-codec-av2` - AV2 encode pipeline, including its shared enum/frame
  types (`common` module — previously the separate `whytho-codec-av2-common`
  crate, merged in since nothing else depended on it).
- `whytho-codec-opus` - Opus audio encoder.

**Facade**

- `whytho-codecs` - re-exports the contract and, behind cargo features
  (`h264`/`av1`/`av2`/`opus`, all on by default), the per-codec crates, plus a
  capability registry. Consumers depend on this and select codecs via features.

## Status

This is a compileable architecture skeleton only. Real probing, planning,
chunked transcoding, backend execution, verification, and quality measurement
are future work.

## Development

```bash
cargo fmt --manifest-path whytho/Cargo.toml --all -- --check
cargo check --manifest-path whytho/Cargo.toml --workspace --all-targets
cargo test --manifest-path whytho/Cargo.toml --workspace

# build a slimmer facade with only the codecs you need
cargo check --manifest-path whytho/Cargo.toml -p whytho-codecs --no-default-features --features h264
```
