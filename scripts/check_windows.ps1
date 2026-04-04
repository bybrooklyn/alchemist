. (Join-Path $PSScriptRoot "windows_common.ps1")

Require-Tool cargo "Install Rust via rustup or `winget install --id Rustlang.Rustup -e`."
Require-Tool bun "Install Bun via `winget install --id Oven-sh.Bun -e` or from https://bun.sh/docs/installation."

Invoke-Native -Command @("cargo", "fmt", "--all", "--", "--check") -Label "Rust format"
Invoke-Native -Command @("cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings") -Label "Rust clippy"
Invoke-Native -Command @("cargo", "check", "--all-targets") -Label "Rust check"
Invoke-Native -Command @("bun", "install", "--frozen-lockfile") -WorkingDirectory (Join-Path $RepoRoot "web") -Label "Web dependencies"
Invoke-Native -Command @("bun", "run", "verify") -WorkingDirectory (Join-Path $RepoRoot "web") -Label "Frontend verify"

Write-Host "All checks passed ✓"
