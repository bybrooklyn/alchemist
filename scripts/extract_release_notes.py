#!/usr/bin/env python3

from __future__ import annotations

import argparse
from pathlib import Path
import re


def extract_section(changelog: str, version: str) -> str:
    version = version.removeprefix("v")
    pattern = re.compile(
        rf"^## \[{re.escape(version)}\](?: - [^\n]+)?\n(?P<body>.*?)(?=^## \[|\Z)",
        re.MULTILINE | re.DOTALL,
    )
    match = pattern.search(changelog)
    if match is None:
        raise ValueError(f"CHANGELOG.md has no section for {version}")
    return f"## Alchemist {version}\n\n{match.group('body').strip()}\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    changelog = Path(args.changelog).read_text(encoding="utf-8")
    notes = extract_section(changelog, args.version)
    Path(args.output).write_text(notes, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
