. (Join-Path $PSScriptRoot "windows_common.ps1")

Require-Tool cargo "Install Rust via rustup or `winget install --id Rustlang.Rustup -e`."
Require-Tool bun "Install Bun via `winget install --id Oven-sh.Bun -e` or from https://bun.sh/docs/installation."
Warn-If-Missing ffmpeg "Install FFmpeg with `winget install Gyan.FFmpeg`."

Invoke-Native -Command @("bun", "install", "--frozen-lockfile") -WorkingDirectory (Join-Path $RepoRoot "web") -Label "Web dependencies"
Invoke-Native -Command @("bun", "run", "build") -WorkingDirectory (Join-Path $RepoRoot "web") -Label "Frontend build"
Invoke-Native -Command @("cargo", "run") -Label "Backend run"
