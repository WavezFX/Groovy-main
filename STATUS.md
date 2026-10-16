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

## Fixes round 2

| Item | What was done | Tested |
|---|---|---|
| 1 | The island's gear (next to the sound button, now full colour and slightly larger) opens the Control Centre; tooltip "Settings (Ctrl+Alt+,)" follows the bound shortcut (plain "Settings" when it's off). The quick settings (sound, volume, auto-close) open from the small arrow next to it or a long press (450 ms) on the gear. | npm test 456/456, tsc clean; needs a look on the PC |
| 2 | Auto-close choices are now **When I leave** (folds 0.3 s after the pointer leaves), 1, 3, 5, 10 and 30 s, both in the island's quick settings and in Control Centre → General (a menu instead of the 5–120 s number box; older values show the nearest choice). "When I leave" is the default for new installs; an existing user with another delay gets a one-time pill offer ("Use it"). Coming back within the grace period cancels the fold. Stays open while an alert waits, when pinned (Ctrl+P), on a call, while dictating or while a text field in the island has focus (re-checked every second). The countdown bar is hidden under 3 s. | npm test 463/463 (5 new FSM tests: 0 s, 1 s, pointer back, typing hold, pinned; 2 autoclose tests), cargo test 584 passed (default + old-file test) |
| 3 | **Judged by the PC:** a model suits the PC when it needs ≤ 40 % of total memory (≈ 3.2 GB on 8 GB); `memory_refusal` no longer compares with this second's free memory × the profile share. Too big → refused with "pick a lighter one". Suits but memory short now → refused once with **Free up memory** (the 6 apps using the most memory) and **Load anyway**. **Catalogue** marks every entry "Fits this PC" / "Too big for this PC" and gained Gemma 3 1B Instruct Q4_K_M (ggml-org, Gemma Terms of Use), Qwen3 0.6B Q8_0 (Qwen, Apache-2.0) and Qwen3 1.7B Q4_K_M (unsloth, Apache-2.0); SHA-256 and size from Hugging Face, each file downloaded here and checked. **Light local** (Assistant → Groovy's AI, and Models): one model by RAM — Qwen2.5 0.5B up to 6 GB, **Gemma 3 1B up to 12 GB (the 8 GB PC)**, Qwen3 1.7B above — with size, memory and speed, one click installs the CPU engine if missing, downloads, and switches Groovy to it. Light models run with a 2048 context; Groovy's own engine asks Qwen3 not to "think" first. **One engine:** the default chat is no longer Ollama (that is why both ran); a saved "ollama" without Ollama installed becomes Groovy's engine; starting llama-server unloads Ollama's models (keep_alive 0) or stops the Ollama server Groovy started; chatting with Ollama stops llama-server. | Measured with llama-server b11000 built from source here (Linux x64, Xeon 2.8 GHz, 4 threads, ctx 2048, as the Lightweight profile runs it; 3 short chat turns each): Qwen2.5 0.5B 591 MB / 33 tok/s / first token 170 ms; Qwen3 0.6B 906 MB / 24 tok/s; **Gemma 3 1B 971 MB / 18 tok/s / 535 ms, the most natural answers**; Llama 3.2 1B 1366 MB / 21 tok/s; Qwen3 1.7B 2000 MB / 16 tok/s. Memory on 8 GB Windows still to be confirmed on the PC (Windows adds a little). npm test 465/465, cargo test 588 passed. |
| 4 | **One speech engine:** island calls (and « Hey Groovy » in the island) now transcribe with the engine dictation, the Notetaker and « OK Groovy » use — Parakeet, or Whisper when chosen — instead of a separate Whisper download; the island no longer downloads a Whisper model by itself. Voice → **Recognition**: "Parakeet (fast, 25 languages)" / "Whisper (more languages)", with Set up speech when it's missing (Whisper's model picker only shows when Whisper is chosen). **Stages in the call bar:** Listening… → Heard "…" · Thinking (Gemini / Local gemma 3 1b it…) → Speaking…; a failed stage keeps the call up with the reason and one fix button (Download speech model / Choose an AI / Pick a voice) instead of hanging up silently — no speech model, a transcription error, the brain's error, no TTS voice or a voice error are each caught. **Faster answers:** a spoken turn adds "reply in 1–2 sentences unless asked for more, plain words" to the system prompt; the reply is read sentence by sentence as it streams in (speaking starts at the first full sentence); interrupting drops what's still queued. **Test my voice setup** (Voice → Check): microphone level (2 s), a 3 s recording transcribed, "say hello" to the brain the calls use (outside the chat, nothing saved), the answer spoken — ✓/✗/skipped with the reason for each. **voice.log** at %LOCALAPPDATA%\Groovy\logs\voice.log: call start/end, brain, lengths and timings, every failure (never the words said). | npm test 471/471 (6 new: sentence streaming, first sentence before the end, failing voice, interruption, stage text, voice test), cargo test 589 passed. Spoken conversation with Gemma 3 1B through llama-server as Groovy runs it (voice rule, streamed, ctx 2048, 4 threads, here): 4 turns, first sentence after 0.7–1.6 s, whole answers 1–2 s, 1–3 sentences each. Not testable here: microphone, Parakeet on real speech and Windows voices end to end; Gemini and a subscription brain go through the same chat path (chat::send/complete) as before. |
| 5 | **Purpose on the page:** Dictation now opens with one line ("types what you say into any app — Word, Discord, your browser… It needs no AI; an AI only tidies the words (Styles) when one is set up"), the hotkey shown big, and a **Try it here** box. **Never blocks on the brain:** with no AI set up, or "As said", the raw words are pasted (already so); the clean-up now has one 5 s deadline for all brains together (before, each brain tried got 4 s, so three brains could hold a dictation ~12 s). **Shared engine:** dictation uses the speech engine of item 4; a missing model is announced in the pill with its setup (round 1). **Key check:** "Check the key" on the Dictation page waits 6 s for the hook to see the hotkey (new `flow-key-seen` event); if it doesn't, it says so ("some laptops don't have this key, or another app holds it") with **Choose another key**. | npm test 471/471, cargo test 590 passed (new: no AI → words pasted as said), Windows cross-check clean. **Not tested here — needs the PC:** Notepad, Word, Chrome and Discord paste, and the right-Ctrl hook on the owner's laptop (use Check the key). |

**Round 1 item 0 (Windows CI): done.** Run 30 (9fc81bc): installers, smoke, offline install/uninstall, type check, front-end tests, Rust tests and the Skills CLI end-to-end step **all passed on Windows**. The CLI step had failed because npx started the `skills` CLI through cmd.exe, which couldn't find it. Groovy now installs the pinned `skills@1.7.2` once into its own folder (no install scripts) and runs `bin/cli.mjs` with node directly, with no shell.

**Round 2 done means — where each stands:**
- Gear button on the island: done (item 1).
- The island closes when the mouse leaves (by default): done (item 2).
- A light local model downloads and holds a spoken conversation on an 8 GB PC: Gemma 3 1B is the pick for 8 GB, set up in one click, measured at 971 MB and 18 tok/s here with a 4-turn streamed spoken conversation (first sentence 0.7–1.6 s). Still to confirm on the 8 GB Windows PC.
- A voice call works end to end with Gemini and with the light local model: the pipeline now shares one speech engine, shows each stage and fails visibly; the end-to-end call (microphone → speech → brain → Windows voice) needs the PC.
- Dictation types into Notepad with no AI configured: the code pastes raw words without an AI (tested); Notepad itself needs the PC.
- The voice test reports every stage: done (Voice → Check → Test my voice setup).

## Fixes round 3 (FIXES_ROUND3.md)

| Item | What was done | Tested |
|---|---|---|
| 1. One island only | The bottom pill is gone: `pill.html`, `src/pill/`, `flow/pill.rs`, its Vite entry, its capability, the `pill_*` commands, the platform helpers that placed it, and the "Where the pill is" setting (an old `edge` in `flow.json` is ignored). What it held now lives in `flow/live.rs`, which has no window and sends `live-state` (dictation, a recorded call, « OK Groovy »), `live-bands` (the nine voice levels) and `island-notice` to the island. Every `announce` (no speech model → Set up, microphone refused, words copied, skills updates, "Choose Groovy's AI", the close-on-leave offer, errors from the island) opens the island on its notify card, with the same icon, colour, text, button and time; it waits while a permission card, a reminder or a call is up, and stays while the pointer is on the island. « OK Groovy »'s spoken answers (`voice-say`) are said by the island (ElevenLabs MP3, else the system voice in the answer's language). "Paste the last dictation again" is an island action and a global shortcut, **Ctrl+Alt+V** (it waits for the keys to be let go before sending Ctrl+V). The only other windows are the Control Centre, the Coding Studio and the desktop Groovy (sent there on purpose). | `tests/live.test.mjs` (busy states, notice queue, voice choice, no pill window); shortcut tests in TS and Rust; Rust tests; Vite build; Windows target compiles. **Dictation, the Notetaker and calls without the pill window need the owner's PC.** |
| 2. The new island layout (glass) | **Home view (F):** the island now opens on "Up next" + tiles (`src/views/home.ts`, rules in `src/core/home.ts`); the agents overview is the Agents tile. Up next picks a Cal.com meeting within 30 min (its https video link opens; Groovy now reads `meetingUrl`), then new mail, then an agent that needs you or finished, then a reminder due within 15 min, else a greeting; clicking opens it. Dictate / Call / Record start Flow dictation (hands-free), the island voice call, and the Notetaker. Claude Code's hook status is only on its tile now. Right card: 4 × 2 live tiles (GitHub "n PRs", Vercel "Ready", Resend "n sent", Notion pages, Cal.com next time, agents' current step), dashed **+ Add tile** last, page dots and the mouse wheel beyond 7. Spotify with nothing playing is a small tile with working ⏮ ▶ ⏭ (▶ opens Spotify when it isn't running). **Player (E):** as soon as anything plays, a wide player (cover, title, artist, progress, ⏮ ⏯ ⏭) over one row of 4 tiles, the Spotify tile not repeated; 5 s after pause/stop it folds back; 250 ms transform/opacity only. Windows: the media session is no longer Spotify-only — Spotify while it plays, else any app that plays, labelled with its name ("Chrome", "Media Player"…). **Glass:** `glass.rs` asks Windows for Acrylic (Windows 11) or blur (Windows 10) on the island window and clips it with a window region that follows the island (square top, rounded bottom), pushed with each shape change; the page adds the `rgba(18,20,24,0.42)` tint and translucent white cards. Control Centre → General → **Appearance → Island style: Glass / Solid**; Solid is used on its own while Windows' Transparency effects are off or battery saver is on (checked every 30 s). Screenshots of the built views (browser, no Acrylic): `docs/design/island-round3/built/`. Text sizes are the mockup's ÷ 1.5 with a 9.5 px floor so labels stay readable. | `tests/home.test.mjs` (priority order, 30-min window, cells and pages, the 5 s hold); Rust tests for the glass decision and region maths, the island style setting, the player pick (Spotify playing > any playing > Spotify paused) and app names; Windows target compiles with its tests (`cargo xwin check --tests`). **Not measured: the idle CPU/RAM of glass vs solid and while opening/closing need the Windows PC** (Task Manager → Details, `groovy.exe` + its `msedgewebview2.exe` children, 60 s idle each way). Also to check there: the Acrylic clipped by the region (edges are not anti-aliased by Windows), and contrast over a white wallpaper. |

