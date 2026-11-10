<#
  Microsoft Store 提出用の MSIX を作る（GitHub Actions の Windows ジョブで tauri build のあとに実行）。
  署名はしない。Store に提出すると Microsoft が署名して配信するので、自前のコード署名証明書は要らない。

  必要な環境変数（GitHub の Variables。Partner Center →「製品 ID」ページの値）:
    MSIX_IDENTITY_NAME           例: 12345ReplicaLLC.ReplicaWorks  （Package/Identity/Name）
    MSIX_PUBLISHER               例: CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX （Package/Identity/Publisher）
    MSIX_PUBLISHER_DISPLAY_NAME  任意。Partner Center の発行者表示名（既定: 合同会社Replica）
  -Tag を渡すと gh で GitHub Release にも添付する
#>
param([string]$Tag = "")
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot/..").Path
Set-Location $root

foreach ($name in "MSIX_IDENTITY_NAME", "MSIX_PUBLISHER") {
  if (-not [Environment]::GetEnvironmentVariable($name)) { throw "$name が設定されていません（Partner Center の製品 ID ページの値を GitHub Variables に入れる）" }
}

$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
$msixVersion = "$version.0"   # Store 提出は 4 桁目が 0 の必要がある

# Tauri の実行ファイル（CLI は productName に合わせて改名することがあるので両方見る）
$exe = @("src-tauri/target/release/Replica Works.exe", "src-tauri/target/release/replica-works-desktop.exe") |
  Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) { throw "ビルド済みの exe が見つかりません。先に tauri build を実行してください" }

$stage = Join-Path $root "dist/msix"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path (Join-Path $stage "Assets") | Out-Null
Copy-Item $exe (Join-Path $stage "ReplicaWorks.exe")
foreach ($a in "StoreLogo", "Square150x150Logo", "Square44x44Logo", "Square71x71Logo", "Square310x310Logo") {
  Copy-Item "src-tauri/icons/$a.png" (Join-Path $stage "Assets")
}

$publisherDisplay = if ($env:MSIX_PUBLISHER_DISPLAY_NAME) { $env:MSIX_PUBLISHER_DISPLAY_NAME } else { "合同会社Replica" }
$manifest = Get-Content -Raw -Encoding UTF8 "src-tauri/msix/AppxManifest.xml"
$manifest = $manifest.Replace("__IDENTITY_NAME__", $env:MSIX_IDENTITY_NAME).
  Replace("__PUBLISHER__", $env:MSIX_PUBLISHER).
  Replace("__VERSION__", $msixVersion).
  Replace("__PUBLISHER_DISPLAY_NAME__", $publisherDisplay)
[IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding $false))

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe（Windows SDK）が見つかりません" }

$out = Join-Path $root "dist/Replica Works_${version}_x64.msix"
& $makeappx.FullName pack /d $stage /p $out /o
if ($LASTEXITCODE -ne 0) { throw "makeappx が失敗しました（$LASTEXITCODE）" }
Write-Host "MSIX: $out"

if ($Tag) {
  gh release upload $Tag $out --clobber
  if ($LASTEXITCODE -ne 0) { throw "gh release upload が失敗しました" }
}
