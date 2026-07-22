#!/usr/bin/env python3
"""Static guardrails for the Docker GPU runtime contract."""

from __future__ import annotations

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(condition: bool, message: str, failures: list[str]) -> None:
    if not condition:
        failures.append(message)


def contains_word(text: str, word: str) -> bool:
    return re.search(rf"(?<![-\w]){re.escape(word)}(?![-\w])", text) is not None


def check_runtime_dockerfiles(failures: list[str]) -> None:
    for path in ("Dockerfile", "Dockerfile.runtime"):
        text = read(path)
        require(
            contains_word(text, "vainfo"),
            f"{path} must install vainfo; docs tell users to run it in the container",
            failures,
        )
        require(
            contains_word(text, "util-linux"),
            f"{path} must install util-linux for setpriv-based privilege dropping",
            failures,
        )
        require(
            not contains_word(text, "gosu"),
            f"{path} must not use gosu; it does not preserve Docker group_add for PUID/PGID mode",
            failures,
        )


def check_entrypoint(failures: list[str]) -> None:
    text = read("entrypoint.sh")
    require("setpriv --reuid" in text, "entrypoint.sh must drop privileges with setpriv", failures)
    require("id -G" in text, "entrypoint.sh must inspect existing supplemental groups", failures)
    require("--groups" in text, "entrypoint.sh must preserve non-root supplemental groups", failures)
    require("--clear-groups" in text, "entrypoint.sh must clear groups when none are provided", failures)
    require("gosu" not in text, "entrypoint.sh must not use gosu for PUID/PGID mode", failures)


def check_workflow_filters(failures: list[str]) -> None:
    text = read(".github/workflows/docker.yml")
    for path in ("Dockerfile", "Dockerfile.runtime", "entrypoint.sh", "scripts/check_docker_runtime_contract.py"):
        quoted = f"- '{path}'"
        require(
            quoted in text,
            f".github/workflows/docker.yml pull_request paths must include {path}",
            failures,
        )
    require(
        "Runtime GPU contract smoke" in text,
        ".github/workflows/docker.yml must smoke-test runtime GPU tools in the built PR image",
        failures,
    )
    for token in ("command -v vainfo", "command -v setpriv", "h264_vaapi|hevc_vaapi", "h264_qsv|hevc_qsv", "h264_nvenc|hevc_nvenc"):
        require(token in text, f"Docker PR smoke must check {token}", failures)


def check_release_smoke(failures: list[str]) -> None:
    text = read(".github/workflows/release-smoke.yml")
    for token in ("command -v vainfo", "command -v setpriv", "h264_vaapi|hevc_vaapi", "h264_qsv|hevc_qsv", "h264_nvenc|hevc_nvenc"):
        require(token in text, f"Release smoke must check {token}", failures)


def check_justfile_workflow(failures: list[str]) -> None:
    text = read("justfile")
    require(
        "docker-runtime-contract:" in text,
        "justfile must expose docker-runtime-contract for local runtime image validation",
        failures,
    )
    for token in (
        "scripts/check_docker_runtime_contract.py",
        "Dockerfile.runtime",
        "ALCHEMIST_DOCKER_TEST_PLATFORM",
        "command -v vainfo",
        "command -v setpriv",
        "h264_vaapi|hevc_vaapi",
        "h264_qsv|hevc_qsv",
        "h264_nvenc|hevc_nvenc",
        "ALCHEMIST_DOCKER_TEST_RENDER_GID",
        "dev-config:/app/config",
        "ALCHEMIST_CONFIG_PATH=/app/config/config.toml",
    ):
        require(token in text, f"justfile must include Docker workflow token {token}", failures)


def check_docker_path_docs(failures: list[str]) -> None:
    docs = "\n".join(
        read(path)
        for path in (
            "README.md",
            "docs/content/docker.md",
            "docs/content/installation.md",
            "docs/content/environment-variables.md",
            "docs/content/configuration-reference.md",
        )
    )
    require(
        "host_path:container_path" in docs,
        "Docker docs must explain host_path:container_path volume syntax",
        failures,
    )
    require(
        "/data/alchemist/config:/app/config" in docs,
        "Docker docs must explain that /data/alchemist/config is a host path",
        failures,
    )
    require(
        "Docker images set `ALCHEMIST_CONFIG_PATH=/app/config/config.toml`" in docs,
        "Docker docs must state the image-level in-container config path",
        failures,
    )
    require(
        "~/.config/alchemist/config.toml" in docs,
        "Docker docs must contrast container config paths with binary defaults",
        failures,
    )


def main() -> int:
    failures: list[str] = []
    check_runtime_dockerfiles(failures)
    check_entrypoint(failures)
    check_workflow_filters(failures)
    check_release_smoke(failures)
    check_justfile_workflow(failures)
    check_docker_path_docs(failures)

    if failures:
        print("Docker runtime contract FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print("Docker runtime contract OK.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
