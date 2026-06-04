#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "integrations/jellyfin/Alchemist.Jellyfin/Alchemist.Jellyfin.csproj"
VERSION_FILE = ROOT / "integrations/jellyfin/PLUGIN_VERSION"
PLUGIN_ID_FILE = ROOT / "integrations/jellyfin/PLUGIN_ID"


def digest(path: Path, algorithm: str) -> str:
    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def write_checksum(path: Path, algorithm: str, output: Path) -> None:
    output.write_text(f"{digest(path, algorithm)}  {path.name}\n", encoding="ascii")


def package(output_dir: Path, version: str, plugin_id: str) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="alchemist-jellyfin-") as temp:
        publish_dir = Path(temp) / "publish"
        subprocess.run(
            [
                "dotnet",
                "publish",
                str(PROJECT),
                "-c",
                "Release",
                "-o",
                str(publish_dir),
                "--nologo",
            ],
            cwd=ROOT,
            check=True,
        )
        plugin_dll = publish_dir / f"{plugin_id}.dll"
        if not plugin_dll.is_file():
            raise FileNotFoundError(f"missing published plugin assembly: {plugin_dll}")

        archive = output_dir / f"{plugin_id}_{version}.zip"
        info = zipfile.ZipInfo(plugin_dll.name, date_time=(1980, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = 0o644 << 16
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr(info, plugin_dll.read_bytes())

    write_checksum(archive, "md5", archive.with_suffix(archive.suffix + ".md5"))
    write_checksum(archive, "sha256", archive.with_suffix(archive.suffix + ".sha256"))
    return archive


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", default="dist/jellyfin")
    parser.add_argument("--version", default=VERSION_FILE.read_text(encoding="ascii").strip())
    parser.add_argument("--plugin-id", default=PLUGIN_ID_FILE.read_text(encoding="ascii").strip())
    args = parser.parse_args()

    archive = package(Path(args.output_dir), args.version, args.plugin_id)
    print(archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
