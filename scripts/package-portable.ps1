param(
  [string]$OutputDirectory = ""
)

$root = Split-Path -Parent $PSScriptRoot
$releaseDirectory = Join-Path $root "src-tauri\target\release"
$executable = Join-Path $releaseDirectory "dummies.exe"
$dummy = Join-Path $root "src-tauri\resources\dummy-game.exe"
$package = Get-Content (Join-Path $root "package.json") -Raw | ConvertFrom-Json
$version = $package.version

if (-not (Test-Path $executable)) {
  throw "Release executable not found. Run: pnpm tauri build --no-bundle"
}
if (-not (Test-Path $dummy)) {
  throw "dummy-game.exe not found. Run: pnpm build:dummy"
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
  $OutputDirectory = Join-Path $releaseDirectory "portable"
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null

$staging = Join-Path $env:TEMP ("dummies-portable-" + [guid]::NewGuid().ToString("N"))
$resources = Join-Path $staging "resources"
New-Item -ItemType Directory -Force $resources | Out-Null

Copy-Item -Force $executable (Join-Path $staging "Dummies.exe")
Copy-Item -Force $dummy (Join-Path $resources "dummy-game.exe")

$readme = Join-Path $root "README.md"
if (Test-Path $readme) {
  Copy-Item -Force $readme (Join-Path $staging "README.md")
}

$archive = Join-Path $OutputDirectory "Dummies-Portable-$version.zip"
if (Test-Path $archive) {
  Remove-Item -Force $archive
}

Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $archive -CompressionLevel Optimal
Remove-Item -Recurse -Force $staging

Write-Output ("Portable package: {0} ({1:N2} MiB)" -f $archive, ((Get-Item $archive).Length / 1MB))
