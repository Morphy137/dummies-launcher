param(
  [string]$DisplayName = "Dummies Test",
  [switch]$Visible
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$dummy = Join-Path $root "crates\dummy-game\target\release\dummy-game.exe"
$measure = Join-Path $PSScriptRoot "measure-memory.ps1"

if (-not (Test-Path $dummy)) {
  Write-Error "dummy-game.exe not found. Build it first: cargo build --release --manifest-path crates/dummy-game/Cargo.toml"
  exit 1
}

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $dummy
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardInput = $true
$startInfo.Arguments = "`"$DisplayName`" --managed"
if ($Visible) {
  $startInfo.Arguments += " --window"
} else {
  $startInfo.Arguments += " --hidden"
}

$process = [System.Diagnostics.Process]::Start($startInfo)
try {
Start-Sleep -Seconds 1
if ($process.HasExited) { throw "Managed dummy exited before receiving a command." }

Write-Output "=== Initial state ==="
powershell -ExecutionPolicy Bypass -File $measure -RootProcessId $process.Id

Write-Output ""
Write-Output "=== SHOW ==="
$process.StandardInput.WriteLine("SHOW")
$process.StandardInput.Flush()
Start-Sleep -Seconds 1
$process.Refresh()
Write-Output ("Title: {0}" -f $process.MainWindowTitle)
Write-Output ("HWND: {0}" -f $process.MainWindowHandle)
if ($process.MainWindowHandle -eq 0) { throw "SHOW did not restore the dummy window." }
powershell -ExecutionPolicy Bypass -File $measure -RootProcessId $process.Id

Write-Output ""
Write-Output "=== HIDE ==="
$process.StandardInput.WriteLine("HIDE")
$process.StandardInput.Flush()
Start-Sleep -Milliseconds 500
$process.Refresh()
if ($process.MainWindowHandle -ne 0) { throw "HIDE did not hide the dummy window." }
powershell -ExecutionPolicy Bypass -File $measure -RootProcessId $process.Id

Write-Output ""
Write-Output "=== QUIT ==="
$process.StandardInput.WriteLine("QUIT")
$process.StandardInput.Flush()

if (-not $process.WaitForExit(5000)) {
  throw "Dummy did not exit after QUIT."
}

Write-Output ("Exit code: {0}" -f $process.ExitCode)
if ($process.ExitCode -ne 0) { throw "Dummy exited with a failure status." }
} finally {
  if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
  $process.Dispose()
}
