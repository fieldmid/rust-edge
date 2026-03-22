# FieldMid CLI Windows Installer
# Usage: Run this script in PowerShell (right-click > Run with PowerShell, or run from terminal)

$repo = "fieldmid/rust-edge-repo"
$binaryName = "fieldmid-windows-x86_64.exe"
$installDir = "$env:USERPROFILE\.local\bin"

function Get-LatestRelease {
    $url = "https://api.github.com/repos/$repo/releases/latest"
    $response = Invoke-RestMethod -Uri $url -UseBasicParsing
    return $response
}

function Download-Binary($release) {
    $asset = $release.assets | Where-Object { $_.name -eq $binaryName }
    if (-not $asset) { Write-Error "No Windows binary found in latest release."; exit 1 }
    $outPath = "$installDir\fieldmid.exe"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $outPath
    return $outPath
}

function Ensure-InstallDir {
    if (-not (Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir | Out-Null
    }
}

function Add-ToPath {
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not $currentPath.Split(';') -contains $installDir) {
        [Environment]::SetEnvironmentVariable("Path", "$installDir;$currentPath", "User")
        Write-Host "Added $installDir to your user PATH. Restart your terminal to use 'fieldmid'."
    }
}

Write-Host "FieldMid CLI Windows Installer"
Ensure-InstallDir
$release = Get-LatestRelease
$outPath = Download-Binary $release
Add-ToPath
Write-Host "Installed fieldmid.exe to $installDir. Run 'fieldmid.exe' to get started."
