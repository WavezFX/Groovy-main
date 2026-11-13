# Fixes round 3F: dead code, duplicate logic and bloat (24 Oct)

**Do this first, before round 3E.** The owner wants every piece of dead code, duplicate logic and needless startup work gone, to cut memory and CPU. Commit each item separately with "Fixes round 3F, item N: …" as the subject, tick it in `WORK_QUEUE.md` in the same commit, push, and keep the build green.

**Rules for this round:**
- **No user-facing feature is removed.** Where something is "made lazy", it still works exactly as before, it just doesn't start until used.
- Every deletion is proven unused first: grep, `cargo build`, `tsc --noEmit`, the tests, and CI all pass.
- In `STATUS.md`, record for each item: lines removed, threads before/after, and idle private working set and CPU before/after (from the CI memory step).

## Item 1: Delete what's provably dead
- `src-tauri/src/backdrop.rs` (the composition-backdrop experiment, only active with `GROOVY_BACKDROP=composition`): delete the module, its `mod`, its `backdrop::start` call in `lib.rs` setup and any CI/env references.
- Tauri command `load_report`: the only command the frontend never calls. Remove it and its registration (if a test uses it, keep it test-only).
- Unused frontend exports (used nowhere, not even tests): `callLabel` (views/chat.ts), `isSpeaking` (voice/speak.ts), `PAGE_IDS` (hub/shell.ts), `pageHeader`, `learnMore` (hub/ui.ts), `releaseMiniBot`, `miniBotCount` (mochi/minibots.ts), `OutfitSelection`, `OUTFIT_LABELS` (mochi/wardrobe.ts), `STATE_SOUND` (mochi/engine.ts), `StudioFinished`, `BridgeEvent` (core/bridge.ts), `WAKE_STRIP_W`, `WAKE_STRIP_H`, `colorForProject` (core/layout.ts), `allKeys` (i18n/i18n.ts). Delete them, plus any code only they used.
- Exports used **only by tests** (`liveTextCount`, `openPlanColor`, `micOpen`, `spriteCount`, `OUTFIT_SELECTIONS`, `inSlot`, `tilesHome`, `CELLS_E`, `newerLook`, `MAX_SECONDS`, `isLocalProvider`, `isHookPill`, `TINT_RGB`, `altGrClashes`, `monthShort`, `isComplete`): if the test is testing dead behaviour, delete both; if it's a real helper the app logic depends on, keep it.
- Run a dead-code sweep and remove what it finds: `cargo clippy -- -W dead_code -W unused` (fix, don't `#[allow]`), `cargo machete` (or `cargo udeps`) for unused crates, and `knip` or `ts-prune` for the frontend (unused files, exports, deps in `package.json`, unused CSS classes). List everything removed in `STATUS.md`.

## Item 2: Test-only code out of the release build
- `worktest.rs` (307 lines) is a CI harness but compiles into the release app and `worktest::start` runs at every launch. Put it behind a cargo feature `worktest` (off by default), enabled only in the CI steps that need it. The release build must not contain it.
- Same check for any other CI/smoke/probe-only code paths (e.g. probe loops in `runtime_state.rs` that exist only for tests): feature-gate them.

## Item 3: One resource manager instead of six
Today these overlap: `idle.rs`, `loads.rs`, `ceiling.rs` (own polling loop), `webview_memory.rs`, `runtime_state.rs` (probe loop), and **three reapers** (`models.rs` ~851, `speech/mod.rs` ~476, `speech/tts.rs` ~262), each on its own thread sleeping on `idle::CHECK_EVERY`.
- Merge into **one module** (e.g. `resources.rs`) with one thread (or none: timers on the scheduler from item 4) that owns: what's loaded, last use, unload-after-idle, the 400 MB ceiling, WebView trimming and the Standby/Conversation/Offline state.
- Each engine registers itself (`register(name, size, unload_fn)`) instead of running its own reaper.
- All existing behaviour and tests (3D items 7, 9, 10) keep passing.

## Item 4: One scheduler, event-driven, instead of always-on polling
There are about 44 spawned threads and many always-on loops: glass.rs (200 ms, 1 s, 30 s), island cursor poll, desktop.rs ticks, mail (20 s), tasks (20 s), tray (15 s), spotify (300 ms), integrations, ceiling, runtime_state probe, lib.rs `STEP`.
- Replace them with **one scheduler thread** (a timer wheel or a small tokio runtime that already exists) where each job has a cadence and a **condition** (only while visible, only while enabled, only while Spotify is playing, etc.).
- Prefer **events over polling**: window/monitor/theme changes from Windows messages (`WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE`, `WM_DWMCOMPOSITIONCHANGED`), cursor via a low-cost hook or only while the island is near the cursor, Spotify via its session (GSMTC `MediaPropertiesChanged`) instead of 300 ms polling, mail via IMAP IDLE / push where available.
- When the island is collapsed and nothing is playing, **no job runs more often than every 30 s**.
- Target: idle CPU ≈ 0 % (CI: average < 0.3 % over 60 s idle), thread count reported in CI and reduced by at least half.

## Item 5: One speech runtime (drop whisper.cpp)
Groovy links two ML runtimes: whisper-rs (whisper.cpp, `voice.rs`, the `voice` feature) **and** sherpa-onnx (Parakeet, wake word, Kokoro).
- Move dictation onto **sherpa-onnx** (its Whisper/Moonshine/Parakeet offline recognisers), keeping the same models choice and the dictation fixes from round 3B item 6 (names and model numbers, the hotword/bias list).
- Remove `whisper-rs`, the `voice` feature, and the CMake/LLVM build requirements from CI.
- Dictation accuracy and latency must be at least as good (keep or add the CI test). Report the installer size before/after.

## Item 6: Translations loaded per language
`i18n/strings.json` (204 KB) and `i18n/extra.json` (1,052 KB), all 9 languages, are imported into **every window**.
- Build step splits them into one file per language. English is bundled; others are fetched with a dynamic `import()` only when that language is chosen, and only once.
- No string may go missing (test: every key exists in English, and switching languages works).

## Item 7: Startup does only what's needed
In `lib.rs` `.setup`, these all start at every launch: `phone_link::PhoneLink::new` / `start_if_enabled`, `tasks::spawn`, `mail::spawn`, `island_tiles::spawn_mail_watch`, `music::init`, `integrations::start`, `spotify::sync`, `pipe::start`, `glass::spawn_watch`, `island::spawn_cursor_poll`.
- Each starts only when its feature is **on and in use** (its tile is on the island, an account is connected, the setting is enabled), and stops when turned off.
- **Phone Link** (8,700 lines plus the `relay` crate, rcgen, qrcode, mdns-sd, tokio-tungstenite): off by default, so nothing of it is created or allocated until the user turns it on. Keep the feature.
- **Music-folder playback** (rodio, lofty), **recap**, **LiveKit**, **Studio**, **coding agents**: same rule: nothing allocated until used.
- CI: with a fresh profile, list the threads at idle and fail if any of these features' threads exist.

## Item 8: Merge the two Voice settings pages
`settings/voice.ts` and `hub/pages/voice.ts` both render voice settings. Keep one page (matching `docs/design/settings-v2/Voice.dc.html`), with every option from both, and delete the other. Check the rest of `settings/` and `hub/` for any other page rendered twice and merge the same way.

## Item 9: Checks
- Release build has no `worktest`, no `backdrop`, no whisper-rs (check the binary's symbols / `cargo tree`).
- Idle: private working set lower than before this round, CPU ≈ 0 %, thread count at least halved, all numbers in `STATUS.md`.
- Peak stays under 400 MB; frame-time test passes; every feature still works (the existing CI tests and screenshots).
- `cargo clippy`, `cargo machete`, `knip` clean.

## Not in this round (owner hasn't decided)
- **Linux code** (`portal.rs`, `flow/os_linux.rs`, `platform/linux*.rs`, `tauri.linux.conf.json`, `linux/arch`): already excluded from Windows builds by `cfg`, so it costs no memory. Leave it.
- **Licence:** `Cargo.toml` and `phone_link` say MIT; `LICENSE` says GPL-2.0. Don't change; the owner will decide.
