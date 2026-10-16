# Checks that nothing but the rounded island is drawn (FIXES_ROUND3B item 1:
# the "grey square"). Paints the desktop a bright solid colour, starts Groovy
# held open on a layout (GROOVY_SMOKE_VIEW: "home" is F, "player" is E),
# photographs its window, and fails if a bottom corner — outside the island's
# rounded corners — is anything but the desktop colour, or if the desktop
# doesn't show through the middle of the island (round 3B item 1b).
#
#   pwsh scripts/corner-test.ps1 -Exe release\groovy.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string[]]$Views = @("home", "player"),
  [int]$WaitSeconds = 10,
  [string]$OutDir = "corner-shots"
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class CornerWin {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool SetSysColors(int n, int[] elements, int[] colors);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool SystemParametersInfo(int action, int param, string value, int flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int l, int t, int r, int b);
  [DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr hwnd, IntPtr rgn);
  [DllImport("gdi32.dll")] public static extern int GetRgnBox(IntPtr rgn, out RECT r);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr o);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder s, int n);
  /// Minimises every other visible window (the runner's console sat behind
  /// the island and filled its corners), leaving the desktop and taskbar.
  public static int MinimizeOthers(uint pid) {
    int n = 0;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid || !IsWindowVisible(h)) return true;
      var cls = new StringBuilder(256); GetClassName(h, cls, 256);
      var c = cls.ToString();
      if (c == "Progman" || c == "WorkerW" || c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd") return true;
      if (ShowWindow(h, 6)) n++;
      return true;
    }, IntPtr.Zero);
    return n;
  }
  public static IntPtr Island = IntPtr.Zero;
  /// The island window's region, as "kind l,t,r,b" (kind 0: none, 1: empty, 2: a rectangle, 3: complex).
  public static string RegionOf(IntPtr hwnd) {
    IntPtr rgn = CreateRectRgn(0, 0, 0, 0);
    int kind = GetWindowRgn(hwnd, rgn);
    RECT b; GetRgnBox(rgn, out b);
    DeleteObject(rgn);
    return kind + " " + b.Left + "," + b.Top + "," + b.Right + "," + b.Bottom;
  }
  public static List<RECT> Windows(uint pid) {
    var found = new List<RECT>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      var title = new StringBuilder(256); GetWindowText(h, title, 256);
      RECT r; GetWindowRect(h, out r);
      if (title.ToString() == "Groovy" && r.Right - r.Left > 100 && r.Bottom - r.Top > 40) { found.Add(r); Island = h; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@
[CornerWin]::SetProcessDPIAware() | Out-Null

# A bright magenta desktop, no wallpaper picture.
$wall = [System.Drawing.Color]::FromArgb(255, 0, 255)
Set-ItemProperty -Path "HKCU:\Control Panel\Colors" -Name Background -Value "255 0 255"
Set-ItemProperty -Path "HKCU:\Control Panel\Desktop" -Name Wallpaper -Value ""
[CornerWin]::SystemParametersInfo(20, 0, "", 3) | Out-Null
[CornerWin]::SetSysColors(1, @(1), @(0x00FF00FF)) | Out-Null
Start-Sleep -Seconds 1

function Near($a, $b) {
  return [math]::Abs($a.R - $b.R) -le 12 -and [math]::Abs($a.G - $b.G) -le 12 -and [math]::Abs($a.B - $b.B) -le 12
}

$failed = 0
foreach ($view in $Views) {
  Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
  [CornerWin]::MinimizeOthers(0) | Out-Null
  $env:GROOVY_SMOKE_VIEW = $view
  $proc = Start-Process -FilePath $Exe -PassThru
  Start-Sleep -Seconds $WaitSeconds
  # Only the desktop may be behind the island: whatever else is open (the
  # runner's console) would fill the corners and hide the magenta.
  $hidden = [CornerWin]::MinimizeOthers([uint32]$proc.Id)
  Start-Sleep -Seconds 1
  Write-Host "  ${view}: minimised $hidden other window(s)"
  $rects = [CornerWin]::Windows([uint32]$proc.Id)
  if ($rects.Count -eq 0) {
    Write-Host "  [FAIL] ${view}: no island window on screen"
    $failed++
    continue
  }
  $r = $rects[0]
  Write-Host "  ${view}: window $($r.Left),$($r.Top) to $($r.Right),$($r.Bottom); region (kind box): $([CornerWin]::RegionOf([CornerWin]::Island))"
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $g.Dispose()
  $shot = Join-Path $OutDir "corners-$view.png"
  $bmp.Save($shot, [System.Drawing.Imaging.ImageFormat]::Png)
  # The island's top corners are square (it hangs from the screen's edge);
  # its bottom corners are rounded: just inside them is outside the island.
  $points = @{ "bottom-left" = @(1, ($h - 2)); "bottom-right" = @(($w - 2), ($h - 2)) }
  foreach ($name in $points.Keys) {
    $p = $points[$name]
    $c = $bmp.GetPixel($p[0], $p[1])
    $ok = Near $c $wall
    if (-not $ok) { $failed++ }
    Write-Host ("  [{0}] {1} {2} corner ({3},{4}) = rgb({5},{6},{7}) in a {8}x{9} window" -f $(if ($ok) { "PASS" } else { "FAIL" }), $view, $name, $p[0], $p[1], $c.R, $c.G, $c.B, $w, $h)
  }
  # Round 3B item 1b: the island is see-through. Somewhere across its middle
  # (between the tiles and words) the magenta desktop must show through,
  # tinted darker by the glass: red and blue well above green, but not the
  # untouched desktop colour.
  $seen = $null
  foreach ($y in @([int]($h / 2), ($h - 10))) {
    for ($x = [int]($w * 0.1); $x -lt [int]($w * 0.9) -and -not $seen; $x += 2) {
      $c = $bmp.GetPixel($x, $y)
      if (($c.R - $c.G) -ge 60 -and ($c.B - $c.G) -ge 60 -and -not (Near $c $wall)) { $seen = @($x, $y, $c) }
    }
  }
  if ($seen) {
    Write-Host ("  [PASS] {0} middle ({1},{2}) = rgb({3},{4},{5}): the desktop shows through, tinted" -f $view, $seen[0], $seen[1], $seen[2].R, $seen[2].G, $seen[2].B)
  } else {
    $c = $bmp.GetPixel([int]($w / 2), [int]($h / 2))
    Write-Host ("  [FAIL] {0} middle: no tinted magenta across the island (centre rgb({1},{2},{3})): it isn't see-through" -f $view, $c.R, $c.G, $c.B)
    $failed++
  }
  $bmp.Dispose()
}
Remove-Item Env:GROOVY_SMOKE_VIEW -ErrorAction SilentlyContinue
$log = Join-Path (Join-Path $env:LOCALAPPDATA "Groovy") "groovy.log"
if (Test-Path $log) { Get-Content $log | Select-String "island style|glass|region|tint" | ForEach-Object { Write-Host "  log: $_" } }
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

if ($env:GITHUB_STEP_SUMMARY) {
  "### Island corners: $(if ($failed) { "❌ $failed outside the island" } else { '✅ only the island is drawn' })" | Out-File -Append $env:GITHUB_STEP_SUMMARY
}
if ($failed -gt 0) { throw "$failed corner check(s) failed: something is drawn outside the island" }
