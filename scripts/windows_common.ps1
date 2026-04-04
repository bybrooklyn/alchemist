Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:RepoRoot = Split-Path -Parent $PSScriptRoot

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

    throw "Required tool '$Name' was not found on PATH. $Hint"
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

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Command,
        [string]$WorkingDirectory = $script:RepoRoot,
        [string]$Label = ""
    )

    if ($Label) {
        Write-Host "── $Label ──"
    }

    Push-Location $WorkingDirectory
    try {
        if ($Command.Count -gt 1) {
            & $Command[0] @($Command[1..($Command.Count - 1)])
        } else {
            & $Command[0]
        }

        if ($LASTEXITCODE -ne 0) {
            throw "Command failed ($LASTEXITCODE): $($Command -join ' ')"
        }
    } finally {
        Pop-Location
    }
}
