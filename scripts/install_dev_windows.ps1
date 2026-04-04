Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Require-Tool {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$Hint
    )

    if (Get-Command $Name -ErrorAction SilentlyContinue) {
        return
    }

    Write-Error "Required tool '$Name' was not found on PATH. $Hint"
}

function Warn-If-Missing {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$Hint
    )

    if (Get-Command $Name -ErrorAction SilentlyContinue) {
        return
    }

    Write-Warning "Optional tool '$Name' was not found on PATH. $Hint"
}

$RepoRoot = Split-Path -Parent $PSScriptRoot

Require-Tool cargo "Install Rust via rustup or `winget install --id Rustlang.Rustup -e`."
Require-Tool bun "Install Bun via `winget install --id Oven-sh.Bun -e` or from https://bun.sh/docs/installation."

Write-Host "── Rust dependencies ──"
& cargo fetch --locked

Write-Host "── Web dependencies ──"
Push-Location (Join-Path $RepoRoot "web")
& bun install --frozen-lockfile
Pop-Location

Write-Host "── Docs dependencies ──"
Push-Location (Join-Path $RepoRoot "docs")
& bun install --frozen-lockfile
Pop-Location

Write-Host "── E2E dependencies ──"
Push-Location (Join-Path $RepoRoot "web-e2e")
& bun install --frozen-lockfile
& bunx playwright install chromium
Pop-Location

Warn-If-Missing ffmpeg "Install FFmpeg with `winget install Gyan.FFmpeg`."

Write-Host "Repo ready for development."
Write-Host "Next: just dev"
