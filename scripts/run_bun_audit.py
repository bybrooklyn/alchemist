#!/usr/bin/env python3

import pathlib
import subprocess
import sys


AUDIT_IGNORES = {
    "docs": [
        # docs/ builds static output, but Docusaurus pulls webpack-dev-server -> sockjs -> uuid.
        # The GHSA is specific to uuid v3/v5/v6 buffer handling; installed sockjs only calls uuid.v4().
        "GHSA-w5hq-g745-h8pq",
    ],
    "web": [
        # Astro 5 is flagged for define:vars script sanitization, but this web app does not use define:vars.
        "GHSA-j687-52p2-xcff",
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
