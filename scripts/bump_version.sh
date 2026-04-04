#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>  # example: 0.2.10 or 0.2.10-rc.1" >&2
  exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

printf '%s\n' "$VERSION" > "$ROOT_DIR/VERSION"

perl -0pi -e '
  BEGIN { $version = shift @ARGV }
  s/^version\s*=\s*"[^"]+"/version = "$version"/m
    or die "Failed to update Cargo.toml version\n";
' "$VERSION" "$ROOT_DIR/Cargo.toml"

while IFS= read -r package_json; do
  perl -0pi -e '
    BEGIN { $version = shift @ARGV }
    s/[\x00-\x08\x0B\x0C\x0E-\x1F]//g;
    if (!s/^(\s*"version"\s*:\s*)"[^"]+"/${1}"$version"/m) {
      s/^(\s*"name"\s*:\s*"[^"]+",\n)/${1}  "version": "$version",\n/m
        or die "Failed to update version field\n";
    }
  ' "$VERSION" "$package_json"
done < <(
  find "$ROOT_DIR" \
    \( -path '*/node_modules/*' -o -path '*/dist/*' -o -path '*/.astro/*' \) -prune \
    -o -type f -name 'package.json' -print | sort
)

perl -0pi -e '
  BEGIN { $version = shift @ARGV }
  s/(\[\[package\]\]\nname = "alchemist"\nversion = )"[^"]+"/${1}"$version"/
    or die "Failed to update Cargo.lock version\n";
' "$VERSION" "$ROOT_DIR/Cargo.lock"

echo "Updated version to $VERSION"

cat <<EOF
Next steps:
  1. Update CHANGELOG.md and docs/docs/changelog.md for ${VERSION}
  2. Run just release-check
  3. Complete the manual RC/stable smoke checklist in RELEASING.md
  4. Merge the release-prep commit to main so Docker publishes ${VERSION}
  5. Stable versions also publish latest; prereleases must not
  6. Create annotated tag v${VERSION} on the exact merged commit for binary releases

Note: this script no longer creates git tags. Tags must be created separately after merge.
EOF
