# Releasing Alchemist

## RC cut

Use the repo bump script for version changes:

```bash
just bump <next-rc-version>
```

Then complete the release-candidate preflight:

1. Update `CHANGELOG.md` and `docs/docs/changelog.md`.
2. Run `just release-check`.
3. Confirm release signing is configured:
   - `ALCHEMIST_RELEASE_SIGNING_KEY_PEM` GitHub secret contains the Ed25519 private key used to sign `alchemist-update-manifest.json`.
   - `ALCHEMIST_UPDATE_PUBLIC_KEY_B64` GitHub variable contains the matching base64 public key embedded into release binaries.
   - With OpenSSL 3, generate/extract the key material with:
     `openssl genpkey -algorithm Ed25519 -out update-signing.pem` and
     `openssl pkey -in update-signing.pem -pubout -outform DER | tail -c 32 | base64`.
4. Verify the repo version surfaces all read `<next-rc-version>`.
5. Complete the manual smoke checklist:
   - Docker fresh install over plain HTTP, including login and first dashboard load
   - One packaged binary install and first-run setup
   - Manually install the attached Jellyfin plugin zip on `jellyfin/jellyfin:10.11.10`
   - Verify connection test, dry-run event handling, enqueue, path translation, SSE completion handling, and containing-directory refresh
   - Upgrade from an existing `0.2.x` instance with data preserved
   - One successful encode, one skip, one intentional failure, and one notification test send
6. Complete the Windows contributor follow-up on a real Windows machine:
   - `just install-w`
   - `just dev`
   - `just check`
   - Note that broader utility and release recipes remain Unix-first unless documented otherwise.
7. Commit the release-prep changes on `master`.
8. Create the annotated tag `v<next-rc-version>` on the exact merged commit.
9. Verify the `Release Smoke` workflow passes for Linux, Windows, macOS, Docker,
   and the manually installed Jellyfin plugin artifact.
10. Keep the RC in soak for at least seven days. Do not start another major
    transcoding feature or promote stable while any new P1/P2 issue is open.

## Stable promotion

Promote to stable only after a seven-day RC soak completes without a new P1/P2
issue and the same automated preflight is still green.

1. Run `just bump <stable-version>`.
2. Update `CHANGELOG.md` and `docs/docs/changelog.md` for the stable cut.
3. Run `just release-check`.
4. Confirm release signing secret/variable configuration still matches the active update key.
5. Re-run the manual smoke checklist against the final release artifacts:
   - Docker fresh install
   - Packaged binary first-run
   - Upgrade from the most recent supported stable or RC instance
   - Encode, skip, failure, and notification verification
   - Install the Jellyfin plugin from the stable repository feed and repeat the
     integration behavior checks against `jellyfin/jellyfin:10.11.10`
6. Re-run the Windows contributor verification checklist if Windows parity changed after the last RC.
7. Confirm release notes, docs, and hardware-support wording match the tested release state.
8. Commit the stable release changes on `master`.
9. Create the annotated tag `v<stable-version>` on the exact merged commit.

Stable releases publish the Jellyfin plugin zip, MD5, SHA-256, and update:

```text
https://raw.githubusercontent.com/bybrooklyn/alchemist/jellyfin-plugin-repo/manifest.json
```

RC releases attach the same manually installable plugin assets but must not
update the stable feed.

The Jellyfin release archive is named
`dev.bybrooklyn.alchemist_<plugin-version>.zip`; its MD5 and SHA-256 checksum
files use the same base name.
