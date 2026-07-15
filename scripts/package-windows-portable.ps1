param()

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Package = Get-Content (Join-Path $Root "package.json") -Raw | ConvertFrom-Json
$Tauri = Get-Content (Join-Path $Root "src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json
$CargoVersion = Select-String -Path (Join-Path $Root "src-tauri/Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $CargoVersion) { throw "无法读取 Cargo 版本" }
$CargoVersion = $CargoVersion.Matches[0].Groups[1].Value
if ($Package.version -ne $Tauri.version -or $Package.version -ne $CargoVersion) {
  throw "版本号不一致：package=$($Package.version) tauri=$($Tauri.version) cargo=$CargoVersion"
}
if ($Package.version -notmatch '-') { throw "公开包只允许预发布版本：$($Package.version)" }

$Version = $Package.version
$Exe = Join-Path $Root "src-tauri/target/release/xunqi.exe"
if (-not (Test-Path $Exe -PathType Leaf)) { throw "没有找到 Windows 程序：$Exe" }

$Release = Join-Path $Root "release"
$Name = "XunQi-$Version-Windows-x64-source-preview"
$TempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$Stage = Join-Path $TempRoot $Name
$Runtime = Join-Path $Stage "runtime"
$Source = Join-Path $Stage "source"
$SourceArchive = Join-Path $TempRoot "xunqi-source-$Version.zip"
$Zip = Join-Path $Release "$Name.zip"
$Checksum = "$Zip.sha256"

Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $SourceArchive -Force -ErrorAction SilentlyContinue
New-Item $Runtime -ItemType Directory -Force | Out-Null
New-Item $Source -ItemType Directory -Force | Out-Null
New-Item $Release -ItemType Directory -Force | Out-Null

Copy-Item $Exe (Join-Path $Runtime "XunQi.exe")
Copy-Item (Join-Path $Root "assets/portable/启动讯栖.cmd") (Join-Path $Stage "Launch-XunQi.cmd")
Copy-Item (Join-Path $Root "assets/portable/使用说明-Windows.txt") (Join-Path $Stage "README-Windows.txt")

git -C $Root archive --format=zip HEAD -o $SourceArchive
if ($LASTEXITCODE -ne 0) { throw "无法生成源码快照" }
Expand-Archive -Path $SourceArchive -DestinationPath $Source -Force
Remove-Item $SourceArchive -Force

Remove-Item $Zip, $Checksum -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal
$Entries = [System.IO.Compression.ZipFile]::OpenRead($Zip)
try {
  $Names = $Entries.Entries.FullName
  foreach ($Required in @(
    "$Name/Launch-XunQi.cmd",
    "$Name/README-Windows.txt",
    "$Name/runtime/XunQi.exe",
    "$Name/source/package.json",
    "$Name/source/src/App.tsx",
    "$Name/source/src-tauri/src/main.rs"
  )) {
    if ($Names -notcontains $Required) { throw "压缩包缺少：$Required" }
  }
} finally {
  $Entries.Dispose()
}

$Hash = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
$Line = "$Hash  $([System.IO.Path]::GetFileName($Zip))`n"
[System.IO.File]::WriteAllText($Checksum, $Line, [System.Text.UTF8Encoding]::new($false))
Write-Output "Windows 源码预发布包：$Zip"
Write-Output "SHA-256：$Hash"
