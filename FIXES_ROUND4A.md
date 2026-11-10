# Fixes round 4A: RAM and performance (10 Nov)

Read `docs/plan/ARCHITECTURE_MAP.md` first. The rules are the same as every round:
- One item per commit, ticked in `WORK_QUEUE.md` in the same commit. Push after each and keep the build green.
- Don't remove any user-facing feature, and keep all Linux code.
- Budget: under **150 MB idle, never over 300 MB peak**, all Groovy processes, private working set.
- **Measure, then change, then measure.** Memory can only be measured on Windows CI: put `[build]` in the subject of a commit that touches code (markdown-only commits are filtered), read the numbers from the run summary, and write before and after in `STATUS.md`. Never lower a gate blind; never raise one.
- New Tauri commands: `node scripts/command-access.mjs`. New visible text: all ten languages.

**Where the memory is now** (`STATUS.md`, build of bf97a55): idle 105–115 MB (WebView2 about 100, `groovy.exe` 5–10). Peak test 331 MB: Kokoro's helper 171, WebView2 121, `groovy.exe` 39. Work test 446 MB (not gating). Settings open adds about 78 MB to idle.

Items are ranked by MB saved. The MB figures are estimates until measured.

## Item 1: A light offline voice, so a call fits under 300 MB (saves about 80–110 MB at peak)
**Goal:** the offline voice's helper holds under 80 MB while speaking. Kokoro stays as an option.
**Files:** `src-tauri/src/speech/tts.rs`, `speech/helper.rs`, `speech/kokoro_files.rs` (copy its pattern for the new files), `resources/ceiling.rs` (`KOKORO_MB`), `src/hub/pages/voice.ts`, `docs/licenses/MODELS.md`, `scripts/peak-test.ps1`.
**Steps:**
1. In a test (ignored, like `kokoro_downloads_and_speaks`), load two sherpa-onnx VITS/Piper English voices (a "low" and a "medium" one) in the `tts` helper and log the helper's private memory after three 150-character replies, and the time per reply. Run it in one `[build]`.
2. If one is under 80 MB and no slower than Kokoro, add it as the offline engine `light` with pinned URL, size and SHA-256. Only models with a licence that allows shipping (MIT, Apache-2.0, CC-BY with credit); record it in `MODELS.md`.
3. Setting `offlineVoice`: `light` (default for new installs) or `kokoro` ("Richer voice, uses more memory", same wording pattern as Parakeet). Existing installs keep Kokoro until the user changes it.
4. Add a `LIGHT_MB` estimate from the measured number; `make_room` uses the engine in use.
5. Run the peak test with `light`. Record the result.
**Test:** the ignored test passes on Windows CI; the peak test's two calls still complete; `tests/settings-keys.test.mjs` passes.
**Done when:** the peak test reads under 300 MB with `light`, and Kokoro still speaks when chosen. If no voice meets step 2, write the numbers in `STATUS.md`, change nothing, and go to item 2.

## Item 2: What a call adds to the island's WebView (saves about 30–50 MB at peak)
**Goal:** a call adds under 25 MB to the island's renderer (now about 70).
**Files:** `src/island/voice.ts`, `src/voice/{speak,sentences,call,mic}.ts`, `src-tauri/src/speech/tts.rs`, `src-tauri/src/music.rs` (rodio is already a dependency), `scripts/peak-test.ps1`.
**Steps:**
1. Measure first: in the peak test log the island renderer's private memory before the call, at each reply, and 10 s after. In the test build, also log `performance.memory.usedJSHeapSize` and the count of live `AudioContext`, `Blob` URL and `<audio>` objects.
2. Fix what the numbers show. The likely causes, in order: (a) reply audio crossing IPC as base64 and being decoded in the page: play Kokoro and Fish Audio replies from Rust through rodio instead, sending the page only start, level and end events for his mouth; (b) blob URLs and decoded buffers kept after a reply: revoke and drop them when each clause ends; (c) more than one `AudioContext`: one per call, closed at hang-up; (d) captions kept for the whole call: keep the last 20 lines in the DOM.
3. Barge-in must still work: when the user talks over him, Rust playback stops within 150 ms.
**Test:** `tests/voice*.test.mjs` pass; the peak test's calls complete with the same reply times (within 10 %); manual: talk over him and he stops.
**Done when:** the peak test log shows the island renderer growing by under 25 MB across a call and returning to within 5 MB of its start 10 s after.

## Item 3: Idle with Settings open or left behind (saves about 78 MB in that state)
**Goal:** Groovy is under 150 MB ten minutes after the user stops using Settings, without losing their place.
**Files:** `src-tauri/src/lib.rs` (hub window, near line 997), `resources/{idle,webview,state}.rs`, `src/hub/shell.ts`, `scripts/smoke-test.ps1`.
**Steps:**
1. When the Settings window has been unfocused or minimised for the idle setting (default 10 minutes), save its page and scroll position and destroy the window. Reopening restores the page (`settings.html?page=…` already exists).
2. Never destroy it while it is focused, while a download or a sign-in is running, or while a text field has unsaved input.
3. Add a smoke-test step: open Settings, unfocus, wait past the (test-shortened) idle time, assert one WebView and the idle gate.
**Test:** the new smoke step; `settings-anim-test.ps1` still passes.
**Done when:** the new step reads under 150 MB, and reopening Settings lands on the same page.

## Item 4: One microphone pipeline at idle (saves about 10–25 MB and CPU with the wake word on)
This is done by **round 4B item 1**. Here only: after 4B item 1 lands, add a "listening idle" step to the smoke test (wake word on, models present, `tests/audio/not-wake.wav` played once, then 60 s) and gate it at the idle gate and 1 % CPU.
**Done when:** that step is in CI and green.

## Item 5: Make the work test gate, and finish 3D item 11 (the 446 MB case)
**Goal:** no scenario Groovy can reach on its own exceeds 300 MB.
**Files:** `scripts/work-test.ps1`, `src-tauri/src/resources/ceiling.rs`, `models.rs`, `/.github/workflows/windows-build.yml`.
**Steps:**
1. From the work test's log, list each process at the 446 MB moment. If the built-in `llama-server` is loaded together with a speech helper, add it to `make_room` so the two are never loaded together (speech first, since the user is waiting to be heard).
2. Per the lazy-load rule, confirm the built-in model is never started in cloud mode.
3. When the work test reads under 300 MB, make it gate at the measured number plus 15 MB.
4. Run upstream Coucou's latest release in the same smoke test (download in the step, don't commit it) and write its idle total next to Groovy's in `STATUS.md`. This closes 3D item 11.
**Test:** the work test passes with its gate on.
**Done when:** the work test gates, and 3D item 11 is ticked with the comparison recorded.

## Item 6: WebView2 idle trim (saves about 5–15 MB at idle; only after items 1–5)
**Goal:** idle under 100 MB.
**Files:** `src/main.ts`, `src/island/island.ts`, `vite.config.ts`, `src-tauri/src/resources/webview.rs`.
**Steps:**
1. Measure what the island loads at start: bundle size per chunk (`vite build` report) and the JS heap after 20 s idle.
2. Move anything not needed for the folded island behind a dynamic `import()` (wardrobe art for looks not worn, stage 6 art before stage 6, panel code, the recap and GitHub views).
3. Do not add WebView2 browser flags that disable the GPU or the audio service: the glass and the calls need them.
**Test:** frame test unchanged (p95 under 20 ms); smoke test idle.
**Done when:** idle is lower by a measured amount, written in `STATUS.md`, or the measurement shows under 5 MB to gain and the item is closed as "not worth it".

## Item 7: Checks
- Peak gate set to the measured peak plus 15 MB (target 300). Idle gate stays at measured plus margin (target 150 already met).
- One `[build]` at the end; confirm the Windows job ran. Write "ready for the owner" in `STATUS.md` with the before and after table.
