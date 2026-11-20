# Lighter WebView2 flags, each measured (FIXES_ROUND4F item 6).
#
# For each candidate flag, launches the CI exe in the frame test
# (GROOVY_SMOKE_VIEW=frames) with WebView2's own environment switch,
# WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, set to Groovy's arguments (read from
# tauri.conf.json, so the same as BROWSER_ARGS) plus the candidate; nothing in
# the shipped exe changes. Prints, per candidate: the idle total 10 s after the
# tour, the split by WebView2 process type, the renderer count, the frame p95
# and longest frame, and whether the browser process really got the flag.
# Reports only: a flag is kept by a person reading this table (>= 3 MB saved,
# glass still blurs, calls still work, P10 holds), in both BROWSER_ARGS and
# tauri.conf.json.
#
#   pwsh scripts/flag-matrix.ps1 -Exe release\test\groovy.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$Runs = 1
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$conf = Get-Content (Join-Path $PSScriptRoot "..\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$base = $conf.app.windows[0].additionalBrowserArgs
$report = Join-Path (Join-Path $env:LOCALAPPDATA "Groovy") "frame-test.json"

# The base's --disable-features list is merged with a candidate's, since a
# second --disable-features would replace the first.
function With([string]$flag) {
  if ($flag -match '^--disable-features=(.+)$') {
    return ($base -replace '--disable-features=([^ ]+)', "--disable-features=`$1,$($Matches[1])")
  }
  if ($flag) { return "$base $flag" }
  return $base
}

$candidates = [ordered]@{
  "baseline"                      = ""
  "renderer-process-limit=1"      = "--renderer-process-limit=1"
  "in-process-gpu"                = "--in-process-gpu"
  "enable-low-end-device-mode"    = "--enable-low-end-device-mode"
  "no background networking etc." = "--disable-background-networking --disable-component-update --disable-sync --no-pings"
  "AudioServiceOutOfProcess off"  = "--disable-features=AudioServiceOutOfProcess"
  "V8 heap cap 96 MB"             = "--js-flags=--max-old-space-size=96"
}

function Measure([int]$root) {
  $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
  $tree = @(); $frontier = @($root)
  while ($frontier.Count -gt 0) {
    $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId -and $_.ProcessId -ne $root })
    $tree += $next; $frontier = @($next | ForEach-Object { $_.ProcessId })
  }
  $ids = @($root) + @($tree | ForEach-Object { [int]$_.ProcessId })
  $kinds = [ordered]@{}; $total = 0.0
  foreach ($p in (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess })) {
    $line = ($tree | Where-Object { $_.ProcessId -eq [int]$p.IDProcess } | Select-Object -First 1).CommandLine
    $kind = if ([int]$p.IDProcess -eq $root) { "groovy" } elseif ($line -match '--type=([a-z-]+)') { $Matches[1] } else { "browser" }
    $mb = $p.WorkingSetPrivate / 1MB
    $kinds[$kind] = [math]::Round(([double]($kinds[$kind] ?? 0)) + $mb, 1); $total += $mb
  }
  $browser = ($tree | Where-Object { $_.Name -like "msedgewebview2*" -and $_.CommandLine -notmatch '--type=' } | Select-Object -First 1).CommandLine
  return @{
    Total = [math]::Round($total, 1)
    Renderers = @($tree | Where-Object { $_.CommandLine -match '--type=renderer' }).Count
    Parts = (($kinds.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ", ")
    Browser = "$browser"
  }
}

$rows = @()
foreach ($name in $candidates.Keys) {
  $flag = $candidates[$name]
  for ($run = 1; $run -le $Runs; $run++) {
    Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item $report -ErrorAction SilentlyContinue
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = With $flag
    $env:GROOVY_SMOKE_VIEW = "frames"
    $proc = Start-Process -FilePath $Exe -PassThru
    Remove-Item Env:GROOVY_SMOKE_VIEW, Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $deadline = (Get-Date).AddSeconds(60)
    while (-not (Test-Path $report) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    $p95 = "-"; $max = "-"
    if (Test-Path $report) {
      $s = (Get-Content $report -Raw | ConvertFrom-Json).stats
      $p95 = $s.p95; $max = $s.max
    }
    Start-Sleep -Seconds 10
    $m = Measure $proc.Id
    $applied = if (-not $flag) { "-" } else { (($flag -split ' ') | ForEach-Object { ($_ -replace '^--disable-features=', '') -replace '^--', '' } | Where-Object { $m.Browser -notlike "*$_*" }).Count -eq 0 }
    $rows += [pscustomobject]@{ Flag = $name; Run = $run; TotalMB = $m.Total; Renderers = $m.Renderers; P95 = $p95; Longest = $max; Applied = $applied; Split = $m.Parts }
    Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  }
}

$baseline = ($rows | Where-Object Flag -eq "baseline" | Measure-Object TotalMB -Average).Average
Write-Host "`nWebView2 flag matrix (idle 10 s after the frame tour; base: $base)"
foreach ($r in $rows) {
  $saved = [math]::Round($baseline - $r.TotalMB, 1)
  Write-Host ("  {0,-30} run {1}: {2} MB (saves {3}), {4} renderers, p95 {5} ms, longest {6} ms, flag applied {7}; {8}" -f $r.Flag, $r.Run, $r.TotalMB, $saved, $r.Renderers, $r.P95, $r.Longest, $r.Applied, $r.Split)
}
if ($env:GITHUB_STEP_SUMMARY) {
  "### WebView2 flags (round 4F item 6), idle after the frame tour" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  "| Flag | MB | Saves | Renderers | p95 ms | Longest ms | Applied | Split |" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  "|---|---|---|---|---|---|---|---|" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  foreach ($r in $rows) {
    "| $($r.Flag) | $($r.TotalMB) | $([math]::Round($baseline - $r.TotalMB, 1)) | $($r.Renderers) | $($r.P95) | $($r.Longest) | $($r.Applied) | $($r.Split) |" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  }
}
