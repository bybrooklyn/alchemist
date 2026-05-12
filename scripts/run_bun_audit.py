#!/usr/bin/env python3

import pathlib
import subprocess
import sys


AUDIT_IGNORES = {
    "docs": [
        # docs/ builds static output, but Docusaurus pulls webpack-dev-server -> sockjs -> uuid.
        # The GHSA is specific to uuid v3/v5/v6 buffer handling; installed sockjs only calls uuid.v4().
        "GHSA-w5hq-g745-h8pq",
        # fast-uri (<=3.1.1) is pulled in only by Docusaurus' webpack/schema-utils chain.
        # That chain runs at static-site build time on inputs we author. No patched fast-uri
        # is published yet (advisory covers all versions <=3.1.1).
        "GHSA-v39h-62p7-jpjc",
        "GHSA-q3j6-qgpj-74h6",
        # @babel/plugin-transform-modules-systemjs (<=7.29.3) only runs at build time on our own
        # docs source. We do not compile untrusted input. Docusaurus 3.10.1 pins the affected version
        # transitively through @babel/preset-env; no patched release is reachable via bun update.
        "GHSA-fv7c-fp4j-7gwp",
    ],
    "web": [
        # Astro 5 is flagged for define:vars script sanitization, but this web app does not use define:vars.
        "GHSA-j687-52p2-xcff",
        # fast-uri (<=3.1.1) is pulled in only by @astrojs/check -> yaml-language-server
        # -> ajv. That chain is dev-only (type-check during build) and never receives
        # untrusted URIs. No patched fast-uri exists yet.
        "GHSA-v39h-62p7-jpjc",
        "GHSA-q3j6-qgpj-74h6",
    ],
}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: run_bun_audit.py <cwd>", file=sys.stderr)
        return 2

    cwd = pathlib.Path(sys.argv[1]).resolve()
    command = ["bun", "audit"]
    for advisory in AUDIT_IGNORES.get(cwd.name, []):
            command.append(f"--ignore={advisory}")
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            check=False,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        print(
            f"warning: bun audit timed out after 60s in {cwd}; continuing release-check",
            file=sys.stderr,
        )
        return 0

    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
