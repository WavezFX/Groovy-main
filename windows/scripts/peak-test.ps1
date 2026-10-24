# CI's peak memory test (round 3D item 7): Groovy must never use more than
# 400 MB, even at peak, counting every process it owns.
#
#   pwsh scripts/peak-test.ps1 -Exe release\groovy.exe -Audio src-tauri\tests\audio
#
# Starts Groovy with GROOVY_PEAK_TEST and GROOVY_SMOKE_VIEW=peak: the island is
# held open and turned through the music layout, Reminders and the wardrobe;
# Settings opens on Brains and stays open; a mail refresh, a dictation, a live
# voice call answered out loud (Kokoro) and an « OK Groovy » command run, the
# dictation and the call at the same time, twice. The built-in local model is
# off, as it is by default.
#
# Groovy reads its own total every 250 ms (ceiling.rs, the private working
# set of groovy.exe, its WebView2 processes and its helpers) and writes the
# highest to peak-test.json. This script also reads the same total from
# Windows' performance counters as often as they answer (about once a second)
# and fails if either reading is ever over -CeilingMB.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$Audio,
  [int]$TimeoutSeconds = 1200,
  [int]$CeilingMB = 400
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$Audio = (Resolve-Path $Audio).Path
$local = Join-Path $env:LOCALAPPDATA "Groovy"
$log = Join-Path $local "groovy.log"
$report = Join-Path $local "peak-test.json"

Get-Process -Name groovy, llama-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item $report, $log -Force -ErrorAction SilentlyContinue

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

# Private working set of Groovy's own processes, MB (the user's apps left out).
function Get-Total([int]$root) {
  $tree = @(Get-Tree $root | Where-Object { $_.Name -notmatch '^(ollama|lm studio|lms|spotify)' })
  $ids = @($tree | ForEach-Object { [int]$_.ProcessId })
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Where-Object { $ids -contains [int]$_.IDProcess }
  $total = 0.0
  foreach ($p in $perf) { $total += $p.WorkingSetPrivate / 1MB }
  return [math]::Round($total, 1)
}

$env:GROOVY_PEAK_TEST = $Audio
$env:GROOVY_SMOKE_VIEW = "peak"
$proc = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:GROOVY_PEAK_TEST, Env:GROOVY_SMOKE_VIEW
$seen = 0.0
$readings = 0
$done = $null
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
try {
  while ((Get-Date) -lt $deadline -and -not $proc.HasExited) {
    $m = Get-Total $proc.Id
    $readings++
    if ($m -gt $seen) { $seen = $m }
    if (Test-Path $report) {
      $done = Get-Content $report -Raw | ConvertFrom-Json
      break
    }
    Start-Sleep -Milliseconds 250
  }
}
finally {
  Get-Process -Name groovy, llama-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

$results = [ordered]@{}
$results["Groovy ran the whole scenario"] = [bool]($done -and $done.ok)
if ($done) {
  $results["peak under $CeilingMB MB by Groovy's meter, every $($done.everyMs) ms ($($done.samples) readings; was $($done.peakMb))"] = $done.peakMb -lt $CeilingMB
}
$results["peak under $CeilingMB MB by Windows' counters ($readings readings; was $seen)"] = $seen -lt $CeilingMB

Write-Host ""
Write-Host "Peak test (island on music, Reminders and the wardrobe; Settings on Brains; mail, dictation, a live call with Kokoro, OK Groovy):"
if ($done) {
  foreach ($s in $done.steps) {
    Write-Host ("  [{0}] {1} ({2} ms): {3}" -f $(if ($s.ok) { "PASS" } else { "FAIL" }), $s.name, $s.ms, $s.detail)
  }
  Write-Host ("  at the peak: " + (($done.peakParts | ForEach-Object { "$($_.kind) $([math]::Round($_.mb)) MB" }) -join ", "))
}
$failed = 0
foreach ($k in $results.Keys) {
  $ok = [bool]$results[$k]
  if (-not $ok) { $failed++ }
  Write-Host ("  [{0}] {1}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $k)
}
if (Test-Path $log) { Get-Content $log | Select-String "memory:|peak test|Kokoro|Whisper" | ForEach-Object { Write-Host "  log: $_" } }
if ($env:GITHUB_STEP_SUMMARY) {
  "### Peak: $(if ($done) { "$($done.peakMb) MB (Groovy's meter)" } else { 'not measured' }), $seen MB (Windows' counters); ceiling $CeilingMB MB" | Out-File -Append $env:GITHUB_STEP_SUMMARY
}
if ($failed -gt 0) { throw "$failed peak check(s) failed" }
