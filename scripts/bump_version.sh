#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>" >&2
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

# web/package.json
pkg = root / "web" / "package.json"
text = pkg.read_text()
# Strip unexpected control characters that can break JSON parsing.
text = "".join(ch for ch in text if ch == "\n" or ch == "\t" or ch == "\r" or ord(ch) >= 32)
new_text, count = re.subn(
    r'(?m)^(\s*"version"\s*:\s*)"[^"]+"',
    f'\\1"{version}"',
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
        raise SystemExit("Failed to update web/package.json version field.")
pkg.write_text(new_text)

# CHANGELOG.md (top entry)
changelog = root / "CHANGELOG.md"
text = changelog.read_text()
text = re.sub(r'(?m)^## \[v[^\]]+\]', f'## [v{version}]', text, count=1)
changelog.write_text(text)

# docs/Documentation.md footer + latest changelog entry
docs = root / "docs" / "Documentation.md"
text = docs.read_text()
text = re.sub(r'(?m)^\*Documentation for Alchemist v[^*]+\*', f'*Documentation for Alchemist v{version}*', text, count=1)
# Update the first changelog entry version after the Changelog header
m = re.search(r'(## Changelog\n\n)(### v[^\n]+)', text)
if m:
    prefix = m.group(1)
    text = text.replace(m.group(2), f'### v{version}', 1)

docs.write_text(text)

print(f"Updated version to {version}")
PY

read -r -p "Create git tag (e.g., v${VERSION}) or leave blank to skip: " TAG
if [ -n "$TAG" ]; then
  git -C "$ROOT_DIR" tag -a "$TAG" -m "$TAG"
  echo "Created tag: $TAG"
fi
