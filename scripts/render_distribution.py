#!/usr/bin/env python3

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


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


def channel_for_version(version: str, release_tag: str) -> str:
    combined = f"{version} {release_tag}".lower()
    if "nightly" in combined:
        return "nightly"
    if "-rc" in combined:
        return "rc"
    return "stable"


def asset_size(path: Path) -> int:
    return path.stat().st_size


def release_asset_url(release_tag: str, filename: str) -> str:
    return (
        "https://github.com/bybrooklyn/alchemist/releases/download/"
        f"{release_tag}/{filename}"
    )


def sign_payload(payload_bytes: bytes, signing_key_env: str) -> str:
    raw_key = os.environ.get(signing_key_env)
    if not raw_key:
        raise RuntimeError(f"{signing_key_env} is required to sign update manifests")

    key_pem = raw_key.replace("\\n", "\n")
    if "BEGIN" not in key_pem:
        key_pem = base64.b64decode(raw_key).decode("utf-8")

    with tempfile.NamedTemporaryFile("w", encoding="utf-8") as key_file:
        key_file.write(key_pem)
        key_file.flush()
        with tempfile.NamedTemporaryFile("wb") as payload_file:
            payload_file.write(payload_bytes)
            payload_file.flush()
            result = subprocess.run(
                [
                    "openssl",
                    "pkeyutl",
                    "-sign",
                    "-rawin",
                    "-inkey",
                    key_file.name,
                    "-in",
                    payload_file.name,
                ],
                check=True,
                capture_output=True,
            )
    return base64.b64encode(result.stdout).decode("ascii")


def render_update_manifest(
    *,
    version: str,
    release_tag: str,
    assets_dir: Path,
    output_dir: Path,
    signing_key_env: str,
) -> None:
    asset_specs = [
        ("linux", "x86_64", "alchemist-linux-x86_64.tar.gz"),
        ("linux", "aarch64", "alchemist-linux-aarch64.tar.gz"),
        ("macos", "x86_64", "alchemist-macos-x86_64.tar.gz"),
        ("macos", "aarch64", "alchemist-macos-arm64.tar.gz"),
        ("windows", "x86_64", "alchemist-windows-x86_64.exe"),
    ]
    assets = []
    for os_name, arch, filename in asset_specs:
        path = assets_dir / filename
        assets.append(
            {
                "os": os_name,
                "arch": arch,
                "filename": filename,
                "url": release_asset_url(release_tag, filename),
                "sha256": sha256(path),
                "size": asset_size(path),
            }
        )

    signed = {
        "schema_version": 1,
        "channel": channel_for_version(version, release_tag),
        "version": version,
        "release_url": f"https://github.com/bybrooklyn/alchemist/releases/tag/{release_tag}",
        "assets": assets,
    }
    payload = json.dumps(signed, separators=(",", ":")).encode("utf-8")
    manifest = {
        "signed": signed,
        "signature": sign_payload(payload, signing_key_env),
    }
    (output_dir / "alchemist-update-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--assets-dir", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument(
        "--signing-key-env",
        default="ALCHEMIST_RELEASE_SIGNING_KEY_PEM",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    assets_dir = Path(args.assets_dir).resolve()
    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    replacements = {
        "VERSION": args.version,
        "RELEASE_TAG": args.release_tag,
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

    render_update_manifest(
        version=args.version,
        release_tag=args.release_tag,
        assets_dir=assets_dir,
        output_dir=output_dir,
        signing_key_env=args.signing_key_env,
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
