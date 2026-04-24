---
name: bump
description: Bump repo version files for a release-prep change. Usage: /bump <VERSION>.
---

# /bump <VERSION>

Run the repo version bump flow without creating a tag.

## Behavior

1. Require a version argument.
2. Accept either `0.3.2` or `v0.3.2`, but normalize it to bare `0.3.2` before running any command.
3. Run:
   - `just bump <NORMALIZED_VERSION>`
4. Report the command output and the expected follow-up:
   - update `CHANGELOG.md` and `docs/docs/changelog.md`
   - run `just release-check`
   - complete the manual checklist in `RELEASING.md`
   - commit and merge the release-prep change
   - only then create annotated tag `v<NORMALIZED_VERSION>` on the exact merged commit

## Guardrails

- Do not guess version when argument missing; ask user for the exact version.
- Do not pass a leading `v` to `just bump`; strip it first.
- If `just bump` fails, stop and do not suggest that the release-prep bump succeeded.
- Do not create a git tag as part of `/bump`.
- If the user later asks to create the release tag, use an annotated tag named `v<NORMALIZED_VERSION>` on the merged release commit, not a lightweight `<VERSION>` tag on the pre-merge commit.
- Do not push commits or tags automatically.
