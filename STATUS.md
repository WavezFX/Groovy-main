# Groovy AI — project status

Branch `groovy-ai-phase1` on `WavezFX/Groovy-main`. Version 0.6.0. See
`CHANGELOG.md` for the feature details. Status words: **done** (written and
unit-tested), **CI-verified** (built and run on a Windows runner),
**partial**, **not started**.

## Verified results

| Check | Result |
| --- | --- |
| Baseline (original 0.5.1) | 413 front-end tests, 479 Rust tests, all passing |
| Now, local (Linux) | 448 front-end tests, 577 Rust app tests + 34 relay, all passing; `tsc` clean; production front-end build OK |
| Windows cross-check (`cargo xwin check`, no voice feature) | compiles |
| Linux launch under a virtual display | stays up, creates `groovy.db`, both windows load the interface |
| Windows CI run 1 (commit b8e20b5) | type check, tests, Rust tests on Windows and **both installers built** (`Groovy-AI-0.6.0-Windows-Setup.exe` 6.1 MB, `.msi` 7.4 MB). Smoke test of the portable exe: process up, backend started, interface loaded, database created, no crash — **one failure**: no `settings.json` on first run. Fixed in 1fc13b6 (first run now writes settings). |
| Windows CI run 3 (ef0095c) | full pipeline passed: build, portable smoke test, silent install, offline smoke test (firewall-blocked), uninstall, checksums, artifacts |
| Windows CI runs 8–13 (voice, dictation, skills, studio added) | 8: corrupt cargo cache; 11: link error (C runtime mismatch, sherpa-onnx /MT vs whisper.cpp /MD); 13: **installers built and tested by the owner**, Rust tests still failing to link from a stale cached debug build. Fixes pushed (static CRT for whisper.cpp, new cache key); next run not checked yet. |

## Fixes round 1 (FIXES_ROUND1.md, from the owner's first Windows test)

| Item | State | How it was checked |
| --- | --- | --- |
| 0. Rust tests fail on Windows CI | fix pushed (568991e): the test step reused a `/MD` whisper.cpp from the cache | waiting for the next Windows run |
| 1A. Wardrobe buttons greyed out (Auto → witch hat in October) | **done**: items are never disabled; a click replaces what is in the slot, the seasonal item becomes the user's own (outfit → None), a costume makes way; a one-line hint says what happened | 6 new unit tests (October, December, costume, slot replace, layer list) |
| 1B. Failing actions look like dead buttons | **done**: user actions that fail now show the backend's error — a toast in the Control Centre and Studio, the floating pill for the island and desktop Groovy (`notify_error`); unhandled promise rejections are shown too; 8 places that swallowed errors now report them; music controls with nothing queued explain why; a full pill list says so instead of a greyed switch | type check, tests; **not yet clicked through on Windows** |
| 2. Local AI eats the PC (llama-server ~2 GB, 91 % memory) | **done** — 1) new "Groovy's AI" setup at the top of Assistant (opened at first start, reminded in the pill until chosen): Gemini free key first, then a Claude/OpenAI/OpenRouter key, then a Claude Code/Codex subscription through its CLI, local last with a RAM warning (≤ 9 GB: "will slow everything down", recommended only from 16 GB). 2) `models::ensure_running` is the one gate: llama-server never starts unless local AI was chosen; no hidden local fallback in hybrid, and a reached online limit no longer starts it. 3) idle unload Lightweight 5 min, Balanced 10 min, Performance never; "Local model loaded, using X GB: Unload" in the tray and on the Models page. 4) Windows job object (kill on close) for llama-server, the Ollama server Groovy starts, the subscription CLIs and the skills CLI, plus the existing stop on exit. 5) Lightweight profile chosen from RAM at first start. One brain list (`brain_ai::order`): chat provider, other keys, subscription, local. | Rust unit tests for the order (no keys → nobody, Gemini key → Gemini, local only when chosen, local-only never online), routing and settings; front-end tests for the setup choices and RAM advice; Windows target compiles (`cargo xwin check`). **Not yet run on a Windows PC.** |
| 3. Dictation and speech-to-text didn't work | **done (code)** — guided "Set up speech" page, opened the first time dictation or « OK Groovy » is turned on, from Record a call when the model is missing, and from the pill ("Set up") when the hotkey is pressed without a model: 1) microphone picker with a live level meter (`speech_check_start`, "speech-level" events); 2) engine choice and the model download with size and progress (Parakeet) or a pointer to the Whisper model; 3) a 5-second test that shows the words heard with the chosen engine. The pill shows "Hold {key} to dictate" on hover; the Flow page already showed the key. | Rust test for the level meter; type check and front-end tests. **The end-to-end test (dictation, Notetaker, call, OK Groovy × Parakeet/Whisper × cloud/local brain) needs a Windows PC with a microphone and has not been run.** |
| 4. The island as the user's own launcher | **done (code)** — Control Centre → My island: presets Developer (today's agents view, the default) / Everyday / Custom; tiles added from the Start-menu app list, a website, a folder, a service pill, Mail, Music, Agents, Reminders, This PC; drag (or arrows) to reorder, compact or wide, remove. The island opens on the tiles (a new `tiles` view) unless the preset is Developer or an agent is at work; the house tab goes there. Mail pop-ups read Windows' own notification feed (UserNotificationListener, one permission prompt) and show sender, subject, Open, Dismiss, from all mail-looking apps or the ones chosen. Hooks and agents stay in Coding agents. | Front-end tests for the tile list (untrusted JSON, presets, editing, mail filter); Rust tests for launch safety (only real shortcuts, http(s) websites, existing folders); Windows target compiles. **Windows may refuse notification access to an unsigned, unpackaged app: if so the Mail tile says so; not tested on a PC.** |
| 5. Spotify: explain it in the app | **done** — the Spotify card and the Music tile say "Play something in the Spotify app and it appears here" when nothing plays; "Get Spotify" opens its Microsoft Store page (was the website). Both the Store version (`SpotifyAB.SpotifyMusic…!Spotify`) and the spotify.com installer (`Spotify.exe`) are recognised by the media-session filter and the installed check (existing tests). `open_url` now reports a refused address instead of doing nothing. | front-end test updated; Rust tests. **Not checked on a PC with each Spotify version.** |
| 6 | Tray already had Open Groovy and Settings…. Added: Ctrl+Alt+, ("Open the Control Centre", rebindable in Shortcuts); a native right-click menu on the island and the desktop companion (Open Groovy, Settings…, Wardrobe…, Coding Studio; text fields keep their own menu); "Run setup again" in Control Centre → General (Groovy's AI, speech). Also fixed `open_n8n` ignoring the new `open_url` error. | npm test 455/455, tsc clean, cargo test 583 passed, xwin check clean. Right-click menu and the shortcut need checking on a real PC |

## Done, by phase

- **Phase 1 — stabilisation, Windows build:** imported source; Windows CI
  (`.github/workflows/windows-build.yml`) that builds, smoke-tests the
  portable exe, installs, smoke-tests offline (firewall-blocked), checks the
  Start-menu entry, uninstalls, writes SHA256SUMS and uploads artifacts;
  `scripts/smoke-test.ps1`; crash handler (`crash.log`); first-run settings;
  versioned installer names; cargo cache saved even on failure.
- **Phase 2 — offline AI:** SQLite database with migrations; memory and chat
  history (separate, editable, exportable); personality settings that change
  the system prompt; local / online / hybrid modes with fallback and daily
  limit; built-in llama.cpp engine with pinned, SHA-256-checked downloads and
  a 7-model GGUF catalogue; GGUF reader and RAM estimate; performance profiles
  (Lightweight suggested for 8 GB / Ryzen 7 4700U); reminders; system status;
  Control Centre with sidebar, light theme, translations.
- **Phase 3 — voice:** call state machine, mute, interrupt (button or opt-in
  barge-in), dictation, microphone choice and disconnect handling,
  multilingual Whisper with language picker, pinned speech-model downloads.
- **Phase 4 — wardrobe:** Wardrobe Studio, layered accessories by slot,
  custom colours, presets, favourites, search, randomise, reset, validated
  JSON packs (`windows/docs/WARDROBE_PACKS.md`).
- **Phase 5 — music and tools:** local music player (library, tags, art,
  playlists, favourites, shuffle/repeat/seek/volume/resume), music by
  voice/chat; desktop tools (sites, apps, folders, file search, reminders and
  to-dos from sentences, system status, clipboard) with allow/ask/deny
  permissions, approval buttons in the chat and an audit log.

## In progress — Phase 6, Coding Studio (partial)

Committed but **not reachable from the UI yet**:
- `src-tauri/src/studio.rs` (tested): workspaces from a folder picker,
  path-safe file read/write with history and revert, trash instead of delete,
  project search, command runner (only with explicit approval, streamed
  output, timeout, kill, audit), read-only git views plus explicit
  init/stage/commit/push, AI questions about files with the receiving
  provider reported so online use needs consent (`chat::complete`).
- `src/studio/ai.ts` (tested): parses AI answers into file proposals and
  commands, refuses paths outside the project, parses build errors.
- CodeMirror 6 packages installed; Bridge entries for every Studio command.

Still to write: `src/studio/main.ts` (the window: file tree, editor tabs,
diff and history panels, terminal with Run/Stop, problems list, git panel,
AI panel with consent prompt and Apply/Run buttons, bounded "fix it" loop),
`studio.html` (kept aside, removed from the Vite build until the page
exists), a button to open the Studio from the Control Centre and the tray,
and its translations.

## Not started / remaining

- **Phase 6 rest:** GitHub pull requests from the Studio; autonomous
  "create a project" flow end to end.
- **Phase 7:** UI polish pass, end-to-end tests, performance benchmarks
  (startup, idle memory, model load, voice latency — none measured yet),
  security review, release workflow update (`windows-release.yml` still uses the old
  file names), signed installers.
- **Spotify on Windows:** the pill only works on Linux (MPRIS); a Windows
  backend through the system media controls is planned.
- **Voice:** Piper TTS, a dedicated wake-word engine, custom wake words,
  optional voice identification, live partial transcripts.
- **Companion:** adjustable size, animation-quality setting wired to the
  profile, click-through toggle, new animations (walking, music reaction
  beyond dancing), particles.
- **Memory:** optional semantic retrieval.
- **Integrations:** calendar and email; a plugin interface.
- **Model tool calling** for local models (today tools are recognised from
  plain sentences, not by the model).
- **Known gaps:** Ollama installer download not checksum-verified (existing
  code); a killed Groovy can leave `llama-server` running; no media-key
  support for the local player.
