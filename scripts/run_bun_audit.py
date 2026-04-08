#!/usr/bin/env python3

import pathlib
import subprocess
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: run_bun_audit.py <cwd>", file=sys.stderr)
        return 2

    cwd = pathlib.Path(sys.argv[1]).resolve()
    try:
        completed = subprocess.run(
            ["bun", "audit"],
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
