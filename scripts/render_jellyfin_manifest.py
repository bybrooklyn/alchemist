#!/usr/bin/env python3

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path


PLUGIN_GUID = "c3637bc0-04ad-58b6-b11c-e840af0b1f6e"
TARGET_ABI = "10.11.10.0"


def md5(path: Path) -> str:
    hasher = hashlib.md5(usedforsecurity=False)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def load_manifest(path: Path | None) -> list[dict[str, object]]:
    if path is None or not path.exists():
        return []
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, list):
        raise ValueError("Jellyfin manifest root must be an array")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin-version", required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--zip", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--existing")
    parser.add_argument("--timestamp")
    parser.add_argument(
        "--changelog",
        default="First stable Alchemist Jellyfin plugin catalog release.",
    )
    args = parser.parse_args()

    archive = Path(args.zip)
    timestamp = args.timestamp or datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    version_entry = {
        "version": args.plugin_version,
        "changelog": args.changelog,
        "targetAbi": TARGET_ABI,
        "sourceUrl": (
            "https://github.com/bybrooklyn/alchemist/releases/download/"
            f"{args.release_tag}/{archive.name}"
        ),
        "checksum": md5(archive),
        "timestamp": timestamp,
    }

    existing_path = Path(args.existing) if args.existing else None
    manifest = load_manifest(existing_path)
    plugin = next(
        (entry for entry in manifest if entry.get("guid") == PLUGIN_GUID),
        None,
    )
    if plugin is None:
        plugin = {
            "guid": PLUGIN_GUID,
            "name": "Alchemist",
            "description": (
                "Forward Jellyfin media events to Alchemist and refresh Jellyfin "
                "after completed jobs."
            ),
            "overview": (
                "Integrates Jellyfin with Alchemist using a narrowed API token, "
                "path translations, and completed-job refresh events."
            ),
            "owner": "bybrooklyn",
            "category": "General",
            "versions": [],
        }
        manifest.append(plugin)

    versions = plugin.setdefault("versions", [])
    if not isinstance(versions, list):
        raise ValueError("Jellyfin plugin versions must be an array")
    plugin["versions"] = [
        version_entry,
        *(entry for entry in versions if entry.get("version") != args.plugin_version),
    ]

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
