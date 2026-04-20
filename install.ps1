# HarnessKit CLI installer for Windows — downloads the latest `hk` binary
# to %LOCALAPPDATA%\Programs\HarnessKit. Re-run to update to the latest version.
#
# Usage:
#   irm https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

# Ensure TLS 1.2 (required by GitHub, not default on PowerShell 5.1)
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = "RealZST/HarnessKit"
$Binary = "hk-windows-x64.exe"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\HarnessKit"

# Get latest release tag
$Headers = @{ "User-Agent" = "HarnessKit-Installer" }
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers $Headers
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
