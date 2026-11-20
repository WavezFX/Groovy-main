# The quiet test (FIXES_ROUND4F item 7). Starts Groovy with
# GROOVY_SMOKE_VIEW=quiet: the island folds, settles 8 s, then the page counts
# every timer and frame callback for 30 s (inside the first minute, so the
# island is awake) and writes %LOCALAPPDATA%\Groovy\quiet-test.json. Also reads
# Groovy's CPU (all its processes) over the same 30 s.
# Done when: 0 frames, 0 timers, CPU at most 0.1 %. Reports only for now.
#
#   pwsh scripts/quiet-test.ps1 -Exe release\test\groovy.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$TimeoutSeconds = 75
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$report = Join-Path (Join-Path $env:LOCALAPPDATA "Groovy") "quiet-test.json"
Remove-Item $report -ErrorAction SilentlyContinue
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

function Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $ids = @($root)
  do {
    $more = @($all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object { [int]$_.ProcessId })
    $ids += $more
  } while ($more.Count -gt 0)
  return $ids
}
function CpuSeconds([int]$root) {
  $total = 0.0
  foreach ($id in (Tree $root)) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $total += $p.TotalProcessorTime.TotalSeconds }
  }
  return $total
}
$cores = [Environment]::ProcessorCount

$env:GROOVY_SMOKE_VIEW = "quiet"
$proc = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:GROOVY_SMOKE_VIEW
# The page loads in about 2 s, folds at 4 s, starts counting 8 s later.
Start-Sleep -Seconds 15
$cpu0 = CpuSeconds $proc.Id
$t0 = Get-Date
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not (Test-Path $report) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
$wall = ((Get-Date) - $t0).TotalSeconds
$cpu = [math]::Round(100.0 * ((CpuSeconds $proc.Id) - $cpu0) / ($wall * $cores), 3)
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if (-not (Test-Path $report)) { throw "No quiet report after $TimeoutSeconds s" }

$q = (Get-Content $report -Raw | ConvertFrom-Json).quiet
$line = "folded and idle, 30 s: frames $($q.frames), timeouts $($q.timeouts), intervals $($q.intervals); CPU $cpu% (island $($q.mode)/$($q.view))"
Write-Host "  $line"
foreach ($s in @($q.sources | Select-Object -First 10)) { Write-Host ("    {0}: {1} calls" -f $s.at, $s.calls) }
$quiet = ($q.frames + $q.timeouts + $q.intervals) -eq 0
if ($env:GITHUB_STEP_SUMMARY) {
  "### Quiet island (4F item 7): $(if ($quiet) { '✅ nothing runs' } else { '⚠️ callbacks ran' })" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  $line | Out-File -Append $env:GITHUB_STEP_SUMMARY
}
if (-not $quiet) { throw "The folded, idle island ran callbacks: $line" }
