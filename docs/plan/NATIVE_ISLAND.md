# Native island (prototype, 27 Nov 2026)

Branch `groovy-ai-native-island`. **Off by default:** only `groovy.exe --native-island` uses it. Without the flag the shipped app behaves exactly as before.

## Why
The owner's goal is total RAM under 300 MB at peak, then under 200, then under 100 MB in total (spec §8 S1: under 50 MB idle). Measured today: idle about 100 MB, of which WebView2 is about 100 MB on its own (renderer 52–56, browser 27–30, utility 12, GPU 6), `groovy.exe` 5–10 MB idle, 44 MB at peak. Nothing done inside the page can get an idle Groovy under 100 MB while a WebView2 is alive. The island is the only window that is always there, so it is the one to draw natively; every other window (Settings, chat, panels) can keep using WebView2, loaded when needed and closed after.

## Stack chosen: a raw Win32 layered window plus Groovy's own small rasteriser
| Option | New crates | DLLs it loads | Expected private memory for the pill | Notes |
|---|---|---|---|---|
| **Win32 layered window (`UpdateLayeredWindow`) + own SDF rasteriser + GDI text (chosen)** | **none** (`windows` 0.61 is already a dependency, with `Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse` already on) | none new (gdi32, user32 are in every Windows process) | **under 1 MB**: one 320×48 BGRA DIB (61 KB at 100 %, 245 KB at 200 %), a text DIB of the same order, the drawing code | Anti-aliased shapes from signed distances; grey anti-aliased GDI text used as a coverage mask; per-pixel alpha so the window's empty pixels are click-through. Fully tested on Linux (the drawing has no platform code). |
| Direct2D + DirectWrite through the `windows` crate | none, but 6–8 new `windows` features (more compile time) | d2d1, dwrite, and with a hardware target d3d11, dxgi and the GPU driver | 3–6 MB software target; 10–25 MB with a hardware target (driver heaps) | Best text and paths; worth it later for the open island and rich text. |
| tiny-skia + softbuffer | 2 crates (BSD-3 / MIT-Apache) and their dependencies | none new | about 1–2 MB | Good rasteriser, but softbuffer can't do per-pixel alpha on Windows (needs the layered path anyway), and still no text: a font crate (fontdue/ab_glyph) on top. |
| A second, tiny webview / another GUI toolkit (egui, iced, Slint) | many | wgpu or GL stacks | 20–60 MB | Defeats the purpose. |

All three candidates are MIT/BSD/ISC/Apache-2.0; the chosen one adds no crate at all, so `deny.toml` is untouched.

**Expected totals** (to be confirmed by CI's report step below): the normal idle island is about 100–115 MB; with `--native-island`, idle is `groovy.exe` alone: **about 10–15 MB** (its 5–10 MB today plus the pill's under 1 MB plus some headroom for the GDI font cache). The web island, when opened by a click, costs what it costs today (about +100 MB) and is destroyed again 20 s after it folds and goes quiet.

## What is built
Files: `windows/src-tauri/src/native_island/{mod,draw,win}.rs`.

- **The folded pill** at the top of the screen, where the island's `island_x` setting puts it: frosted-dark glass (86 % opaque, a 1 px light rim), top corners 6 px, bottom corners fully round.
- **Groovy's face:** two eyes in the current look's `eyeInk`, each with a soft glow in its `eyeGlow` (Night green, Ember orange, Frost blue, from `src/mochi/skins.ts`); a blink (160 ms, sine) when the cursor arrives and every 3.4 s while it stays. Changes of look or name are redrawn on `settings-changed`.
- **Text:** the time (`14:05`), and when open the time and his name. No new visible strings (nothing to translate).
- **Hover to expand:** an invisible alpha-1 band 8 px around the folded pill takes the mouse (alpha-0 pixels are click-through on their own); the pill grows from 176×30 to 300×40 with a small overshoot (240 ms), and folds back when the cursor leaves (`TrackMouseEvent`).
- **Click opens the web island; right-click opens Settings.** The web island is built from `tauri.conf.json` on demand (it is now `"create": false` there and built in `setup` in the normal case, which is what Tauri did itself). It is treated as the *awake* state of `island_sleep.rs`: at launch the island counts as asleep; an event that would wake a sleeping island (`Held::Wake`: a hook approval, a reminder, Hey Groovy…) builds the web island too, and the events kept meanwhile are delivered once the new page reports its shape (`set_island_rect`, or after 5 s). When the page asks to sleep (20 s folded and quiet, the same blockers as sleep), the web island window is **destroyed** (not suspended) and the pill comes back, so WebView2's processes exit.
- **Zero frames when folded and idle:** a 16 ms frame timer exists only while the pill opens, folds or blinks; the blink timer only while hovered; otherwise one redraw a minute for the clock. Hidden (web island open), no timer at all. `Anim::moving` is unit-tested.
- Non-activating (`WS_EX_NOACTIVATE`, `MA_NOACTIVATE`), top-most, tool window (no taskbar button).

## Measured
CI step **"Native island prototype: total memory idle at 30 s (report only)"** in `.github/workflows/windows-build.yml` runs `scripts/measure-app.ps1` twice on the shipped exe, without and with `--native-island`, and prints both rows plus the `native island:` log lines (proof it ran native). Never a gate.

## What a full switch still needs
See `STATUS.md` (native island section) for the inventory of island features and how each would be drawn natively or loaded on demand.
