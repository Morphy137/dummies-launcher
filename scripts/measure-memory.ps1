param(
  [string]$ProcessName = "dummies",
  [int]$RootProcessId = 0
)

$root = if ($RootProcessId -gt 0) {
  Get-Process -Id $RootProcessId -ErrorAction SilentlyContinue
} else {
  Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
}

if (-not $root) {
  Write-Error "Process '$ProcessName' was not found."
  exit 1
}

$all = Get-CimInstance Win32_Process
$byParent = @{}
foreach ($process in $all) {
  $parentId = [int]$process.ParentProcessId
  if (-not $byParent.ContainsKey($parentId)) {
    $byParent[$parentId] = @()
  }
  $byParent[$parentId] += $process
}

$ids = [System.Collections.Generic.List[int]]::new()
$queue = [System.Collections.Generic.Queue[int]]::new()
$queue.Enqueue($root.Id)

while ($queue.Count -gt 0) {
  $id = $queue.Dequeue()
  if ($ids.Contains($id)) {
    continue
  }
  $ids.Add($id)

  if ($byParent.ContainsKey($id)) {
    foreach ($child in $byParent[$id]) {
      $queue.Enqueue([int]$child.ProcessId)
    }
  }
}

$processes = $ids | ForEach-Object {
  Get-Process -Id $_ -ErrorAction SilentlyContinue
}

# Task Manager's "Memory" column is the private working set, not private commit.
$perfById = @{}
Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | ForEach-Object {
  $perfById[[int]$_.IDProcess] = [double]$_.WorkingSetPrivate
}

$rows = $processes | ForEach-Object {
  [pscustomobject]@{
    Id                  = $_.Id
    ProcessName         = $_.ProcessName
    PrivateWorkingSetMB = [math]::Round(($perfById[$_.Id] / 1MB), 1)
    PrivateCommitMB     = [math]::Round(($_.PrivateMemorySize64 / 1MB), 1)
    WorkingSetMB        = [math]::Round(($_.WorkingSet64 / 1MB), 1)
  }
}

$rows | Sort-Object PrivateWorkingSetMB -Descending | Format-Table -AutoSize

$privateWorkingSet = ($rows | Measure-Object PrivateWorkingSetMB -Sum).Sum
$privateCommit = ($rows | Measure-Object PrivateCommitMB -Sum).Sum
$workingSet = ($rows | Measure-Object WorkingSetMB -Sum).Sum

Write-Output ""
Write-Output ("Processes: {0}" -f @($rows).Count)
Write-Output ("Private working set (Task Manager-style): {0:N1} MB" -f $privateWorkingSet)
Write-Output ("Private commit: {0:N1} MB" -f $privateCommit)
Write-Output ("Total working set (includes shared pages): {0:N1} MB" -f $workingSet)
