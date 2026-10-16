# Launches a built Groovy and checks that it really came up, then stops it.
#
#   pwsh scripts/smoke-test.ps1 -Exe target\release\groovy.exe [-Offline] [-Label portable]
#
# Passes only when, within the timeout:
#   - the process is still running (no crash on start),
#   - groovy.log has the "started" line from the backend,
#   - groovy.log has the "boot: interface loaded" line, which the backend only
#     writes when a webview has loaded the interface and called it,
#   - settings were initialised (settings.json in %APPDATA%\Groovy),
#   - the local database exists (groovy.db in %LOCALAPPDATA%\Groovy),
#   - no crash.log was written,
#   - once idle with the island open, Groovy's whole working set — groovy.exe
#     and every process under it (WebView2, llama-server, speech) — is under
#     -BudgetMB (300 MB, FIXES_ROUND3 §9).
# -Offline blocks all outbound traffic for that executable with a Windows
# Firewall rule first (needs an elevated shell, which CI runners have), so the
# run proves Groovy starts with no network at all.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [switch]$Offline,
  [string]$Label = "groovy",
  [int]$TimeoutSeconds = 60,
  [int]$BudgetMB = 300,
  [int]$IdleSeconds = 20
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$local = Join-Path $env:LOCALAPPDATA "Groovy"
$roaming = Join-Path $env:APPDATA "Groovy"
$log = Join-Path $local "groovy.log"
$crash = Join-Path $local "crash.log"
$db = Join-Path $local "groovy.db"
$settings = Join-Path $roaming "settings.json"
$results = [ordered]@{}

function Stop-Groovy {
  Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

Stop-Groovy
Remove-Item $log, $crash -Force -ErrorAction SilentlyContinue

$rule = "Groovy smoke test ($Label) offline"
if ($Offline) {
  New-NetFirewallRule -DisplayName $rule -Direction Outbound -Program $Exe -Action Block | Out-Null
  Write-Host "Outbound network blocked for $Exe"
}

try {
  Write-Host "Launching $Exe"
  $started = Get-Date
  $proc = Start-Process -FilePath $Exe -PassThru
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  $booted = $false
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    if ($proc.HasExited) { break }
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern "boot: interface loaded" -Quiet)) {
      $booted = $true
      $bootSeconds = ((Get-Date) - $started).TotalSeconds
      break
    }
  }
  # Alive after boot, and idle: the intro over, the island open on its Home
  # view and settled. A crash right after the interface loads fails here too.
  if ($booted) { Start-Sleep -Seconds $IdleSeconds }

  # Time to the interface (reported), and memory once idle (judged against
  # -BudgetMB): groovy.exe and every process below it, however deep.
  $metrics = [ordered]@{}
  if ($booted) {
    $metrics["time to interface (s, 2 s polling)"] = [math]::Round($bootSeconds, 1)
    if (-not $proc.HasExited) {
      $proc.Refresh()
      $metrics["groovy.exe working set (MB)"] = [math]::Round($proc.WorkingSet64 / 1MB, 1)
      $metrics["groovy.exe private memory (MB)"] = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
      $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
      $tree = @()
      $frontier = @($proc.Id)
      while ($frontier.Count -gt 0) {
        $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId -and $_.ProcessId -ne $proc.Id })
        $tree += $next
        $frontier = @($next | ForEach-Object { $_.ProcessId })
      }
      $ws = ($tree | Measure-Object -Property WorkingSetSize -Sum).Sum
      $metrics["child processes (WebView2…)"] = $tree.Count
      $metrics["child processes working set (MB)"] = [math]::Round($ws / 1MB, 1)
      foreach ($group in ($tree | Group-Object Name)) {
        $sum = ($group.Group | Measure-Object -Property WorkingSetSize -Sum).Sum
        $metrics["  $($group.Name) x$($group.Count) (MB)"] = [math]::Round($sum / 1MB, 1)
      }
      $total = [math]::Round(($proc.WorkingSet64 + $ws) / 1MB, 1)
      $metrics["TOTAL working set (MB, budget $BudgetMB)"] = $total
      # Task Manager's "Memory" column (private working set), for comparison.
      $ids = @($proc.Id) + @($tree | ForEach-Object { $_.ProcessId })
      $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Where-Object { $ids -contains [int]$_.IDProcess }
      if ($perf) {
        $private = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum
        $metrics["total private working set (MB, Task Manager)"] = [math]::Round($private / 1MB, 1)
      }
      $results["idle total under $BudgetMB MB (was $total)"] = $total -lt $BudgetMB
    }
  }

  $results["process still running"] = -not $proc.HasExited
  $results["backend started (log)"] = (Test-Path $log) -and (Select-String -Path $log -Pattern "--- Groovy .* started ---" -Quiet)
  $results["interface loaded (log)"] = $booted
  $results["settings initialised"] = Test-Path $settings
  $results["local database created"] = Test-Path $db
  $results["no crash report"] = -not (Test-Path $crash)
}
finally {
  Stop-Groovy
  if ($Offline) { Remove-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue }
}

Write-Host ""
Write-Host "Smoke test: $Label$(if ($Offline) { ' (offline)' })"
$failed = 0
foreach ($k in $results.Keys) {
  $ok = [bool]$results[$k]
  if (-not $ok) { $failed++ }
  Write-Host ("  [{0}] {1}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $k)
}
if ($metrics -and $metrics.Count) {
  Write-Host ""
  Write-Host "Measured:"
  foreach ($k in $metrics.Keys) { Write-Host ("  {0}: {1}" -f $k, $metrics[$k]) }
}
if (Test-Path $log) {
  Write-Host ""
  Write-Host "groovy.log:"
  Get-Content $log | Select-Object -Last 40 | ForEach-Object { Write-Host "  $_" }
}
if (Test-Path $crash) { Write-Host "crash.log:"; Get-Content $crash }

if ($env:GITHUB_STEP_SUMMARY) {
  "### Smoke test: $Label$(if ($Offline) { ' (offline)' })" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  foreach ($k in $results.Keys) {
    "- $(if ([bool]$results[$k]) { '✅' } else { '❌' }) $k" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  }
  if ($metrics -and $metrics.Count) {
    "" | Out-File -Append $env:GITHUB_STEP_SUMMARY
    "| Measured | |" | Out-File -Append $env:GITHUB_STEP_SUMMARY
    "|---|---|" | Out-File -Append $env:GITHUB_STEP_SUMMARY
    foreach ($k in $metrics.Keys) { "| $k | $($metrics[$k]) |" | Out-File -Append $env:GITHUB_STEP_SUMMARY }
  }
}
if ($failed -gt 0) { throw "$failed smoke check(s) failed" }
