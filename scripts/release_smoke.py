#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from urllib.request import Request, urlopen


def run_checked(command: list[str], expected_version: str) -> None:
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    output = f"{result.stdout}\n{result.stderr}"
    if expected_version not in output:
        raise RuntimeError(
            f"{' '.join(command)} did not report expected version {expected_version}: {output}"
        )


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def poll_server(port: int, expected_version: str, timeout: int) -> None:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            with urlopen(f"http://127.0.0.1:{port}/api/ready", timeout=2) as response:
                ready = json.load(response)
            with urlopen(f"http://127.0.0.1:{port}/api/health", timeout=2) as response:
                health = json.load(response)
            if ready.get("ready") is True and health.get("version") == expected_version:
                return
        except Exception as error:
            last_error = error
        time.sleep(1)
    raise TimeoutError(f"server did not become ready: {last_error}")


def complete_setup(port: int, media_dir: Path, timeout: int) -> None:
    payload = json.dumps(
        {
            "username": "release-smoke",
            "password": "release-smoke-password",
            "size_reduction_threshold": 0.3,
            "min_bpp_threshold": 0.1,
            "min_file_size_mb": 50,
            "concurrent_jobs": 1,
            "output_codec": "av1",
            "quality_profile": "balanced",
            "directories": [str(media_dir)],
            "allow_cpu_encoding": True,
            "enable_telemetry": False,
        }
    ).encode("utf-8")
    request = Request(
        f"http://127.0.0.1:{port}/api/setup/complete",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urlopen(request, timeout=timeout) as response:
        if response.status != 200:
            raise RuntimeError(f"setup returned HTTP {response.status}")


def stop(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()

    binary = str(Path(args.binary).resolve())
    run_checked([binary, "--version"], args.expected_version)

    with tempfile.TemporaryDirectory(prefix="alchemist-release-smoke-") as temp:
        root = Path(temp)
        media_dir = root / "media"
        media_dir.mkdir()
        port = free_port()
        env = os.environ.copy()
        env.update(
            {
                "ALCHEMIST_CONFIG_PATH": str(root / "config.toml"),
                "ALCHEMIST_DB_PATH": str(root / "alchemist.db"),
                "ALCHEMIST_CONFIG_MUTABLE": "true",
                "ALCHEMIST_SERVER_PORT": str(port),
                "RUST_LOG": "info",
            }
        )
        log_path = root / "server.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen([binary], env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                poll_server(port, args.expected_version, args.timeout)
                complete_setup(port, media_dir, args.timeout)
            except Exception:
                stop(process)
                print(log_path.read_text(encoding="utf-8", errors="replace"))
                raise
            stop(process)

        subprocess.run([binary, "selftest"], env=env, check=True)

        with log_path.open("ab") as log:
            process = subprocess.Popen([binary], env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                poll_server(port, args.expected_version, args.timeout)
            except Exception:
                stop(process)
                print(log_path.read_text(encoding="utf-8", errors="replace"))
                raise
            stop(process)

    print(f"release smoke passed for {args.expected_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
