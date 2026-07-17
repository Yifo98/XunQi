param()

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ManifestPath = Join-Path $PSScriptRoot "authorized-sniffer-helper-windows.json"
$Manifest = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
$Version = [string]$Manifest.releaseTag
$ZipName = [string]$Manifest.assetName
$DownloadUrl = [string]$Manifest.downloadUrl
$ZipSha256 = ([string]$Manifest.archiveSha256).ToLowerInvariant()
$BinarySha256 = ([string]$Manifest.binarySha256).ToLowerInvariant()
$Target = Join-Path $Root "src-tauri/bin/xunqi-authorized-sniffer-x86_64-pc-windows-msvc.exe"
$LicenseTarget = Join-Path $Root "src-tauri/bin/xunqi-authorized-sniffer-license.txt"

if ((Test-Path -LiteralPath $Target -PathType Leaf) -and
    ((Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash.ToLowerInvariant() -eq $BinarySha256) -and
    (Test-Path -LiteralPath $LicenseTarget -PathType Leaf)) {
  exit 0
}

$Work = Join-Path ([System.IO.Path]::GetTempPath()) ("xunqi-sniffer-helper-" + [Guid]::NewGuid().ToString("N"))
$Zip = Join-Path $Work $ZipName
$Unpacked = Join-Path $Work "unpacked"
New-Item -ItemType Directory -Force -Path $Unpacked | Out-Null
try {
  Invoke-WebRequest -Uri $DownloadUrl -OutFile $Zip -UseBasicParsing
  $ActualZipSha256 = (Get-FileHash -LiteralPath $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($ActualZipSha256 -ne $ZipSha256) {
    throw "Authorized helper ZIP checksum mismatch: $ActualZipSha256"
  }
  Expand-Archive -LiteralPath $Zip -DestinationPath $Unpacked -Force
  $SourceBinary = Join-Path $Unpacked "wx_video_download.exe"
  $SourceLicense = Join-Path $Unpacked "LICENSE"
  $ActualBinarySha256 = (Get-FileHash -LiteralPath $SourceBinary -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($ActualBinarySha256 -ne $BinarySha256) {
    throw "Authorized helper binary checksum mismatch: $ActualBinarySha256"
  }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Target) | Out-Null
  Copy-Item -LiteralPath $SourceBinary -Destination $Target -Force
  $License = @"
XunQi authorized detection helper third-party notice
Source: https://github.com/ltaoo/wx_channels_download
Pinned release: $Version
Release ZIP SHA-256: $ZipSha256
Binary SHA-256: $BinarySha256

"@ + [System.IO.File]::ReadAllText($SourceLicense, [System.Text.UTF8Encoding]::new($false))
  [System.IO.File]::WriteAllText($LicenseTarget, $License, [System.Text.UTF8Encoding]::new($false))
} finally {
  Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction SilentlyContinue
}
