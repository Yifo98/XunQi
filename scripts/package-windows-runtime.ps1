param()

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Package = Get-Content (Join-Path $Root "package.json") -Raw | ConvertFrom-Json
$Tauri = Get-Content (Join-Path $Root "src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json
$CargoVersion = Select-String -Path (Join-Path $Root "src-tauri/Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $CargoVersion) { throw "Cannot read Cargo version" }
$CargoVersion = $CargoVersion.Matches[0].Groups[1].Value
if ($Package.version -ne $Tauri.version -or $Package.version -ne $CargoVersion) {
  throw "Version mismatch: package=$($Package.version) tauri=$($Tauri.version) cargo=$CargoVersion"
}

$Version = $Package.version
$Release = Join-Path $Root "release"
$Name = "XunQi-$Version-Windows-x64-portable-unsigned"
$TempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$Stage = Join-Path $TempRoot $Name
$Runtime = Join-Path $Stage "runtime"
$Binary = Join-Path $Root "src-tauri/target/release/xunqi.exe"
$Zip = Join-Path $Release "$Name.zip"
$Checksum = "$Zip.sha256"

if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
  throw "Native Windows runtime was not found: $Binary"
}
if ((Get-Item -LiteralPath $Binary).Length -le 0) {
  throw "Native Windows runtime is empty: $Binary"
}

Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item $Runtime -ItemType Directory -Force | Out-Null
New-Item $Release -ItemType Directory -Force | Out-Null

function Copy-AsciiBat([string]$Source, [string]$Destination) {
  $Text = [System.IO.File]::ReadAllText($Source, [System.Text.UTF8Encoding]::new($false))
  if ($Text.ToCharArray() | Where-Object { [int]$_ -gt 127 } | Select-Object -First 1) {
    throw "BAT must be ASCII-only: $Source"
  }
  $Text = [System.Text.RegularExpressions.Regex]::Replace($Text, "\r?\n", "`r`n")
  [System.IO.File]::WriteAllText($Destination, $Text, [System.Text.ASCIIEncoding]::new())
}

Copy-AsciiBat (Join-Path $Root "assets/portable/Launch-XunQi-Windows-Runtime.bat") (Join-Path $Stage "Launch-XunQi.bat")
Copy-AsciiBat (Join-Path $Root "assets/portable/Open-XunQi-Logs.bat") (Join-Path $Stage "Open-XunQi-Logs.bat")
Copy-Item (Join-Path $Root "assets/portable/使用说明-Windows便携版.txt") (Join-Path $Stage "README-Windows.txt")
Copy-Item $Binary (Join-Path $Runtime "xunqi.exe")

$PreviousSelfTest = $env:XUNQI_RUNTIME_LAUNCHER_SELF_TEST
try {
  $env:XUNQI_RUNTIME_LAUNCHER_SELF_TEST = "1"
  Push-Location $Stage
  try {
    $SelfTestOutput = @(& $env:ComSpec /d /c "Launch-XunQi.bat" 2>&1)
    $SelfTestExitCode = $LASTEXITCODE
  } finally {
    Pop-Location
  }
} finally {
  $env:XUNQI_RUNTIME_LAUNCHER_SELF_TEST = $PreviousSelfTest
}
if ($SelfTestExitCode -ne 0 -or ($SelfTestOutput -join "`n") -notmatch 'XUNQI_RUNTIME_LAUNCHER_SELF_TEST_OK') {
  throw "Runtime BAT self-test failed: $($SelfTestOutput -join ' | ')"
}

Remove-Item $Zip, $Checksum -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal

$Archive = [System.IO.Compression.ZipFile]::OpenRead($Zip)
try {
  $Expected = @(
    "$Name/Launch-XunQi.bat",
    "$Name/Open-XunQi-Logs.bat",
    "$Name/README-Windows.txt",
    "$Name/runtime/xunqi.exe"
  )
  $Names = @(
    $Archive.Entries |
      Where-Object { -not $_.FullName.EndsWith('/') } |
      ForEach-Object { $_.FullName }
  )
  if ($Names.Count -ne $Expected.Count) {
    throw "Portable ZIP file count mismatch: expected=$($Expected.Count) actual=$($Names.Count)"
  }
  foreach ($Required in $Expected) {
    if ($Names -notcontains $Required) { throw "Portable ZIP is missing: $Required" }
  }
  foreach ($Actual in $Names) {
    if ($Expected -notcontains $Actual) {
      throw "Portable ZIP contains an unexpected file: $Actual"
    }
  }
} finally {
  $Archive.Dispose()
}

$Hash = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
$Line = "$Hash  $([System.IO.Path]::GetFileName($Zip))`n"
[System.IO.File]::WriteAllText($Checksum, $Line, [System.Text.UTF8Encoding]::new($false))
Write-Output "Windows portable runtime package: $Zip"
Write-Output "SHA-256: $Hash"
