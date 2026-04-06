# Releasing Alchemist

## RC cut

Use the repo bump script for version changes:

```bash
just bump <next-rc-version>
```

Then complete the release-candidate preflight:

1. Update `CHANGELOG.md` and `docs/docs/changelog.md`.
2. Run `just release-check`.
3. Verify the repo version surfaces all read `<next-rc-version>`.
4. Complete the manual smoke checklist:
   - Docker fresh install over plain HTTP, including login and first dashboard load
   - One packaged binary install and first-run setup
   - Upgrade from an existing `0.2.x` instance with data preserved
   - One successful encode, one skip, one intentional failure, and one notification test send
5. Complete the Windows contributor follow-up on a real Windows machine:
   - `just install-w`
   - `just dev`
   - `just check`
   - Note that broader utility and release recipes remain Unix-first unless documented otherwise.
6. Commit the release-prep changes and merge them to `main`.
7. Create the annotated tag `v<next-rc-version>` on the exact merged commit.

## Stable promotion

Promote to stable only after the RC burn-in is complete and the same automated preflight is still green.

1. Run `just bump 0.3.0`.
2. Update `CHANGELOG.md` and `docs/docs/changelog.md` for the stable cut.
3. Run `just release-check`.
4. Re-run the manual smoke checklist against the final release artifacts:
   - Docker fresh install
   - Packaged binary first-run
   - Upgrade from the most recent `0.2.x` or `0.3.0-rc.x`
   - Encode, skip, failure, and notification verification
5. Re-run the Windows contributor verification checklist if Windows parity changed after the last RC.
6. Confirm release notes, docs, and hardware-support wording match the tested release state.
7. Merge the stable release commit to `main`.
8. Create the annotated tag `v0.3.0` on the exact merged commit.
