## [Unreleased]

- Updated the codec and tooling dependency set, including `av-scenechange`
  0.15, `bitstream-io` 4.10, current parser/metrics/trace dependencies, and
  refreshed standalone and fuzz lockfiles.
- Adapted the scene-change detector construction to the 0.15 API, which removed
  the obsolete runtime CPU-feature argument.
- Made the fuzz target's local dependency alias explicit for the renamed
  `whytho-rav1e` package.

## Version 0.6.0

- See https://github.com/xiph/rav1e/projects/20
