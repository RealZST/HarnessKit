# HarnessKit CLI installer for Windows — downloads the latest `hk` binary
# to ~/.local/bin. Re-run to update to the latest version.
#
# Usage:
#   irm https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "RealZST/HarnessKit"
$Binary = "hk-windows-x64.exe"
$InstallDir = Join-Path $env:USERPROFILE ".local\bin"

# Get latest release tag
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Tag = $Release.tag_name
if (-not $Tag) {
    Write-Error "Failed to fetch latest release"
    exit 1
}

$Url = "https://github.com/$Repo/releases/download/$Tag/$Binary"

Write-Host "Installing HarnessKit CLI $Tag..."

# Download
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$OutPath = Join-Path $InstallDir "hk.exe"
Invoke-WebRequest -Uri $Url -OutFile $OutPath -UseBasicParsing

Write-Host "Installed hk to $OutPath"

# Add to PATH if needed
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$UserPath", "User")
    Write-Host "Added $InstallDir to your PATH."
    Write-Host ""
    Write-Host "Restart your terminal for PATH changes to take effect."
}

Write-Host ""
Write-Host "Verify with: hk status"
