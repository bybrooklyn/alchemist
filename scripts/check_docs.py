#!/usr/bin/env python3
"""Validate authoritative Alchemist docs and compare the published mirror."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
CONTENT_ROOT = ROOT / "docs" / "content"
PUBLISHED_INPUTS = (
    CONTENT_ROOT,
    ROOT / "docs" / "assets" / "social-card.png",
    ROOT / "contracts" / "openapi.yaml",
)
DEFAULT_MANIFEST_URL = (
    "https://deadsignal.works/alchemist/docs/source-manifest.json"
)
LINK_PATTERN = re.compile(r"\]\((/[^)\s]+)\)")
DOCUSAURUS_DIRECTIVE_PATTERN = re.compile(r"^:::", re.MULTILINE)


def frontmatter(source: str, path: Path) -> dict[str, str]:
    lines = source.splitlines()
    if len(lines) < 3 or lines[0].strip() != "---":
        raise ValueError(f"{path}: missing YAML frontmatter")
    try:
        end = lines.index("---", 1)
    except ValueError as error:
        raise ValueError(f"{path}: unterminated YAML frontmatter") from error

    fields: dict[str, str] = {}
    for line in lines[1:end]:
        match = re.match(r"^([A-Za-z][A-Za-z0-9_-]*):\s*(.*)$", line)
        if match:
            fields[match.group(1)] = match.group(2).strip().strip("'\"")
    return fields


def route_for(path: Path, fields: dict[str, str]) -> str:
    slug = fields.get("slug")
    if slug:
        return normalize_route(slug)
    relative = path.relative_to(CONTENT_ROOT).with_suffix("")
    if relative.as_posix() == "overview":
        return "/"
    if relative.name == "index":
        relative = relative.parent
    return normalize_route(f"/{relative.as_posix()}")


def normalize_route(route: str) -> str:
    if route == "/":
        return route
    return f"/{route.strip('/')}"


def source_files() -> list[Path]:
    return sorted(CONTENT_ROOT.rglob("*.md"))


def published_files() -> list[Path]:
    files: list[Path] = []
    for item in PUBLISHED_INPUTS:
        if item.is_dir():
            files.extend(path for path in item.rglob("*") if path.is_file())
        elif item.is_file():
            files.append(item)
        else:
            raise ValueError(f"missing published documentation input: {item}")
    return sorted(files, key=lambda path: path.relative_to(ROOT).as_posix())


def content_digest() -> str:
    digest = hashlib.sha256()
    for path in published_files():
        relative = path.relative_to(ROOT).as_posix().encode()
        content = path.read_bytes()
        digest.update(relative)
        digest.update(b"\0")
        digest.update(str(len(content)).encode())
        digest.update(b"\0")
        digest.update(content)
        digest.update(b"\0")
    return digest.hexdigest()


def source_revision() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def validate() -> dict[str, object]:
    failures: list[str] = []
    routes: dict[str, Path] = {}
    files = source_files()
    if not files:
        failures.append("documentation tree contains no Markdown files")

    parsed: list[tuple[Path, str, dict[str, str], str]] = []
    for path in files:
        source = path.read_text(encoding="utf-8")
        try:
            fields = frontmatter(source, path)
            route = route_for(path, fields)
        except ValueError as error:
            failures.append(str(error))
            continue
        for required in ("title", "description"):
            if not fields.get(required):
                failures.append(f"{path}: missing non-empty {required} frontmatter")
        if route in routes:
            failures.append(f"duplicate documentation route {route}: {routes[route]} and {path}")
        routes[route] = path
        parsed.append((path, source, fields, route))

    allowed_assets = {"/openapi.yaml"}
    for path, source, _fields, _route in parsed:
        if DOCUSAURUS_DIRECTIVE_PATTERN.search(source):
            failures.append(f"{path}: contains unsupported Docusaurus directive syntax")
        for match in LINK_PATTERN.finditer(source):
            raw_target = match.group(1)
            path_part = urlsplit(raw_target).path
            target = normalize_route(path_part)
            if target not in routes and target not in allowed_assets:
                failures.append(f"{path}: unresolved documentation link {raw_target}")

    if not (ROOT / "LICENSE-DOCUMENTATION-CC-BY-SA-4.0").is_file():
        failures.append("missing CC BY-SA 4.0 documentation license")

    if failures:
        raise ValueError("\n".join(failures))

    return {
        "schemaVersion": 1,
        "sourceRepository": "https://github.com/bybrooklyn/alchemist",
        "sourceRevision": source_revision(),
        "sourceContentSha256": content_digest(),
        "version": (ROOT / "VERSION").read_text(encoding="utf-8").strip(),
        "fileCount": len(files),
        "routes": sorted(routes),
    }


def load_manifest(location: str) -> dict[str, object]:
    if location.startswith(("https://", "http://")):
        with urllib.request.urlopen(location, timeout=15) as response:
            return json.load(response)
    return json.loads(Path(location).read_text(encoding="utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print the local contract as JSON")
    parser.add_argument(
        "--published-manifest",
        nargs="?",
        const=DEFAULT_MANIFEST_URL,
        help="compare the local source digest with a published or local manifest",
    )
    args = parser.parse_args()

    try:
        contract = validate()
        if args.published_manifest:
            manifest = load_manifest(args.published_manifest)
            if manifest.get("sourceContentSha256") != contract["sourceContentSha256"]:
                raise ValueError(
                    "published documentation digest does not match authoritative source"
                )
        if args.json:
            print(json.dumps(contract, indent=2, sort_keys=True))
        else:
            print(
                "Alchemist docs OK: "
                f"{contract['fileCount']} pages, {contract['sourceContentSha256']}"
            )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"documentation validation failed:\n{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
