# Fixes round 4F: stretch RAM goals (10 Nov)

Read `docs/GROOVY_SPEC.md` (section 8, "Stretch targets", and section 10) first. One commit for the round with `[build]`. **Do this round after 4A** (4A gets Groovy under the hard limits; this round goes after the stretch targets).

| # | Stretch target | Today |
|---|---|---|
| S1 | Idle under **50 MB** total | 105–115 MB |
| S2 | Peak under **150 MB** in normal use (island open, panels, chat, Settings) | about 190 MB with Settings open |
| S3 | Peak under **150 MB** in a call with a cloud brain and no local model | about 160 MB |
| S4 | Local-voice call as low as possible; first goal under **200 MB** | 331 MB |

**The reference app, same runner, same counters:** see `STATUS.md` ("Reference measure"). Re-run `.github/workflows/reference-measure.yml` (Actions → Reference measure → Run workflow) after each `[build]` and add the row.

**What the numbers say before any work:** a running WebView2 costs about 100 MB by itself (renderer 52–56, browser 27–30, utility 12, GPU 6), and `groovy.exe` is 5–10 MB. So **S1 cannot be met while a WebView2 stays awake at idle**: trimming the page alone can't take 100 MB to under 45. Item 1 is therefore the only route to S1; the other items serve S2–S4.

Items are ranked by MB saved (estimates until measured). Each new behaviour gets a CI step that prints its number; a stretch number becomes a gate once it is met.

## Item 1: The island sleeps at idle (Opus) (saves about 60–95 MB at idle: S1)
**Goal:** when nothing is happening, no WebView2 work is done and its memory is given back; he still breathes and blinks on screen.
**Files:** `src-tauri/src/island.rs`, `resources/{webview,state,idle}.rs`, `platform/windows.rs`, `glass.rs`, new `src-tauri/src/island_sleep.rs`, `src/island/{island,fsm}.ts`, `src/mochi/engine.ts`, `scripts/smoke-test.ps1`.
**Steps:**
1. **What must stay awake in Rust.** List everything the island page owns that has to work while it sleeps (agent sessions and the pending request, phone link publishing, reminders, music state, notices). Each one either already lives in Rust, or its events queue in Rust and wake the page. Write the list in `STATUS.md` before coding; nothing may be lost while asleep.
2. **Sleep stage A (suspend).** After the island has been folded with no activity for 20 s: the page renders one breathing-and-blink loop of the folded strip to bitmaps (about 24 small frames) and hands them to Rust; a native layered window (no WebView) shows them at about 10 fps in exactly the strip's place, inside the same glass region; the WebView is hidden and suspended (`ICoreWebView2_3::TrySuspend`, which needs it invisible). Windows then takes back its working set.
3. **Waking.** Pointer over the strip, the wake word, a notice, an agent event, a shortcut: resume, show the WebView, hide the native strip in the same frame. No visible jump: the last native frame and the page's first frame match.
4. **Sleep stage B (close), only if stage A leaves idle over 50 MB.** After 2 minutes asleep, destroy the island's WebView so the WebView2 processes exit; recreate it on wake. Pre-create it when the pointer comes within 40 px of the strip. State the page needs comes back from Rust (step 1).
5. Never sleep during a call, while music shows in the strip, while a panel is open, in the first minute after launch, or when "Reduce memory at idle" is turned off in Settings → System (on by default).
6. Linux: not built; the island stays as it is there.
**Test:** smoke-test step "asleep idle" (launch, wait 60 s, measure); a step that hovers the strip and times the open; the corner and frame tests after a wake; a test that an agent permission request arriving while asleep opens the island.
**Done when:** "asleep idle" reads **under 50 MB (S1)** and idle CPU **at most 0.3 % (P1)**; the island opens from sleep **within 200 ms** (stage A) or **within 600 ms** (stage B); the first second after a wake keeps **p95 at most 20 ms (P10)**; and no event is lost in the asleep test.

## Item 2: The lightest local voice and speech-to-text that still sound good (saves about 90–130 MB in a local call: S4)
**Builds on 4A item 1.**
**Files:** `src-tauri/src/speech/{tts,helper,mod}.rs`, `voice.rs`, `resources/ceiling.rs`, `docs/licenses/MODELS.md`, `scripts/peak-test.ps1`.
**Steps:**
1. One ignored benchmark test, run in CI, that for each candidate logs: helper private memory while working, time to first clause (voice) or to transcript (speech-to-text) on the test clips, download size. Candidates, all through sherpa-onnx and all MIT, Apache-2.0 or CC-BY: voices: two Piper/VITS English voices (low, medium), one smaller neural voice if sherpa-onnx ships one; speech-to-text: Whisper tiny and tiny.en, Moonshine tiny and base, against today's Whisper base.
2. The test uploads a WAV of the same three sentences from each voice, and each recogniser's transcript of the test clips with its word error count, as a CI artifact. **The owner listens and picks**; "sounds good" is the owner's call, not the build's.
3. Make the picks the defaults (`offlineVoice`, speech model). Everything heavier stays an optional download.
4. Voice and speech-to-text helpers are never loaded at the same moment unless both fit: `make_room` uses the measured sizes.
**Done when:** the benchmark table is in `STATUS.md`, the owner has picked, and the peak test's local call reads **under 200 MB (S4)**, with **P6 at most 1.0 s** and **P7 at most 2.5 s** still met.

## Item 3: Voice models exist only during a call (saves the helper's whole size outside calls: S2, S4)
**Files:** `src-tauri/src/speech/helper.rs` (`IDLE`, `SHORT_IDLE`), `calls.rs`, `okgroovy/mod.rs`, `resources/idle.rs`.
**Steps:**
1. Helpers start when a call starts or a wake fires, not before. At hang-up both helpers are stopped at once (`helper::stop`), not after 20 s.
2. A single wake command: the speech helper exits when the transcript is returned; the voice helper exits when the reply has finished playing.
3. While listening for the wake word, only the keyword spotter (a few MB) is loaded.
4. No helper is ever started in a call that uses a real-time voice model or cloud speech and voice.
**Test:** a Rust test with a fake helper: started on call start, gone within 2 s of hang-up; the peak test logs process lists at each stage.
**Done when:** 2 s after a call ends, Groovy is back **within 5 MB of its pre-call total**, and a cloud call never shows a helper process in the peak test's log.

## Item 4: One renderer instead of several (saves about 40–75 MB with Settings or the desktop Groovy open: S2)
**Supersedes 4A item 3 step 4.**
**Files:** `src-tauri/src/lib.rs` (`BROWSER_ARGS`, hub window near line 997), `desktop.rs`, `tauri.conf.json`, `src/settings/main.ts`, `src/hub/shell.ts`, `scripts/smoke-test.ps1`.
**Steps:**
1. Measure first: smoke-test steps "Settings open" and "desktop Groovy out", each logging every process.
2. Cheapest first: add `--renderer-process-limit=1` to `BROWSER_ARGS` (every window already shares one WebView2 browser), so the island, Settings and the desktop Groovy share one renderer process. Measure; run the frame test with Settings open, since they now share a main thread.
3. If Settings open is still over 150 MB: Settings becomes a view in the island's own window. The one window grows from the island into the Settings surface (the existing grow-out animation) and folds back; the island strip stays drawn at its top. `settings.html` remains as the Linux path.
4. Don't make one full-monitor transparent window for everything: a 4K transparent surface costs more memory than the windows it replaces.
**Test:** `settings-anim-test.ps1`, Settings screenshots unchanged, frame test with Settings open.
**Done when:** "Settings open" and "desktop Groovy out" each read **under 150 MB (S2)** with **p95 at most 20 ms (P10)**.

## Item 5: A cloud call under 150 MB (saves about 10–45 MB: S3)
**Builds on 4A item 2** (a call adds at most 25 MB to the island).
**Files:** `src/island/voice.ts`, `src/voice/{instant,call,mic}.ts`, `src-tauri/src/calls.rs`, `scripts/peak-test.ps1`.
**Steps:**
1. Add "cloud call" to the peak test: stub cloud brain, stub cloud speech and voice (fixed delays), two turns. Log each process.
2. Find what `groovy.exe` holds in a call (39–47 MB against 5–10 idle): audio buffers, the HTTP client's pools, transcripts. Cap buffers to the clause being played; free at hang-up.
3. During a call the island draws only the call view: panels and tiles are unloaded until hang-up.
**Done when:** "cloud call" peaks **under 150 MB (S3)**.

## Item 6: Lighter WebView2 flags (saves about 5–25 MB; measure each)
**Files:** `src-tauri/src/lib.rs` (`BROWSER_ARGS`), `tauri.conf.json` (`additionalBrowserArgs`; a Rust test keeps the two equal), `/.github/workflows/windows-build.yml`.
**Steps:**
1. A CI matrix step launches the test exe once per candidate (override through a test-only environment switch, compiled out of the release build) and prints idle total, per-process split and the frame test's p95.
2. Candidates: `--renderer-process-limit=1` (item 4); `--in-process-gpu`; `--enable-low-end-device-mode`; `--disable-background-networking --disable-component-update --disable-sync --no-pings`; `--disable-features=AudioServiceOutOfProcess` (only matters once audio is used); a V8 heap cap through `--js-flags=--max-old-space-size=96`.
3. Keep a flag only if it saves **at least 3 MB**, the glass still blurs (corner test), calls still work (peak test) and P10 holds. Record every result, kept or not. Never disable the GPU or web security.
**Done when:** the results table is in `STATUS.md` and the kept flags are in both places.

## Item 7: Nothing runs when nothing is happening (enables item 1; saves CPU and a few MB)
**Files:** `src/island/{island,fsm}.ts`, `src/mochi/engine.ts`, `src/main.ts`, `src/hub/{shell,hero,ui}.ts`, `src/views/ticker.ts`, new `src/core/activity.ts`, `src-tauri/src/scheduler.rs`.
**Steps:**
1. Inventory every `requestAnimationFrame`, `setInterval` and `setTimeout` loop (about 80 call sites; `island.ts` 14, `engine.ts` 13): owner, period, what stops it. Put the table in `STATUS.md`.
2. One gate, `Activity`: loops register with it and run only while their surface is visible **and** something is moving. Folded and idle means zero callbacks. Clocks that must tick (growth, reminders) live in Rust's one scheduler, not in page timers.
3. Test hook (test build only): count timer and frame callbacks over 30 s.
**Done when:** folded and idle, the page runs **0 frame callbacks and 0 timers in 30 s**, and idle CPU is **at most 0.1 %** (inside P1's 0.3 %).

## Item 8: Settings fully unloaded when closed; no hidden page kept alive (saves 0–10 MB; mostly proof)
**Files:** `src-tauri/src/lib.rs`, `desktop.rs`, `studio.rs`, `scripts/smoke-test.ps1`.
**Steps:**
1. Smoke-test sequence: idle → open Settings, visit three pages, close → send the desktop Groovy out and home → idle. Log renderers and total after each step.
2. Fix whatever doesn't return: a window hidden instead of destroyed, a cached page image, a listener that keeps a closed window's objects alive in the island page.
**Done when:** after each close there is **one renderer** (or none, asleep) and the total is **within 5 MB** of the idle before it, 10 s later.

## Item 9: Anything else in the island (saves about 5–15 MB)
**Replaces 4A item 7.**
**Files:** `src/main.ts`, `src/island/island.ts`, `src/style.css`, `vite.config.ts`, `src-tauri/src/tile_icons.rs`.
**Steps:** measure the island's chunks and JS heap after 20 s, then: art for looks not worn and for stage 6 behind `import()`; panel, recap and GitHub views loaded on first open and dropped on close; tile icons as files the WebView loads, not base64 strings in the page's memory; split `style.css` so the folded island loads only its own rules; no cached preview canvases kept after the wardrobe closes.
**Done when:** the measured saving is in `STATUS.md`, or "under 5 MB to gain, not changed".

## Item 10: Checks
- CI reports S1–S4 in the run summary beside the hard limits; each stretch number met becomes a gate at the measured value plus 10 %.
- Re-run the reference measure; `STATUS.md` gets one table: reference app, Groovy before, Groovy after.
- Hard limits still hold: idle under 150 MB, peak under 300 MB. "Ready for the owner": the voice and speech samples to listen to, and how sleep looks.
