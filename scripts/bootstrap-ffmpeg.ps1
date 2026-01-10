# Alchemist FFmpeg Bootstrap Script (Windows)
# This script downloads a stable FFmpeg static build and places it in the 'bin' folder.

$ffmpegVersion = "7.1"
$downloadUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
$binDir = Join-Path $PSScriptRoot "..\bin"
$tempDir = Join-Path $PSScriptRoot "..\temp_ffmpeg"

if (-not (Test-Path $binDir)) {
    New-Item -ItemType Directory -Path $binDir | Out-Null
    Write-Host "Created bin directory at $binDir"
}

if (Test-Path (Join-Path $binDir "ffmpeg.exe")) {
    Write-Host "FFmpeg is already installed in $binDir. Skipping download."
    exit
}

Write-Host "Downloading FFmpeg $ffmpegVersion essentials build..."
try {
    if (-not (Test-Path $tempDir)) {
        New-Item -ItemType Directory -Path $tempDir | Out-Null
    }
    
    $zipFile = Join-Path $tempDir "ffmpeg.zip"
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipFile
    
    Write-Host "Extracting..."
    Expand-Archive -Path $zipFile -DestinationPath $tempDir -Force
    
    $extractedFolder = Get-ChildItem -Path $tempDir -Directory | Where-Object { $_.Name -like "ffmpeg-*" } | Select-Object -First 1
    if ($extractedFolder) {
        $ffmpegPath = Join-Path $extractedFolder.FullName "bin\ffmpeg.exe"
        $ffprobePath = Join-Path $extractedFolder.FullName "bin\ffprobe.exe"
        
        Copy-Item -Path $ffmpegPath -Destination $binDir -Force
        Copy-Item -Path $ffprobePath -Destination $binDir -Force
        
        Write-Host "✅ FFmpeg and FFprobe installed successfully to $binDir"
    } else {
        Write-Error "Could not find extracted FFmpeg folder."
    }
} catch {
    Write-Error "Failed to download or install FFmpeg: $_"
} finally {
    if (Test-Path $tempDir) {
        Remove-Item -Recurit -Force $tempDir
    }
}
