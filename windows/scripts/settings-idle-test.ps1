# Idle with Settings open (round 4A item 3; docs/GROOVY_SPEC.md section 8:
# under 150 MB idle, Settings open included).
#
# Launches the CI exe, opens Settings on Home, and reads the private working
# set of every Groovy process (WebView2 included):
#   1. with Settings in front, 20 s after it opened;
#   2. with Settings left open behind another window for 30 s (its webview on
#      the Low memory target, the island asleep, memory handed back);
#   3. after Settings closed itself for sitting unused (the test shortens the
#      ten minutes to 45 s with GROOVY_HUB_IDLE_SECS).
# It reports; it fails only if Groovy dies or Settings never opens.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$BudgetMB = 150
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$log = Join-Path (Join-Path $env:LOCALAPPDATA "Groovy") "groovy.log"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Stop-Groovy {
  Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

function Get-Total([int]$root) {
  $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
  $tree = @()
  $frontier = @($root)
  while ($frontier.Count -gt 0) {
    $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId -and $_.ProcessId -ne $root })
    $tree += $next
    $frontier = @($next | ForEach-Object { $_.ProcessId })
  }
  $ids = @($root) + @($tree | ForEach-Object { [int]$_.ProcessId })
  $kinds = [ordered]@{}
  $total = 0.0
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Where-Object { $ids -contains [int]$_.IDProcess }
  foreach ($p in $perf) {
    $line = ($tree | Where-Object { $_.ProcessId -eq [int]$p.IDProcess } | Select-Object -First 1).CommandLine
    $kind = if ([int]$p.IDProcess -eq $root) { "groovy.exe" } elseif ($line -match '--type=([a-z-]+)') { $Matches[1] } else { "browser" }
    $mb = $p.WorkingSetPrivate / 1MB
    $kinds[$kind] = [math]::Round(([double]($kinds[$kind] ?? 0)) + $mb, 1)
    $total += $mb
  }
  $renderers = @($tree | Where-Object { $_.CommandLine -match '--type=renderer' }).Count
  return @{ Total = [math]::Round($total, 1); Renderers = $renderers; Parts = (($kinds.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ", ") }
}

function Wait-Log([string]$text, [int]$seconds) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $deadline) {
    if ((Test-Path $log) -and (Select-String -Path $log -SimpleMatch $text -Quiet)) { return $true }
    Start-Sleep -Milliseconds 500
  }
  return $false
}

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ($screen.Width - 40), ($screen.Height - 80)

Stop-Groovy
Remove-Item $log -Force -ErrorAction SilentlyContinue
$metrics = [ordered]@{}
$failed = $null
$other = $null
$env:GROOVY_SLEEP_TEST = "1"
$env:GROOVY_HUB_IDLE_SECS = "45"
try {
  $proc = Start-Process -FilePath $Exe -PassThru
  Start-Sleep -Seconds 12
  $alone = Get-Total $proc.Id
  $metrics["island alone (MB)"] = "$($alone.Total) ($($alone.Parts))"

  Start-Process -FilePath $Exe -ArgumentList "--settings", "home" | Out-Null
  if (-not (Wait-Log "control centre opened" 30)) {
    $failed = "Settings did not open"
  } else {
    Start-Sleep -Seconds 20
    $front = Get-Total $proc.Id
    $metrics["Settings open, in front, 20 s (MB, limit $BudgetMB)"] = "$($front.Total), $($front.Renderers) renderers ($($front.Parts))"

    # Another window takes the focus: Settings sits open behind it.
    $other = Start-Process -FilePath "notepad.exe" -PassThru
    Start-Sleep -Seconds 30
    $behind = Get-Total $proc.Id
    $metrics["Settings open, left behind another window, 30 s (MB, limit $BudgetMB)"] = "$($behind.Total), $($behind.Renderers) renderers ($($behind.Parts))"
    $metrics["under $BudgetMB MB with Settings open and unused"] = $behind.Total -lt $BudgetMB

    $closed = Wait-Log "control centre closed after sitting unused" 60
    $metrics["Settings closes itself after sitting unused"] = $closed
    if ($closed) {
      Start-Sleep -Seconds 10
      $after = Get-Total $proc.Id
      $metrics["after it closed (MB)"] = "$($after.Total), $($after.Renderers) renderers ($($after.Parts))"
    }
    $proc.Refresh()
    if ($proc.HasExited) { throw "Groovy exited during the Settings idle test (code $($proc.ExitCode))" }
  }
} finally {
  Remove-Item Env:GROOVY_SLEEP_TEST, Env:GROOVY_HUB_IDLE_SECS -ErrorAction SilentlyContinue
  if ($other) { Stop-Process -Id $other.Id -Force -ErrorAction SilentlyContinue }
  Get-Process -Name notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Stop-Groovy
}

Write-Host "`nSettings idle test:"
foreach ($k in $metrics.Keys) { Write-Host "  $k = $($metrics[$k])" }
if (Test-Path $log) {
  Select-String -Path $log -Pattern "island:|control centre" | ForEach-Object { Write-Host "  $($_.Line)" }
}
if ($env:GITHUB_STEP_SUMMARY) {
  "### Idle with Settings open (round 4A item 3)" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  foreach ($k in $metrics.Keys) { "- ${k}: **$($metrics[$k])**" | Out-File -Append $env:GITHUB_STEP_SUMMARY }
  if ($failed) { "- ❌ $failed" | Out-File -Append $env:GITHUB_STEP_SUMMARY }
}
if ($failed) { Write-Host "::error::$failed"; exit 1 }
