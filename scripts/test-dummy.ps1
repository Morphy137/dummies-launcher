param(
  [string]$DisplayName = "Dummies Test",
  [switch]$Visible
)

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
$startInfo.Arguments = "`"$DisplayName`""
if ($Visible) {
  $startInfo.Arguments += " --window"
}

$process = [System.Diagnostics.Process]::Start($startInfo)
Start-Sleep -Seconds 1

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
powershell -ExecutionPolicy Bypass -File $measure -RootProcessId $process.Id

Write-Output ""
Write-Output "=== HIDE ==="
$process.StandardInput.WriteLine("HIDE")
$process.StandardInput.Flush()
Start-Sleep -Milliseconds 500
powershell -ExecutionPolicy Bypass -File $measure -RootProcessId $process.Id

Write-Output ""
Write-Output "=== QUIT ==="
$process.StandardInput.WriteLine("QUIT")
$process.StandardInput.Flush()

if (-not $process.WaitForExit(5000)) {
  Write-Warning "Dummy did not exit in time; terminating it."
  $process.Kill()
  $process.WaitForExit()
}

Write-Output ("Exit code: {0}" -f $process.ExitCode)
