# Round 4C item 5: marks land where they were asked for. Starts Groovy with
# GROOVY_MARKS_TEST: through a picture at half scale (as a 4K look would be
# shrunk) it draws a red arrow from (400, 300) to (600, 500) and a green box at
# (900, 300), 200 x 120, desktop pixels of the primary monitor, and Groovy flies
# over to point at the arrow. A black page sits behind them. Fails unless each
# mark's colour is found within 10 px of where it belongs, and nowhere far away.
#
#   pwsh scripts/marks-test.ps1 -Exe release\groovy.exe -OutDir marks-shots

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$WaitSeconds = 12,
  [string]$OutDir = "marks-shots"
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System.Runtime.InteropServices;
public static class MarksWin { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
"@
[MarksWin]::SetProcessDPIAware() | Out-Null

Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$page = New-Object System.Windows.Forms.Form
$page.FormBorderStyle = "None"
$page.StartPosition = "Manual"
$page.Bounds = $screen
$page.BackColor = [System.Drawing.Color]::Black
$page.ShowInTaskbar = $false
$page.Show()
[System.Windows.Forms.Application]::DoEvents()

$env:GROOVY_MARKS_TEST = "1"
$proc = Start-Process -FilePath $Exe -PassThru
Remove-Item Env:GROOVY_MARKS_TEST
$until = (Get-Date).AddSeconds($WaitSeconds)
while ((Get-Date) -lt $until) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100 }

$bmp = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $bmp.Size)
$g.Dispose()
$bmp.Save((Join-Path $OutDir "marks.png"), [System.Drawing.Imaging.ImageFormat]::Png)

function IsRed($c) { $c.R -gt 170 -and $c.G -lt 130 -and $c.B -lt 130 }
function IsGreen($c) { $c.G -gt 150 -and $c.R -lt 120 -and $c.B -gt 60 -and $c.B -lt 190 }
# Whether a pixel of the colour is within $r px of (x, y).
function Near($x, $y, $test, $r = 10) {
  for ($dy = -$r; $dy -le $r; $dy++) {
    for ($dx = -$r; $dx -le $r; $dx++) {
      $px = $x + $dx; $py = $y + $dy
      if ($px -lt 0 -or $py -lt 0 -or $px -ge $bmp.Width -or $py -ge $bmp.Height) { continue }
      if (& $test $bmp.GetPixel($px, $py)) { return $true }
    }
  }
  return $false
}

$failed = 0
$checks = @(
  @{ name = "arrow start (400, 300)"; x = 400; y = 300; test = ${function:IsRed} },
  @{ name = "arrow middle (500, 400)"; x = 500; y = 400; test = ${function:IsRed} },
  @{ name = "arrow tip (600, 500)"; x = 600; y = 500; test = ${function:IsRed} },
  @{ name = "box top edge (1000, 300)"; x = 1000; y = 300; test = ${function:IsGreen} },
  @{ name = "box left edge (900, 360)"; x = 900; y = 360; test = ${function:IsGreen} },
  @{ name = "box bottom-right (1100, 420)"; x = 1100; y = 420; test = ${function:IsGreen} }
)
foreach ($c in $checks) {
  if (Near $c.x $c.y $c.test) { Write-Host "  [ok] $($c.name)" } else { Write-Host "  [FAIL] $($c.name): not within 10 px"; $failed++ }
}
# Nothing drawn where nothing was asked for.
foreach ($p in @(@(200, 800), @(1000, 360), @(1400, 700))) {
  if ((Near $p[0] $p[1] ${function:IsRed} 4) -or (Near $p[0] $p[1] ${function:IsGreen} 4)) { Write-Host "  [FAIL] a mark at ($($p[0]), $($p[1]))"; $failed++ }
}
$bmp.Dispose()
$log = Join-Path $env:LOCALAPPDATA "Groovy\groovy.log"
if (Test-Path $log) { Get-Content $log | Select-String "marks test|desktop Groovy" | Select-Object -Last 5 | ForEach-Object { Write-Host "  log: $_" } }
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
$page.Close()
if ($failed -gt 0) { throw "$failed mark check(s) failed (see $OutDir/marks.png)" }
Write-Host "marks land within 10 px"
