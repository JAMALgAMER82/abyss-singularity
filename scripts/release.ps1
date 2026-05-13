# Abyss Singularity — one-command signed release builder.
#
# Reads your private key from %USERPROFILE%\.tauri\abyss-singularity.key,
# builds the app in release mode with updater artifacts + signature, and
# emits a ready-to-upload latest.json manifest into release-output/.
#
# Usage (from repo root):
#   pwsh scripts/release.ps1                 # picks version from package.json
#   pwsh scripts/release.ps1 -Version 0.2.0  # explicit
#   pwsh scripts/release.ps1 -RepoUrl https://your-hosting/path
#
# Then upload the three files listed at the bottom to that host and the
# updater is live for every running Abyss instance.

[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$RepoUrl = "",
    [string]$Notes   = "",
    [string]$KeyPath = "$env:USERPROFILE\.tauri\abyss-singularity.key"
)
$ErrorActionPreference = "Stop"

# ----------------------------------------------------------------------
# 0. Resolve metadata
# ----------------------------------------------------------------------
$repo = Resolve-Path "$PSScriptRoot\.."
Set-Location $repo

if (-not $Version) {
    $pkg = Get-Content "package.json" -Raw | ConvertFrom-Json
    $Version = $pkg.version
}
$tag = "v$Version"
Write-Host "Building Abyss Singularity $tag..." -ForegroundColor Cyan

if (-not (Test-Path $KeyPath)) {
    Write-Host "Private key not found at $KeyPath" -ForegroundColor Red
    Write-Host "Generate one with: npx @tauri-apps/cli signer generate -w `"$KeyPath`" --password `"`" --ci" -ForegroundColor Yellow
    exit 1
}

# ----------------------------------------------------------------------
# 1. Build with the signing key in the environment
# ----------------------------------------------------------------------
$env:TAURI_SIGNING_PRIVATE_KEY          = Get-Content $KeyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
if ($LASTEXITCODE -ne 0) { Write-Host "tauri build failed" -ForegroundColor Red; exit 1 }

# ----------------------------------------------------------------------
# 2. Collect artifacts
# ----------------------------------------------------------------------
$bundle  = "src-tauri\target\release\bundle\nsis"
$setup   = Get-ChildItem "$bundle\*_x64-setup.exe"     | Select-Object -First 1
$nsisZip = Get-ChildItem "$bundle\*_x64-setup.nsis.zip" | Select-Object -First 1
$sigFile = Get-ChildItem "$bundle\*_x64-setup.nsis.zip.sig" | Select-Object -First 1

if (-not $setup -or -not $nsisZip -or -not $sigFile) {
    Write-Host "Expected artifacts missing under $bundle" -ForegroundColor Red
    Write-Host "  setup:   $setup"
    Write-Host "  nsisZip: $nsisZip"
    Write-Host "  sigFile: $sigFile"
    exit 1
}

# ----------------------------------------------------------------------
# 3. Emit latest.json
# ----------------------------------------------------------------------
$outDir = "release-output"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$signature = (Get-Content $sigFile.FullName -Raw).Trim()
$baseUrl   = $RepoUrl.TrimEnd("/")
if (-not $baseUrl) {
    # Fallback: emit relative URLs the user can rewrite by hand if they
    # haven't picked a host yet. The endpoint in tauri.conf.json still
    # needs to be filled in for the running app to find it.
    $baseUrl = "REPLACE_WITH_YOUR_RELEASE_DOWNLOAD_BASE_URL"
}
$manifest = [ordered]@{
    version  = $Version
    notes    = if ($Notes) { $Notes } else { "Release $tag" }
    pub_date = (Get-Date -AsUTC).ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = "$baseUrl/$($nsisZip.Name)"
        }
    }
}
$jsonPath = "$outDir\latest.json"
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding UTF8

Copy-Item $setup.FullName    "$outDir\"
Copy-Item $nsisZip.FullName  "$outDir\"
Copy-Item $sigFile.FullName  "$outDir\"

# ----------------------------------------------------------------------
# 4. Report
# ----------------------------------------------------------------------
Write-Host ""
Write-Host "==================== READY TO PUBLISH ====================" -ForegroundColor Green
Write-Host "Upload these three files to your release host:"
Write-Host "  $outDir\$($setup.Name)              # the user-facing installer"
Write-Host "  $outDir\$($nsisZip.Name)            # what the auto-updater downloads"
Write-Host "  $outDir\latest.json                 # the manifest the endpoint URL serves"
Write-Host ""
Write-Host "If you set -RepoUrl, latest.json already points at the right files."
Write-Host "Otherwise open $outDir\latest.json and replace REPLACE_WITH_YOUR_RELEASE_DOWNLOAD_BASE_URL."
Write-Host ""
Write-Host "Then make sure tauri.conf.json's plugins.updater.endpoints points at"
Write-Host "the URL of the hosted latest.json (e.g. https://your-host/abyss/latest.json)."
Write-Host "==========================================================" -ForegroundColor Green
