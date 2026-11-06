# Settings opens and closes with an animation (FIXES_ROUND3C item 6). Starts
# Groovy, opens Settings with `groovy --settings general`, and records the
# Settings window's bounds and opacity every few milliseconds, from outside
# the app, while it opens and while it closes. Fails when:
#   - the window is fully opaque in its first 50 ms on screen (no fade, or a
#     window shown before its page painted);
#   - neither its size nor its opacity changes while it opens;
#   - it doesn't end fully opaque at its own size;
#   - it vanishes on close without shrinking or fading first.
# Writes every sample to -Out (uploaded by CI).
#
#   pwsh scripts/settings-anim-test.ps1 -Exe release\groovy.exe

param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$Out = "settings-anim.json"
)

$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

public static class SettingsWatch {
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern int GetWindowLongW(IntPtr h, int index);
  [DllImport("user32.dll")] static extern bool GetLayeredWindowAttributes(IntPtr h, out uint key, out byte alpha, out uint flags);
  [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("winmm.dll")] static extern uint timeBeginPeriod(uint ms);

  public struct RECT { public int Left, Top, Right, Bottom; }
  public class Sample { public double T; public int X, Y, W, H, Alpha; }

  const int GWL_EXSTYLE = -20;
  const int WS_EX_LAYERED = 0x80000;
  const uint LWA_ALPHA = 0x2;

  static Sample Read(IntPtr h, Stopwatch clock) {
    RECT r;
    GetWindowRect(h, out r);
    int alpha = 255;
    if ((GetWindowLongW(h, GWL_EXSTYLE) & WS_EX_LAYERED) != 0) {
      uint key, flags; byte a;
      if (GetLayeredWindowAttributes(h, out key, out a, out flags) && (flags & LWA_ALPHA) != 0) alpha = a;
    }
    return new Sample { T = clock.Elapsed.TotalMilliseconds, X = r.Left, Y = r.Top, W = r.Right - r.Left, H = r.Bottom - r.Top, Alpha = alpha };
  }

  // While it opens: from the first moment it is visible until it has stayed
  // still and opaque for 400 ms (or `limitMs`).
  public static List<Sample> Opening(string title, int limitMs) {
    timeBeginPeriod(1);
    var clock = Stopwatch.StartNew();
    var list = new List<Sample>();
    double stillSince = -1;
    while (clock.ElapsedMilliseconds < limitMs) {
      IntPtr h = FindWindowW(null, title);
      if (h != IntPtr.Zero && IsWindowVisible(h)) {
        var s = Read(h, clock);
        var last = list.Count > 0 ? list[list.Count - 1] : null;
        bool same = last != null && last.X == s.X && last.Y == s.Y && last.W == s.W && last.H == s.H && last.Alpha == s.Alpha;
        list.Add(s);
        if (same && s.Alpha == 255) { if (stillSince < 0) stillSince = s.T; else if (s.T - stillSince > 400) break; }
        else stillSince = -1;
      }
      System.Threading.Thread.Sleep(2);
    }
    return list;
  }

  // Closes it and samples until it is gone.
  public static List<Sample> Closing(string title, int limitMs) {
    var clock = Stopwatch.StartNew();
    var list = new List<Sample>();
    IntPtr h = FindWindowW(null, title);
    if (h == IntPtr.Zero) return list;
    PostMessageW(h, 0x0010, IntPtr.Zero, IntPtr.Zero);
    while (clock.ElapsedMilliseconds < limitMs) {
      h = FindWindowW(null, title);
      if (h == IntPtr.Zero || !IsWindowVisible(h)) break;
      list.Add(Read(h, clock));
      System.Threading.Thread.Sleep(2);
    }
    return list;
  }
}
"@

$title = "Settings " + [char]0x2014 + " Groovy"
$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 10
if ($proc.HasExited) { throw "Groovy exited at start ($($proc.ExitCode))" }

# The running Groovy gets the argument (single instance) and opens Settings.
Start-Process -FilePath $Exe -ArgumentList "--settings", "general" | Out-Null
$open = [SettingsWatch]::Opening($title, 6000)
Start-Sleep -Milliseconds 500
$close = [SettingsWatch]::Closing($title, 3000)
Start-Sleep -Milliseconds 300

@{ opening = $open; closing = $close } | ConvertTo-Json -Depth 4 | Set-Content -Path $Out -Encoding utf8
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Get-Process -Name groovy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

function Line($s) { "{0,7:N1} ms  {1,5},{2,5}  {3,5} x {4,5}  alpha {5,3}" -f $s.T, $s.X, $s.Y, $s.W, $s.H, $s.Alpha }

if ($open.Count -eq 0) { throw "Settings never appeared (no visible window called '$title')" }
"Opening: $($open.Count) samples"
$open | Select-Object -First 12 | ForEach-Object { Line $_ }
"…"
$open | Select-Object -Last 3 | ForEach-Object { Line $_ }

$t0 = $open[0].T
$early = @($open | Where-Object { $_.T - $t0 -lt 50 })
$opaqueEarly = @($early | Where-Object { $_.Alpha -ge 255 })
$sizes = @($open | ForEach-Object { "$($_.W)x$($_.H)" } | Select-Object -Unique)
$alphas = @($open | ForEach-Object { $_.Alpha } | Select-Object -Unique)
$final = $open[-1]

$fail = @()
if ($opaqueEarly.Count -gt 0) { $fail += "fully visible at $([math]::Round($opaqueEarly[0].T - $t0, 1)) ms, within its first 50 ms on screen" }
if ($sizes.Count -lt 3 -and $alphas.Count -lt 3) { $fail += "no change while opening ($($sizes.Count) sizes, $($alphas.Count) opacities)" }
if ($final.Alpha -lt 255) { $fail += "it ended at opacity $($final.Alpha), not fully shown" }
if ($final.W -lt 400 -or $final.H -lt 400) { $fail += "it ended at $($final.W) x $($final.H), not its own size" }

"Closing: $($close.Count) samples"
$close | Select-Object -First 8 | ForEach-Object { Line $_ }
$closeStates = @($close | ForEach-Object { "$($_.W)x$($_.H)@$($_.Alpha)" } | Select-Object -Unique)
if ($closeStates.Count -lt 3) { $fail += "it closed without shrinking or fading ($($closeStates.Count) states)" }

"Opening: $($sizes.Count) sizes, $($alphas.Count) opacities, $([math]::Round($final.T - $t0)) ms to settle; closing: $($closeStates.Count) states"
if ($fail.Count -gt 0) { throw "Settings animation: $($fail -join '; ')" }
"Settings grows and fades in, and folds back out."
