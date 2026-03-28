#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>  # example: 0.2.10 or 0.2.10-rc.1" >&2
  exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - <<PY
from pathlib import Path
import re

root = Path("$ROOT_DIR")
version = "$VERSION"

# VERSION file
(root / "VERSION").write_text(version)

# Cargo.toml
cargo = root / "Cargo.toml"
text = cargo.read_text()
text = re.sub(r'(?m)^version\s*=\s*"[^"]+"', f'version = "{version}"', text, count=1)
cargo.write_text(text)

def update_package_json(pkg: Path) -> None:
    text = pkg.read_text()
    # Strip unexpected control characters that can break JSON parsing.
    text = "".join(ch for ch in text if ch == "\n" or ch == "\t" or ch == "\r" or ord(ch) >= 32)
    new_text, count = re.subn(
        r'(?m)^(\s*"version"\s*:\s*)"[^"]+"',
        rf'\1"{version}"',
        text,
        count=1,
    )
    if count == 0:
        new_text, count = re.subn(
            r'(?m)^(\s*"name"\s*:\s*"[^"]+",\s*)$',
            r'\1  "version": "' + version + r'",\n',
            text,
            count=1,
        )
        if count == 0:
            raise SystemExit(f"Failed to update {pkg.relative_to(root)} version field.")
    pkg.write_text(new_text)


# package.json files across the repo
for pkg in sorted(root.rglob("package.json")):
    relative = pkg.relative_to(root)
    if any(part in {"node_modules", "dist", ".astro"} for part in relative.parts):
        continue
    update_package_json(pkg)

# Cargo.lock (root package entry)
lock = root / "Cargo.lock"
text = lock.read_text()
text = re.sub(
    r'(?ms)(\[\[package\]\]\nname = "alchemist"\nversion = )"[^"]+"',
    rf'\1"{version}"',
    text,
    count=1,
)
lock.write_text(text)

print(f"Updated version to {version}")
PY

cat <<EOF
Next steps:
  1. Update CHANGELOG.md and redoc/reference/changelog.md for v${VERSION}
  2. Run cargo test --quiet
  3. Run bun run typecheck && bun run build (in web/)
  4. Run bun run test:reliability (in web-e2e/)
  5. Merge the release-prep commit to main so Docker publishes ${VERSION}
  6. Stable versions also publish latest; prereleases must not
  7. Create annotated tag v${VERSION} on that exact merged commit for binary releases

Note: this script no longer creates git tags. Tags must be created separately after merge.
EOF
