# The frame-time test (FIXES_ROUND3B item 8b). Starts Groovy with
# GROOVY_SMOKE_VIEW=frames: the island goes F → E → grown tiles → the detail card → quick
# settings → the wardrobe → F, timing every frame, and writes
# %LOCALAPPDATA%\Groovy\frame-test.json. Fails when more than 5% of frames
# take over 20 ms or any frame over 50 ms. Prints Groovy's CPU (all its
# processes, WebView2 included) while animating and while idle afterwards.
#
#   pwsh scripts/frame-test.ps1 -Exe release\groovy.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$TimeoutSeconds = 60,
  [int]$IdleSeconds = 15,
  [string]$Trace = "frame-trace.json"
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$report = Join-Path (Join-Path $env:LOCALAPPDATA "Groovy") "frame-test.json"
Remove-Item $report -ErrorAction SilentlyContinue
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

# Groovy and every process under it (the WebView2 ones), by parent id.
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
function Percent($cpu, $wall) { return [math]::Round(100.0 * $cpu / ($wall * $cores), 2) }

$env:GROOVY_SMOKE_VIEW = "frames"
$proc = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:GROOVY_SMOKE_VIEW
# The tour starts 4 s after the island loads; count from just before it.
Start-Sleep -Seconds 5
$cpu0 = CpuSeconds $proc.Id
$t0 = Get-Date
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not (Test-Path $report) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (-not (Test-Path $report)) {
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  throw "No frame report after $TimeoutSeconds s"
}
$animating = Percent ((CpuSeconds $proc.Id) - $cpu0) ((Get-Date) - $t0).TotalSeconds
Start-Sleep -Seconds 2
$cpu1 = CpuSeconds $proc.Id
$t1 = Get-Date
Start-Sleep -Seconds $IdleSeconds
$idle = Percent ((CpuSeconds $proc.Id) - $cpu1) ((Get-Date) - $t1).TotalSeconds
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

Copy-Item $report $Trace
$r = Get-Content $report -Raw | ConvertFrom-Json
$s = $r.stats
Write-Host ("  frames {0}, mean {1} ms, p95 {2} ms, longest {3} ms, over 20 ms {4} ({5:P1}), over 50 ms {6}" -f $s.frames, $s.mean, $s.p95, $s.max, $s.over20, $s.share20, $s.over50)
foreach ($p in $r.steps.PSObject.Properties) {
  Write-Host ("    {0}: {1} frames, p95 {2} ms, longest {3} ms" -f $p.Name, $p.Value.frames, $p.Value.p95, $p.Value.max)
}
Write-Host "  CPU (all Groovy processes, % of all $cores cores): $animating% animating, $idle% idle"
if ($env:GITHUB_STEP_SUMMARY) {
  "### Frame times: $(if ($r.verdict) { "❌ $($r.verdict)" } else { '✅ smooth' })" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  "p95 $($s.p95) ms, longest $($s.max) ms, $($s.frames) frames; CPU $animating% animating, $idle% idle" | Out-File -Append $env:GITHUB_STEP_SUMMARY
}
if ($r.verdict) { throw "Frame test failed: $($r.verdict)" }
