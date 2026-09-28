#!/usr/bin/env python3
"""Enforce the SEC-3 CSP baseline: strict script-src, no DOM-XSS sinks.

The Content-Security-Policy omits 'unsafe-inline' from script-src. That is
only honest if:

1. No page authors an inline <script> in web/src (Astro `is:inline`
   without `src`). Ordinary `<script>` blocks are bundled by Astro into
   external files and `<script src>` tags are external by definition.
2. No DOM-XSS sinks exist in web/src (dangerouslySetInnerHTML, innerHTML
   assignment, eval, `new Function`), so there is no injection path the
   CSP would need to contain.
3. The static middleware CSP value itself has no 'unsafe-inline' in
   script-src.
4. The per-response nonce injector (`inject_csp_nonce` in
   src/server/mod.rs) is still present: Astro injects src-less inline
   scripts (island hydration runtime) into built HTML, and the injector
   is what lets those pages satisfy the strict policy. Removing it while
   keeping script-src strict would blank every island.

Any violation fails loudly so a future change cannot silently weaken
the policy or silently break hydration.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WEB_SRC = ROOT / "web/src"
MIDDLEWARE = ROOT / "src/server/middleware.rs"
SERVER_MOD = ROOT / "src/server/mod.rs"

INLINE_SCRIPT = re.compile(r"<script[^>]*>", re.IGNORECASE)
IS_INLINE_ATTR = re.compile(r"<script[^>]*\bis:inline\b", re.IGNORECASE)

SINKS = [
    re.compile(r"dangerouslySetInnerHTML"),
    re.compile(r"\.innerHTML\s*="),
    re.compile(r"\.outerHTML\s*="),
    re.compile(r"(?<![\w$])eval\s*\("),
    re.compile(r"new\s+Function\s*\("),
    re.compile(r"insertAdjacentHTML\s*\("),
]

SKIP_DIRS = {"node_modules", "dist", ".astro"}
TEXT_SUFFIXES = {
    ".astro",
    ".tsx",
    ".ts",
    ".jsx",
    ".js",
    ".mjs",
    ".cjs",
    ".css",
    ".html",
}


def check_no_inline_scripts() -> list[str]:
    failures: list[str] = []
    for path in sorted(WEB_SRC.rglob("*")):
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        rel = str(path.relative_to(ROOT))
        # Only `is:inline` scripts WITHOUT src are emitted inline. Plain
        # <script> blocks are bundled by Astro into external files served
        # from 'self', and <script src="..."> tags (even with is:inline,
        # which only tells Astro not to bundle them) are external by
        # definition, so neither violates the policy.
        if path.suffix == ".astro":
            for match in INLINE_SCRIPT.finditer(text):
                tag = match.group(0).lower()
                if "is:inline" in tag and "src=" not in tag:
                    failures.append(f"{rel}: Astro `is:inline` script emits inline code")
        else:
            for match in INLINE_SCRIPT.finditer(text):
                tag = match.group(0)
                if "src=" not in tag.lower():
                    failures.append(f"{rel}: inline <script> tag without src: {tag}")
    return failures


def check_no_sinks() -> list[str]:
    failures: list[str] = []
    for path in sorted(WEB_SRC.rglob("*")):
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        rel = str(path.relative_to(ROOT))
        for sink in SINKS:
            for match in sink.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                failures.append(f"{rel}:{line}: DOM-XSS sink `{match.group(0).strip()}`")
    return failures


def check_middleware_csp() -> list[str]:
    try:
        text = MIDDLEWARE.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read {MIDDLEWARE}: {exc}"]
    # Only inspect single-line string literals (the emitted header value),
    # not comments which discuss 'unsafe-inline' by name.
    literals = re.findall(r'"([^"\n]*script-src[^"\n]*)"', text)
    if not literals:
        return ["no script-src directive found in middleware.rs"]
    failures = []
    for literal in literals:
        directive = literal.split(";", 1)[0] if ";" in literal else literal
        # Isolate the script-src directive itself (style-src also carries
        # 'unsafe-inline' by design for React inline styles).
        match = re.search(r"script-src([^;]*)", literal)
        if match is None:
            failures.append("script-src directive missing from CSP value")
        elif "unsafe-inline" in match.group(1):
            failures.append("script-src in middleware.rs still allows 'unsafe-inline'")
    return failures


def check_nonce_injector() -> list[str]:
    try:
        text = SERVER_MOD.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read {SERVER_MOD}: {exc}"]
    failures = []
    if "fn inject_csp_nonce" not in text:
        failures.append("CSP nonce injector missing from src/server/mod.rs")
    if "html_response_with_nonce" not in text:
        failures.append("nonce HTML responder missing from src/server/mod.rs")
    if "'nonce-" not in text and '"nonce-' not in text:
        failures.append("no nonce-bearing script-src construction in src/server/mod.rs")
    return failures


def main() -> int:
    failures = (
        check_no_inline_scripts()
        + check_no_sinks()
        + check_middleware_csp()
        + check_nonce_injector()
    )
    if failures:
        print("CSP baseline violations:")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("CSP baseline ok: strict script-src + nonces, no inline authoring, no DOM-XSS sinks.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
