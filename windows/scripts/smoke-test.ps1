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
#   - no crash.log was written.
# -Offline blocks all outbound traffic for that executable with a Windows
# Firewall rule first (needs an elevated shell, which CI runners have), so the
# run proves Groovy starts with no network at all.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [switch]$Offline,
  [string]$Label = "groovy",
  [int]$TimeoutSeconds = 60
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
  $proc = Start-Process -FilePath $Exe -PassThru
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  $booted = $false
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    if ($proc.HasExited) { break }
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern "boot: interface loaded" -Quiet)) {
      $booted = $true
      break
    }
  }
  # Give it a few more seconds alive after boot: a crash right after the
  # interface loads must fail the test too.
  if ($booted) { Start-Sleep -Seconds 5 }

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
}
if ($failed -gt 0) { throw "$failed smoke check(s) failed" }
