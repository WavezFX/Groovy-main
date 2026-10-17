# CI's realistic memory and CPU test (round 3B item 10).
#
#   pwsh scripts/work-test.ps1 -Exe release\groovy.exe -Audio src-tauri\tests\audio
#
# Starts Groovy with GROOVY_WORK_TEST: it downloads the speech models, the
# wake word spotter, llama.cpp and the smallest local model (cached by the
# workflow), then does one dictation, one « OK Groovy » command and one local
# AI reply, waits for the idle unload and writes work-test.json. Meanwhile
# this script records the peak memory of Groovy's whole process tree (groovy.exe,
# every WebView2 child, llama-server). Then it measures 60 s of idle: fails if
# the private working set (Task Manager's "Memory") is over -BudgetMB or the
# CPU of all those processes together is over -CpuBudget % of the machine.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$Audio,
  [int]$TimeoutSeconds = 1500,
  [int]$BudgetMB = 300,
  [double]$CpuBudget = 2.0,
  [int]$IdleSeconds = 60
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$Audio = (Resolve-Path $Audio).Path
$local = Join-Path $env:LOCALAPPDATA "Groovy"
$log = Join-Path $local "groovy.log"
$report = Join-Path $local "work-test.json"
$cores = [Environment]::ProcessorCount

Get-Process -Name groovy, llama-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item $report, $log -Force -ErrorAction SilentlyContinue

# Groovy and everything under it, however deep.
function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
  $tree = @($all | Where-Object { $_.ProcessId -eq $root })
  $frontier = @($root)
  while ($frontier.Count -gt 0) {
    $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId -and $_.ProcessId -ne $root })
    $tree += $next
    $frontier = @($next | ForEach-Object { $_.ProcessId })
  }
  return $tree
}

function Kind($p, [int]$root) {
  if ($p.ProcessId -eq $root) { return "groovy.exe" }
  if ($p.Name -eq "llama-server.exe") { return "llama-server" }
  if ($p.CommandLine -match '--type=([a-z-]+)') { return "webview2 $($Matches[1])" }
  return "webview2 browser"
}

# Private working set per kind of process, in MB.
function Get-Memory([int]$root) {
  $tree = Get-Tree $root
  $ids = @($tree | ForEach-Object { [int]$_.ProcessId })
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Where-Object { $ids -contains [int]$_.IDProcess }
  $byKind = [ordered]@{}
  $total = 0.0
  foreach ($p in $perf) {
    $proc = $tree | Where-Object { $_.ProcessId -eq [int]$p.IDProcess } | Select-Object -First 1
    $k = Kind $proc $root
    $mb = $p.WorkingSetPrivate / 1MB
    $byKind[$k] = [math]::Round(([double]($byKind[$k] ?? 0)) + $mb, 1)
    $total += $mb
  }
  return @{ total = [math]::Round($total, 1); byKind = $byKind; ids = $ids }
}

function Get-CpuSeconds([int[]]$ids) {
  $sum = 0.0
  foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $sum += $p.TotalProcessorTime.TotalSeconds }
  }
  return $sum
}

$results = [ordered]@{}
$env:GROOVY_WORK_TEST = $Audio
$proc = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:GROOVY_WORK_TEST
$peak = @{ total = 0.0 }
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$done = $null
try {
  while ((Get-Date) -lt $deadline -and -not $proc.HasExited) {
    Start-Sleep -Seconds 3
    $m = Get-Memory $proc.Id
    if ($m.total -gt $peak.total) { $peak = $m }
    if (Test-Path $report) {
      $done = Get-Content $report -Raw | ConvertFrom-Json
      break
    }
  }
  $results["Groovy did the work (dictation, OK Groovy, local AI) and freed the models"] = [bool]($done -and $done.ok)

  if ($done -and -not $proc.HasExited) {
    # Idle: the island folded away, nothing playing, the models freed.
    Start-Sleep -Seconds 10
    $before = Get-Memory $proc.Id
    $cpu0 = Get-CpuSeconds $before.ids
    $t0 = Get-Date
    Start-Sleep -Seconds $IdleSeconds
    $idle = Get-Memory $proc.Id
    $cpu1 = Get-CpuSeconds $idle.ids
    $seconds = ((Get-Date) - $t0).TotalSeconds
    $cpu = [math]::Round(100 * ($cpu1 - $cpu0) / ($seconds * $cores), 2)
    $results["idle memory under $BudgetMB MB (was $($idle.total))"] = $idle.total -lt $BudgetMB
    $results["idle CPU under $CpuBudget% of all $cores cores over $IdleSeconds s (was $cpu%)"] = $cpu -lt $CpuBudget
  }
}
finally {
  Get-Process -Name groovy, llama-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Realistic test (models downloaded, real work, then idle):"
if ($done) {
  foreach ($s in $done.steps) {
    Write-Host ("  [{0}] {1} ({2} ms): {3}" -f $(if ($s.ok) { "PASS" } else { "FAIL" }), $s.name, $s.ms, $s.detail)
  }
}
$failed = 0
foreach ($k in $results.Keys) {
  $ok = [bool]$results[$k]
  if (-not $ok) { $failed++ }
  Write-Host ("  [{0}] {1}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $k)
}
Write-Host ""
Write-Host "Peak while working (private working set, MB): $($peak.total)"
foreach ($k in $peak.byKind.Keys) { Write-Host "    ${k}: $($peak.byKind[$k])" }
if ($idle) {
  Write-Host "Idle after the unload (private working set, MB): $($idle.total)"
  foreach ($k in $idle.byKind.Keys) { Write-Host "    ${k}: $($idle.byKind[$k])" }
}
if (Test-Path $log) {
  Write-Host ""
  Write-Host "groovy.log:"
  Get-Content $log | Select-String "work test|freed|loaded|Parakeet|Whisper|Kokoro|local model|spotter" | Select-Object -Last 40 | ForEach-Object { Write-Host "  $_" }
}
if ($env:GITHUB_STEP_SUMMARY) {
  "### Realistic idle: $(if ($idle) { "$($idle.total) MB, $cpu% CPU" } else { 'not measured' }) (peak while working $($peak.total) MB)" | Out-File -Append $env:GITHUB_STEP_SUMMARY
}
if ($failed -gt 0) { throw "$failed realistic check(s) failed" }
