. (Join-Path $PSScriptRoot "windows_common.ps1")

Require-Tool cargo "Install Rust via rustup or `winget install --id Rustlang.Rustup -e`."
Require-Tool bun "Install Bun via `winget install --id Oven-sh.Bun -e` or from https://bun.sh/docs/installation."

Invoke-Native -Command @("cargo", "fetch", "--locked") -Label "Rust dependencies"
Invoke-Native -Command @("bun", "install", "--frozen-lockfile") -WorkingDirectory (Join-Path $RepoRoot "web") -Label "Web dependencies"
Invoke-Native -Command @("bun", "install", "--frozen-lockfile") -WorkingDirectory (Join-Path $RepoRoot "docs") -Label "Docs dependencies"
Invoke-Native -Command @("bun", "install", "--frozen-lockfile") -WorkingDirectory (Join-Path $RepoRoot "web-e2e") -Label "E2E dependencies"
Invoke-Native -Command @("bunx", "playwright", "install", "chromium") -WorkingDirectory (Join-Path $RepoRoot "web-e2e")

Warn-If-Missing ffmpeg "Install FFmpeg with `winget install Gyan.FFmpeg`."

Write-Host "Repo ready for development."
Write-Host "Next: just dev"
