#!/usr/bin/env python3
"""Validate that the locked v1 router is represented in OpenAPI."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SERVER = ROOT / "src/server/mod.rs"
OPENAPI = ROOT / "docs/static/openapi.yaml"


def extract_function_body(source: str, function_name: str) -> str:
    match = re.search(rf"\bfn\s+{re.escape(function_name)}\b[^\{{]*\{{", source)
    if match is None:
        raise RuntimeError(f"could not find function {function_name}")

    index = match.end()
    depth = 1
    while index < len(source) and depth:
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        index += 1

    if depth != 0:
        raise RuntimeError(f"could not parse function {function_name}")

    return source[match.end() : index - 1]


def route_literals(body: str) -> set[str]:
    return set(re.findall(r'\.route\(\s*"([^"]+)"', body))


def openapi_paths(source: str) -> set[str]:
    paths: set[str] = set()
    in_paths = False
    for line in source.splitlines():
        if line == "paths:":
            in_paths = True
            continue
        if in_paths and line.startswith("components:"):
            break
        match = re.match(r"^  (/[^\s]+):\s*$", line)
        if match:
            paths.add(match.group(1))
    return paths


def normalize_params(path: str) -> str:
    return re.sub(r":([A-Za-z_][A-Za-z0-9_]*)", r"{\1}", path)


def canonical_v1_from_legacy(path: str) -> str | None:
    if path == "/metrics":
        return "/metrics"
    if not path.startswith("/api/"):
        return None

    aliases = {
        "/api/jobs/table": "/api/v1/jobs",
        "/api/jobs/:id/delete": "/api/v1/jobs/{id}",
    }
    if path in aliases:
        return aliases[path]

    return normalize_params(f"/api/v1{path.removeprefix('/api')}")


def main() -> int:
    server_source = SERVER.read_text()
    openapi_source = OPENAPI.read_text()
    app_body = extract_function_body(server_source, "app_router")
    v1_body = extract_function_body(server_source, "v1_api_router")

    v1_routes = {
        normalize_params(f"/api/v1{route}")
        for route in route_literals(v1_body)
        if route.startswith("/")
    }
    legacy_expected = {
        canonical
        for route in route_literals(app_body)
        if (canonical := canonical_v1_from_legacy(route)) is not None
    }
    documented = openapi_paths(openapi_source)

    missing_v1_aliases = sorted(legacy_expected - v1_routes - {"/metrics"})
    missing_openapi = sorted((v1_routes | {"/metrics"}) - documented)
    stale_openapi = sorted(
        path
        for path in documented
        if path.startswith("/api/v1/") and path not in v1_routes
    )

    failed = False
    if missing_v1_aliases:
        failed = True
        print("Legacy API routes without v1 aliases:")
        for path in missing_v1_aliases:
            print(f"  - {path}")
    if missing_openapi:
        failed = True
        print("v1 API routes missing from docs/static/openapi.yaml:")
        for path in missing_openapi:
            print(f"  - {path}")
    if stale_openapi:
        failed = True
        print("OpenAPI v1 paths not present in the router:")
        for path in stale_openapi:
            print(f"  - {path}")

    if failed:
        return 1

    print(f"API contract OK: {len(v1_routes)} v1 routes documented.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
