$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root "crates\dummy-game\target\release\dummy-game.exe"
$destinationDir = Join-Path $root "src-tauri\resources"
$destination = Join-Path $destinationDir "dummy-game.exe"

if (-not (Test-Path $source)) {
  throw "dummy-game.exe was not found. Build it first: cargo build --release --manifest-path crates/dummy-game/Cargo.toml"
}

New-Item -ItemType Directory -Force $destinationDir | Out-Null
Copy-Item -Force $source $destination
Write-Output "Copied dummy-game.exe to $destination"
