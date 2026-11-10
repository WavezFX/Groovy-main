# The island sleeps at idle and wakes for the cursor (round 4F item 1).
#
# Launches the CI exe with GROOVY_SLEEP_TEST, keeps the cursor away, and waits
# for groovy.log to say the island is asleep. Then it reads, the way
# docs/GROOVY_SPEC.md section 8 counts memory (private working set of every
# Groovy process, WebView2 included):
#   - the total while asleep (stretch target S1: under 50 MB),
#   - how much the picture standing in for the island differs from the live
#     island (two screenshots of the top of the screen),
#   - how long the island takes to come back when the cursor reaches it,
#   - the total once awake again, and whether he falls asleep a second time.
# It reports; it fails only if Groovy dies or never sleeps.

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$OutDir = "sleep-shots",
  [int]$SleepWithin = 90
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$local = Join-Path $env:LOCALAPPDATA "Groovy"
$log = Join-Path $local "groovy.log"
New-Item -ItemType Directory -Force $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

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
  return @{ Total = [math]::Round($total, 1); Parts = (($kinds.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ", ") }
}

# The top middle of the main screen, where the island sits.
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$shotW = [math]::Min(900, $screen.Width)
$shotX = [int](($screen.Width - $shotW) / 2)
function Get-Shot([string]$name) {
  $bmp = New-Object System.Drawing.Bitmap $shotW, 200
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($shotX, 0, 0, 0, $bmp.Size)
  $g.Dispose()
  $bmp.Save((Join-Path $OutDir $name))
  return $bmp
}

# Mean difference per colour channel (0-255) between two shots, every 2nd pixel.
function Compare-Shots($a, $b) {
  $sum = 0.0; $n = 0
  for ($y = 0; $y -lt $a.Height; $y += 2) {
    for ($x = 0; $x -lt $a.Width; $x += 2) {
      $p = $a.GetPixel($x, $y); $q = $b.GetPixel($x, $y)
      $sum += [math]::Abs($p.R - $q.R) + [math]::Abs($p.G - $q.G) + [math]::Abs($p.B - $q.B)
      $n += 3
    }
  }
  return [math]::Round($sum / $n, 2)
}

function Count-Log([string]$text) {
  if (-not (Test-Path $log)) { return 0 }
  return @(Select-String -Path $log -SimpleMatch $text -ErrorAction SilentlyContinue).Count
}

function Wait-Log([string]$text, [int]$count, [int]$seconds) {
  $deadline = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $deadline) {
    if ((Count-Log $text) -ge $count) { return $true }
    Start-Sleep -Milliseconds 500
  }
  return $false
}

$away = New-Object System.Drawing.Point ($screen.Width - 40), ($screen.Height - 80)
$onIsland = New-Object System.Drawing.Point ([int]($screen.Width / 2)), 6

Stop-Groovy
Remove-Item $log -Force -ErrorAction SilentlyContinue
$metrics = [ordered]@{}
$failed = $null
$env:GROOVY_SLEEP_TEST = "1"
try {
  [System.Windows.Forms.Cursor]::Position = $away
  $proc = Start-Process -FilePath $Exe -PassThru
  Start-Sleep -Seconds 14
  $awake = Get-Total $proc.Id
  $metrics["awake, before sleeping (MB)"] = "$($awake.Total) ($($awake.Parts))"
  $before = Get-Shot "1-awake.png"

  if (-not (Wait-Log "island: asleep" 1 $SleepWithin)) {
    $why = @(Select-String -Path $log -SimpleMatch "island: not sleeping" -ErrorAction SilentlyContinue | ForEach-Object { $_.Line }) | Select-Object -Last 3
    $failed = "the island did not fall asleep within $SleepWithin s. $($why -join ' | ')"
  } else {
    Start-Sleep -Seconds 6
    $proc.Refresh()
    if ($proc.HasExited) { throw "Groovy exited while asleep (code $($proc.ExitCode))" }
    $asleep = Get-Total $proc.Id
    $metrics["ASLEEP total (MB, stretch target S1: under 50)"] = "$($asleep.Total) ($($asleep.Parts))"
    $during = Get-Shot "2-asleep.png"
    $metrics["stand-in against the live island (mean colour difference, 0-255)"] = Compare-Shots $before $during

    # The cursor comes to the island: how long until the page is back.
    [System.Windows.Forms.Cursor]::Position = $onIsland
    if (Wait-Log "island: awake after" 1 10) {
      $line = @(Select-String -Path $log -SimpleMatch "island: awake after" | ForEach-Object { $_.Line }) | Select-Object -Last 1
      if ($line -match 'awake after (\d+) ms') { $metrics["wake for the cursor (ms; target 200)"] = [int]$Matches[1] }
    } else {
      $failed = "the island did not wake for the cursor within 10 s"
    }
    Start-Sleep -Seconds 3
    $null = Get-Shot "3-awake-again.png"
    $again = Get-Total $proc.Id
    $metrics["awake again, 3 s after (MB)"] = "$($again.Total) ($($again.Parts))"

    # Left alone again, he sleeps again.
    [System.Windows.Forms.Cursor]::Position = $away
    $second = Wait-Log "island: asleep" 2 ($SleepWithin + 20)
    $metrics["falls asleep a second time"] = $second
    if ($second) {
      Start-Sleep -Seconds 6
      $asleep2 = Get-Total $proc.Id
      $metrics["asleep the second time (MB)"] = "$($asleep2.Total) ($($asleep2.Parts))"
    }
    $proc.Refresh()
    if ($proc.HasExited) { throw "Groovy exited during the sleep test (code $($proc.ExitCode))" }
  }
} finally {
  Remove-Item Env:GROOVY_SLEEP_TEST -ErrorAction SilentlyContinue
  Stop-Groovy
}

Write-Host "`nSleep test:"
foreach ($k in $metrics.Keys) { Write-Host "  $k = $($metrics[$k])" }
if (Test-Path $log) {
  Write-Host "`ngroovy.log, the island's lines:"
  Select-String -Path $log -SimpleMatch "island:" | ForEach-Object { Write-Host "  $($_.Line)" }
}
if ($env:GITHUB_STEP_SUMMARY) {
  "### The island asleep (round 4F item 1)" | Out-File -Append $env:GITHUB_STEP_SUMMARY
  foreach ($k in $metrics.Keys) { "- ${k}: **$($metrics[$k])**" | Out-File -Append $env:GITHUB_STEP_SUMMARY }
  if ($failed) { "- ❌ $failed" | Out-File -Append $env:GITHUB_STEP_SUMMARY }
}
if ($failed) { Write-Host "::error::$failed"; exit 1 }
