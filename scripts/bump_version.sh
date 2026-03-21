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

# web/package.json
pkg = root / "web" / "package.json"
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
        raise SystemExit("Failed to update web/package.json version field.")
pkg.write_text(new_text)

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

# docs/Documentation.md footer
docs = root / "docs" / "Documentation.md"
text = docs.read_text()
text = re.sub(r'(?m)^\*Documentation for Alchemist v[^*]+\*', f'*Documentation for Alchemist v{version}*', text, count=1)
docs.write_text(text)

print(f"Updated version to {version}")
PY

cat <<EOF
Next steps:
  1. Update CHANGELOG.md and docs/Documentation.md release notes for v${VERSION}
  2. Run cargo test --quiet
  3. Run bun run verify (in web/)
  4. Run bun run test:reliability (in web-e2e/)
  5. Merge the release-prep commit to main so Docker publishes ${VERSION}
  6. Stable versions also publish latest; prereleases must not
  7. Create annotated tag v${VERSION} on that exact merged commit for binary releases

Note: this script no longer creates git tags. Tags must be created separately after merge.
EOF
