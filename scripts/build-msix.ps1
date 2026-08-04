<#
.SYNOPSIS
  Packages the built desktop application as an MSIX for the Microsoft Store.

.DESCRIPTION
  The Store re-signs an MSIX with a Microsoft certificate after certification, which is
  where both the zero cost and the absence of any SmartScreen warning come from. That
  applies to MSIX only: a Win32 EXE or MSI submitted instead is not re-signed and needs a
  purchased certificate.

  The package is left unsigned: signing it before submission causes validation failures,
  because the publisher in the manifest has to match what Partner Center expects.

.PARAMETER IdentityName
  The package identity Partner Center reserved, for example Publisher.plateforce.

.PARAMETER Publisher
  The publisher subject Partner Center shows, for example CN=<GUID>.

.PARAMETER PublisherDisplayName
  The name a reader sees on the listing.
#>
param(
    [Parameter(Mandatory = $true)][string]$IdentityName,
    [Parameter(Mandatory = $true)][string]$Publisher,
    [Parameter(Mandatory = $true)][string]$PublisherDisplayName,
    [string]$OutputDirectory = "dist"
)

$ErrorActionPreference = "Stop"
$repository = Split-Path -Parent $PSScriptRoot
Set-Location $repository

# An MSIX version is four parts and its revision must be zero for a Store submission. The
# first three come from the manifest Tauri reads, so the package cannot name a release the
# installer does not.
$cargoToml = Get-Content "src-tauri/Cargo.toml" -Raw
if ($cargoToml -notmatch '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"') {
    throw "src-tauri/Cargo.toml declares no version, and the package version is read from it"
}
$version = "$($Matches[1]).0"

foreach ($given in @($IdentityName, $Publisher, $PublisherDisplayName)) {
    if ($given -like "STORE_*") {
        throw "the Store identity is still a placeholder; pass the values Partner Center reserved"
    }
}

$staging = Join-Path ([System.IO.Path]::GetTempPath()) "plateforce-msix"
Remove-Item -Recurse -Force $staging -ErrorAction SilentlyContinue
$assets = New-Item -ItemType Directory -Path (Join-Path $staging "Assets") -Force

$executable = "src-tauri/target/release/plateforce-desktop.exe"
if (-not (Test-Path $executable)) {
    throw "no built application at $executable; run cargo tauri build --no-bundle first"
}
Copy-Item $executable $staging

# The logo set cargo tauri icon writes beside the five bundle icons. The manifest reads
# these by name, so a tidy-up that left only the bundle icons would fail here.
foreach ($logo in @("StoreLogo.png", "Square150x150Logo.png", "Square44x44Logo.png")) {
    $source = Join-Path "src-tauri/icons" $logo
    if (-not (Test-Path $source)) {
        throw "src-tauri/icons/$logo is missing; run scripts/render-icons.py"
    }
    Copy-Item $source $assets
}

$manifest = Get-Content "src-tauri/gen/windows/AppxManifest.xml" -Raw
$manifest = $manifest.Replace("STORE_IDENTITY_NAME", $IdentityName)
$manifest = $manifest.Replace("STORE_PUBLISHER_DISPLAY_NAME", $PublisherDisplayName)
$manifest = $manifest.Replace("STORE_PUBLISHER", $Publisher)
$manifest = $manifest.Replace("STORE_VERSION", $version)
Set-Content -Path (Join-Path $staging "AppxManifest.xml") -Value $manifest -Encoding UTF8

$makeappx = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
    -Filter "makeappx.exe" -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*x64*" } |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) {
    throw "makeappx.exe is not on this machine; it ships with the Windows SDK"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$package = Join-Path $OutputDirectory "plateforce_${version}_x64.msix"
& $makeappx.FullName pack /d $staging /p $package /o
if ($LASTEXITCODE -ne 0) {
    throw "makeappx pack failed with exit code $LASTEXITCODE"
}

Write-Output "msix submission-ready: $package, version $version, unsigned for Store re-signing"
