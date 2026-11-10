# Measures any desktop app the way docs/GROOVY_SPEC.md section 8 counts Groovy:
# Task Manager's private working set, summed over the app's process and every
# process under it (its WebView2 processes included), after it has sat idle.
# Used to put another app's numbers beside Groovy's, measured on the same runner.
#
#   ./scripts/measure-app.ps1 -Exe C:\path\app.exe -Label "Groovy"

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$Label,
  [int]$IdleSeconds = 30,
  [int]$CpuSeconds = 60,
  [string]$Out = ""
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "no executable at $Exe" }

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
  $tree = @()
  $frontier = @($root)
  while ($frontier.Count -gt 0) {
    $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId -and $_.ProcessId -ne $root })
    $tree += $next
    $frontier = @($next | ForEach-Object { $_.ProcessId })
  }
  return $tree
}

# A copy left running (an installer that starts the app) would be the single instance.
Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds $IdleSeconds
$proc.Refresh()
if ($proc.HasExited) { throw "$Label exited within $IdleSeconds s (code $($proc.ExitCode))" }

$tree = Get-Tree $proc.Id
$ids = @($proc.Id) + @($tree | ForEach-Object { [int]$_.ProcessId })
$main = [IO.Path]::GetFileName($Exe)

$kinds = [ordered]@{}
$total = 0.0
$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Where-Object { $ids -contains [int]$_.IDProcess }
foreach ($p in $perf) {
  $line = ($tree | Where-Object { $_.ProcessId -eq [int]$p.IDProcess } | Select-Object -First 1).CommandLine
  $kind = if ([int]$p.IDProcess -eq $proc.Id) { $main } elseif ($line -match '--type=([a-z-]+)') { "webview " + $Matches[1] } else { "webview browser" }
  $mb = $p.WorkingSetPrivate / 1MB
  $kinds[$kind] = [math]::Round(([double]($kinds[$kind] ?? 0)) + $mb, 1)
  $total += $mb
}
$total = [math]::Round($total, 1)

$cpuOf = { ($ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue } | Measure-Object -Property { $_.TotalProcessorTime.TotalSeconds } -Sum).Sum }
$cpu0 = & $cpuOf
$t0 = Get-Date
Start-Sleep -Seconds $CpuSeconds
$cpu = [math]::Round(100.0 * ((& $cpuOf) - $cpu0) / (((Get-Date) - $t0).TotalSeconds * [Environment]::ProcessorCount), 2)

$renderers = @($tree | Where-Object { $_.CommandLine -match '--type=renderer' }).Count
$parts = ($kinds.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ", "
$row = "| $Label | $total | $($ids.Count) | $renderers | $cpu | $parts |"

Write-Host "$Label idle: $total MB private working set over $($ids.Count) processes ($renderers renderers), CPU $cpu % of the machine over $CpuSeconds s"
Write-Host "  $parts"
if ($Out) { $row | Out-File -Append -Encoding utf8 $Out }

foreach ($id in ($ids | Sort-Object -Descending)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2
