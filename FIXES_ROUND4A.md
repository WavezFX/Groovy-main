# Fixes round 4A: RAM and performance (10 Nov, revised to `docs/GROOVY_SPEC.md`)

Read `docs/GROOVY_SPEC.md` (sections 8 and 10) and `docs/plan/ARCHITECTURE_MAP.md` first. The spec's rules apply: one commit for the round with `[build]`, items ticked in it, no feature removed, Linux kept.

**The three gaps to close** (private working set, all Groovy processes):
| Gap | Today | Limit |
|---|---|---|
| Voice call peak | 331 MB (offline voice helper 171, island WebView2 121, `groovy.exe` 39) | under 300 MB |
| What a call adds to the island | about 70 MB | part of the 300 |
| Idle with Settings open | about 190 MB (Settings adds about 78) | under 150 MB |
Idle with the island alone is 105–115 MB, already under 150.

**Measuring:** only Windows CI can measure. Add every measurement step first (items 1.1, 2.1, 3.1, 5.1, 6), so the round's one `[build]` reports before-numbers from the logs of the previous build and after-numbers from its own. If a "done when" is missed, a fix commit with `[build]` follows.

## Item 1: The light offline voice is the default; Kokoro is an optional download (saves about 90 MB at peak)
**Goal:** the offline voice's helper holds **at most 80 MB** while speaking.
**Files:** `src-tauri/src/speech/tts.rs`, `speech/helper.rs`, `speech/kokoro_files.rs` (copy its pattern), `resources/ceiling.rs` (`KOKORO_MB`), `src/hub/pages/voice.ts`, `docs/licenses/MODELS.md`, `scripts/peak-test.ps1`.
**Steps:**
1. An ignored test (like `kokoro_downloads_and_speaks`) loads two small sherpa-onnx VITS/Piper English voices in the `tts` helper and logs the helper's private memory after three 150-character replies and the time to the first clause.
2. Pick the one that is at most 80 MB and at least as fast as Kokoro. Licence must be MIT, BSD, ISC, Apache-2.0, or CC-BY with credit; record it in `MODELS.md`. Pin URL, size and SHA-256.
3. Setting `offlineVoice`: `light` (default for everyone) or `kokoro`. Kokoro moves to "Optional downloads: richer voice, uses about 170 MB while speaking". An install that already has Kokoro's files keeps them and can choose it; nothing is deleted.
4. `ceiling.rs`: a measured `LIGHT_MB`; `make_room` uses the engine in use.
**Test:** the ignored test on Windows CI; the peak test's two calls complete; `tests/settings-keys.test.mjs`.
**Done when:** the peak test (light voice, two calls) reads **under 300 MB** and the voice helper **at most 80 MB**; Kokoro still speaks when chosen.

## Item 2: What a call adds to the island (saves about 45 MB at peak)
**Goal:** a call adds **at most 25 MB** to the island's renderer (now about 70).
**Files:** `src/island/voice.ts`, `src/voice/{speak,sentences,call,mic}.ts`, `src-tauri/src/speech/tts.rs`, `src-tauri/src/music.rs` (rodio is already a dependency), `scripts/peak-test.ps1`.
**Steps:**
1. Log in the peak test: the island renderer's private memory before the call, at each reply and 10 s after; in the test build also the JS heap and the count of live `AudioContext`, blob URL and `<audio>` objects.
2. Play offline-voice replies from Rust through rodio; the page gets only start, level and end events for his mouth. No reply audio crosses into the page.
3. One `AudioContext` per call, closed at hang-up. Revoke blob URLs and drop decoded buffers when a clause ends. Keep the last 20 caption lines in the DOM.
4. Barge-in: Rust playback stops within 150 ms of the user talking over him.
**Test:** voice page tests; peak test calls complete; reply time no worse.
**Done when:** the peak test log shows the island renderer growing **at most 25 MB** across a call and back within 5 MB of its start 10 s after.

## Item 3: Idle under 150 MB with Settings open (Opus) (saves about 40 MB in that state)
**Goal:** island plus Settings, sitting open and idle, **under 150 MB** (now about 190). Settings may add at most about 35 MB.
**Files:** `src-tauri/src/lib.rs` (hub window, near line 997), `resources/{webview,idle,state}.rs`, `src/hub/{shell,hero,ui}.ts`, `src/hub/hub.css`, `src/hub/fonts/`, `src/settings/main.ts`, `scripts/smoke-test.ps1`.
**Steps:**
1. Smoke-test step "Settings open idle": open Settings on Home, wait 20 s, log each process. This gives the split between the Settings renderer and what the island holds meanwhile.
2. Trim the Settings renderer: load only the font subset the language needs (six Manrope subsets ship); the hero's live character draws one still frame unless hovered; no particle loop; pages stay lazy (`settings/main.ts:1453`); drop each page's DOM when leaving it.
3. While Settings has focus the island is folded: set the island WebView to WebView2's Low memory target and stop its animation loop until the pointer returns (`resources/webview.rs` already does this for the folded strip).
4. If step 1's numbers show the fixed cost of a second renderer process alone is over 35 MB, make Settings share the island's renderer process (same-origin window opened from the island page), and re-measure. Choose by measurement and record why.
5. Settings left unfocused for the idle setting (default 10 minutes) is destroyed and reopens on the same page. Never while a download, a sign-in or unsaved input is in progress.
**Test:** the new smoke step; `settings-anim-test.ps1`; Settings screenshots unchanged.
**Done when:** "Settings open idle" reads **under 150 MB**, and a voice call started with Settings open peaks **under 300 MB without the ceiling closing Settings**.

## Item 4: "Listening idle" CI step (after round 4B item 1)
**Steps:** smoke-test step with the wake word on and models present: 60 s quiet, then `tests/audio/not-wake.wav` looped for 60 s. Log memory and CPU.
**Done when:** it reads **under 150 MB**, **at most 1.0 % CPU quiet (P2)** and **at most 3.0 % with speech (P3)**.

## Item 5: The work test gates at 300 MB (446 MB today, not gating)
**Files:** `scripts/work-test.ps1`, `src-tauri/src/resources/ceiling.rs`, `models.rs`, `/.github/workflows/windows-build.yml`.
**Steps:**
1. List each process at the 446 MB moment from the log. The built-in local model server must never be loaded together with a speech helper: add it to `make_room` (speech first).
2. Confirm no local model starts in cloud mode.
3. Turn the gate on.
**Done when:** the work test's peak reads **under 300 MB** and gates.

## Item 6: Start-up and CPU numbers in CI
**Files:** `scripts/smoke-test.ps1`, `src-tauri/src/lib.rs` (log lines "island drawn", "listening").
**Steps:** log launch-to-island-drawn (cold and warm) and launch-to-listening. Keep the idle CPU step.
**Done when:** the run summary shows **P1 at most 0.3 %**, **P8 at most 2.0 s cold and 1.0 s warm**, **P9 at most 4.0 s**; or the measured numbers are written in `STATUS.md` with what holds each one back.

## Item 7: Island idle trim (only if items 1–3 leave under 10 MB of headroom anywhere)
**Steps:** measure the island's chunks and JS heap after 20 s; move art for looks not worn, stage 6 art, panels, recap and GitHub views behind `import()`. No WebView2 flags that disable the GPU or audio.
**Done when:** a measured saving is recorded, or "under 5 MB to gain, not changed".

## Item 8: Checks
- CI gates: idle **150 or lower**, Settings-open idle **150**, peak test **300**, work test **300**. Frame test still p95 at most 20 ms (P10).
- `STATUS.md`: one before and after table for the three gaps. "Ready for the owner".
