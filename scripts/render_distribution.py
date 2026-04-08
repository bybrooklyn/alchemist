#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


def sha256(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def render_template(template: str, replacements: dict[str, str]) -> str:
    rendered = template
    for key, value in replacements.items():
        rendered = rendered.replace(f"{{{{{key}}}}}", value)
    return rendered


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--assets-dir", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    assets_dir = Path(args.assets_dir).resolve()
    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    replacements = {
        "VERSION": args.version,
        "LINUX_X86_64_SHA256": sha256(assets_dir / "alchemist-linux-x86_64.tar.gz"),
        "MACOS_X86_64_SHA256": sha256(assets_dir / "alchemist-macos-x86_64.tar.gz"),
        "MACOS_ARM64_SHA256": sha256(assets_dir / "alchemist-macos-arm64.tar.gz"),
    }

    templates = [
        (
            root / "packaging/homebrew/alchemist.rb.tmpl",
            output_dir / "homebrew/alchemist.rb",
        ),
        (
            root / "packaging/aur/PKGBUILD.tmpl",
            output_dir / "aur/PKGBUILD",
        ),
    ]

    for template_path, output_path in templates:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            render_template(template_path.read_text(), replacements),
            encoding="utf-8",
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
