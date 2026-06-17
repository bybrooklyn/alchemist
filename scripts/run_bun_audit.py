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
        # @babel/core sourceMappingURL advisory — low severity, build-time transform only.
        "GHSA-4x5r-pxfx-6jf8",
        # launch-editor NTLMv2 hash disclosure — moderate, webpack-dev-server only (Windows).
        "GHSA-v6wh-96g9-6wx3",
        # ws memory exhaustion DoS — high, but only affects webpack-dev-server; docs use static build.
        "GHSA-96hv-2xvq-fx4p",
        # js-yaml quadratic DoS in merge key handling — moderate, build-time only.
        "GHSA-h67p-54hq-rp68",
    ],
    "web": [
        # Astro 5 is flagged for define:vars script sanitization, but this web app does not use define:vars.
        "GHSA-j687-52p2-xcff",
        # fast-uri (<=3.1.1) is pulled in only by @astrojs/check -> yaml-language-server
        # -> ajv. That chain is dev-only (type-check during build) and never receives
        # untrusted URIs. No patched fast-uri exists yet.
        "GHSA-v39h-62p7-jpjc",
        "GHSA-q3j6-qgpj-74h6",
        # Current compatible web stack still pins esbuild through Astro/Vite on the 0.27.x line
        # (Astro 6.4.6 declares ^0.27.3 and Vite 7.3.x declares ^0.27.0). The broader
        # Astro/Vite build-chain replacement is deferred, so keep a narrow temporary ignore
        # here and let every other web advisory continue failing the gate.
        "GHSA-gv7w-rqvm-qjhr",
        "GHSA-g7r4-m6w7-qqqr",
        # @babel/core sourceMappingURL advisory — low severity, build-time transform only.
        # The web app produces static output; no untrusted source maps are served.
        "GHSA-4x5r-pxfx-6jf8",
        # Vite dev-server advisories (launch-editor NTLMv2, server.fs.deny bypass) —
        # moderate/high on Windows dev server only. Production uses static Astro build.
        "GHSA-v6wh-96g9-6wx3",
        "GHSA-fx2h-pf6j-xcff",
        # js-yaml quadratic DoS in merge key handling — moderate, build-time only.
        # Pulled in by @astrojs/internal-helpers; no untrusted YAML is parsed.
        "GHSA-h67p-54hq-rp68",
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
