# AGENTS.md

Follow `../CLAUDE.md` and `../AGENTS.md` for repository-wide discipline. This
file adds instructions specific to the `whytho/` workspace.

## Orientation

- At the start of WhyTho work, read `../CHANGELOG.md`, `../VERSION`,
  `../CLAUDE.md`, this file, `README.md`, and the relevant sections of
  `Spec.md`.
- Treat `Spec.md` as the WhyTho architecture and product source of truth.
  Update it when architecture, roadmap, or product decisions change.
- Treat `README.md` as the current workspace, license-boundary, crate-layout,
  and development-command summary.
- Use `whytho.` in code-facing contexts. `WhyTho?` may be used as the
  title/product spelling.

## Workspace Boundary

- `whytho/` is a standalone Rust workspace and is not a member of the root
  Alchemist Cargo package. From the repository root, use
  `--manifest-path whytho/Cargo.toml` for Cargo commands.
- Keep Alchemist-specific behavior out of `whytho-*` crates and WhyTho docs.
  Alchemist is one possible consumer, not the product center.
- Preserve the license boundary: first-party `whytho-*` crates are Apache-2.0,
  AV2-derived crates keep their BSD-3-Clause-Clear lineage, and root Alchemist
  remains AGPL-3.0-or-later.
- The root reverse-domain identifier `dev.bybrooklyn.alchemist` is for
  Alchemist packaging and plugin identity. Do not apply it to WhyTho crate,
  CLI, or artifact identity unless a platform integration explicitly requires
  an Alchemist-owned component suffix.

## Architecture Rules

- Do not reintroduce the rejected plugin, dynamic extension, marketplace, or
  scripting-runtime architecture. Use normal Rust crates, traits, config
  structs, feature flags, and built-in presets.
- Keep the core model clear: WhyTho performs media work; apps decide product
  behavior, storage policy, replacement prompts, UI, and database state.
- File operations must be explicit and reversible by default. Do not hide
  destructive replacement behavior behind defaults.
- `whytho plan` is the primary dry-run and explanation command. Prefer
  deterministic planning and explicit reports over implicit heuristics.
- Keep codecs and backends behind shared traits. Backend details should not
  leak into high-level app policy.
- Preserve the crate layering described in `README.md`: contract types, shared
  codec kernels, per-codec crates, facade, core policy, backends, and CLI.
- Keep `unsafe` confined to `whytho-dsp` unless `Spec.md` and the crate
  contract are updated with a deliberate exception.

## Development

From the repository root, use:

```bash
cargo fmt --manifest-path whytho/Cargo.toml --all -- --check
cargo check --manifest-path whytho/Cargo.toml --workspace --all-targets
cargo test --manifest-path whytho/Cargo.toml --workspace
```

From inside `whytho/`, the equivalent commands are:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
```

- Run the narrowest gate that proves the change, and run the broader workspace
  gates when touching shared contracts, feature flags, codec facades, or public
  CLI behavior.
- Do not claim real probing, planning, transcoding, verification, or quality
  behavior unless it is implemented and verified. The current workspace may be
  a compileable architecture skeleton in those areas.
- If adding substantial automation, put helper scripts under `whytho/scripts/`
  and keep any root `justfile` recipes as thin wrappers.
- Treat vendored or upstream-derived codec code as externally sourced: avoid
  broad rewrites, style churn, or license/header changes unless that is the
  explicit task.
