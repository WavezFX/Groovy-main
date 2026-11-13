# Changelog

Status words used below: **implemented** (code merged, unit-tested),
**CI-verified** (built and smoke-tested on a Windows runner), **unverified**
(written, not exercised end to end yet).

## 0.6.0 — Groovy AI

### Phase 1 — stabilisation and Windows build
- Windows build workflow (`.github/workflows/windows-build.yml`): type check,
  front-end and Rust tests, NSIS + MSI build, then on the Windows runner:
  launch of the portable `groovy.exe`, silent install, launch of the installed
  copy **with all outbound traffic blocked by a firewall rule**, Start-menu
  shortcut check, silent uninstall, SHA-256 checksums, artifacts.
- `scripts/smoke-test.ps1`: passes only when the process stays up, the backend
  logs its start, a webview loads the interface and calls the backend, settings
  and the local database are created, and no crash report is written.
- Crash handling: panics (the release build aborts on them) are written to
  `groovy.log` and `crash.log` before the process ends.
- Release names: `Groovy-AI-<version>-Windows-Setup.exe`, `Groovy-AI-<version>-Windows.msi`.

### Phase 2 — offline AI foundation
- **Local database** (`db.rs`): SQLite compiled in, schema migrations, refuses
  a database from a newer version instead of damaging it.
- **Memory** (`memory.rs`): long-term memories (preference / fact / project /
  task) separate from chat history; add, edit, pin, delete, clear per store,
  JSON export; “remember that …” in the chat; credential-looking text refused;
  pinned + newest memories go into the system prompt within a budget.
- **Chat history**: every finished turn saved (can be turned off), retention
  in days, prune at start; clearing chats never touches memories and vice versa.
- **Personality** (`assistant.rs`): name, six presets, humour, answer length,
  proactivity, answer language, custom instructions — all part of the system
  prompt sent to every provider.
- **AI modes**: local only (never sends to a cloud provider), online only,
  hybrid (falls back to the local model on network errors, missing key, 429/5xx,
  or when the daily online limit is reached). Online usage counted per day.
- **Built-in offline engine** (`models.rs`): llama.cpp `llama-server` b11000
  (CPU or Vulkan build, pinned URL + SHA-256), GGUF models from a pinned
  catalogue (7 models, 0.5B–4B, each with SHA-256 and licence), GGUF header
  reader, memory estimate (weights + KV cache + overhead), refusal when the
  model would not fit in the memory free now, loopback-only server, idle unload.
- **Performance profiles**: Lightweight (2K context, ≤4 threads, unload after
  5 min — suggested on ≤ 9 GB machines such as a Ryzen 7 4700U / 8 GB),
  Balanced, High performance.
- **Reminders and to-dos** (`tasks.rs`): stored locally, fire once (also after
  a restart), shown on the island with Done / In 10 min, spoken when voice is on.
- **System status** (`system.rs`): CPU, memory, disks, battery, Groovy's own
  memory, online messages today.
- **Control Centre**: the settings window has a sidebar with Assistant, Models,
  Memory, Reminders, Voice, Chat providers, Coding agents, Pills, Android phone,
  Breaks, General and System; light theme following Windows; visible focus;
  reduced motion respected. All new text translated into the 9 other languages.

### Phase 3 — voice
- **Explicit call state machine** (`src/voice/call-state.ts`): idle → listening →
  processing → speaking → listening, plus muted and error; the island's call
  bar shows the state, a live microphone level, mute, “stop talking” and hang up.
- **Interrupting**: tap “Stop talking”, or (opt-in, best with headphones)
  just speak over Groovy — a barge-in detector stops him and he listens.
- **Dictation (push-to-talk)**: the microphone button in the chat puts one
  spoken message in the text field to edit before sending.
- **Microphone choice** in Settings → Voice, with noise suppression; a
  microphone that disappears ends the call with a clear message.
- **Multilingual speech recognition**: Whisper tiny / base / small
  multilingual models besides the English ones, with a language picker
  (Spanish, German, Punjabi, Hindi… or auto-detect). Every speech-model
  download is now pinned to its size and SHA-256 and discarded on mismatch.

### Phase 4 — companion and wardrobe
- **Wardrobe Studio** in the Control Centre: live animated preview, look,
  outfit, **layered accessories by slot** (head / face / neck; costumes alone),
  **custom colours** for the body gradient, glow and eyes, search, favourites,
  presets (save / wear / delete), “Surprise me”, reset to default.
- **Wardrobe packs**: JSON presets import/export with validation
  (`docs/WARDROBE_PACKS.md`); packs can only name built-in items and colours.
- Colours and accessories follow Groovy everywhere: island and desktop window.

### Phase 5 — music and desktop tools
- **Local music player** (`music.rs`): music folders picked in a dialog,
  library indexed in SQLite with tags (title, artist, album, genre, duration)
  and cover art, search, favourites, playlists, queue with shuffle / repeat
  (off / all / one), seek, volume, auto-advance, resume after restart.
  MP3, FLAC, Ogg Vorbis, WAV, AAC/M4A through rodio + Symphonia.
- **Music by voice or chat**: “play my favourite songs”, “pause the music”,
  “next song”, “lower the volume”, “play the playlist I made yesterday”,
  “play some relaxing music” — answered with what actually happened.
- **Desktop tools with permissions** (`tools.rs`): open websites, apps (Start
  menu only) and your own folders, find files by name, reminders and to-dos
  from plain sentences (“remind me to call Sam at 5pm”), system status,
  clipboard. Each tool is Allowed / Ask me first / Off in Settings →
  Permissions; “ask” shows Allow once / Always / Don't under the answer; an
  audit log records every run, request and refusal.

### Phase 6 — Coding Studio
- **Studio window** (`studio.html`, `src/studio/main.ts`, backend `studio.rs`),
  opened from the tray or Control Centre → Coding agents: project folders
  (open or create), file tree and project search, an editor where every save
  goes into the file's history (restore any version; delete keeps a copy),
  a terminal where a command runs **only when Run is clicked** (streamed
  output, Stop, 10-minute limit, errors you can click to jump to), git status /
  diff / log and init / stage / commit / push one click at a time (push asks).
- **AI in the Studio**: the local model by default; code goes to an online
  provider only after an explicit OK naming the provider and the files.
  Answers are parsed into file proposals shown as diffs with **Apply**, and
  commands shown with **Run** — nothing is written or run by itself; paths
  leaving the project are refused.

### Voice, dictation and notes (ported from Coucou 0.5.0, MIT, rebranded)
- **Speech engine** (`speech/`): Parakeet through sherpa-onnx (25 European
  languages), models downloaded once (~700 MB, SHA-256 checked), microphone
  and system-audio capture. **Whisper as an alternative engine** for every
  other language (Punjabi, Hindi, Arabic, Japanese, Chinese…), chosen in
  Dictation settings.
- **Flow dictation** (`flow/`): hold a key anywhere, speak, release — the
  text is pasted where you type (clipboard restored). Dictionary,
  replacements, snippets, per-app styles, optional clean-up by AI, history
  and statistics, all on this computer. A floating **pill** window shows
  listening / thinking / messages.
- **Notetaker** (`notes/`): records calls (microphone + system audio),
  transcribes person by person, summary and to-dos, Markdown export.
- **« OK Groovy »** (`okgroovy/`): wake phrase (“OK / Hey / Yo Groovy”),
  then any request: pills, Spotify, your data, open an app, prepare an email
  (never sent by Groovy), agent skills. Answers spoken with the system voice
  (ElevenLabs optional).
- **Local-first brain** (`brain_ai.rs`): « OK Groovy », the dictation
  clean-up and note summaries use the local model (llama.cpp, Ollama, LM
  Studio or a custom server) by default; Claude only if chosen and a key is
  set (and never in “This PC only” mode). Web search exists only with Claude;
  locally Groovy says it has no internet access.
- **Control Centre** rebuilt on the ported hub: Groovy's pages plus a “Voice
  and dictation” group, a coloured band per page with Groovy's own character
  (Coucou's reserved Mochi artwork, icons and sounds are not used), narrow
  windows get a page strip on top.
- **Spotify on Windows** through the system media controls (track, artwork,
  play/pause, next/previous, seek, shuffle, repeat, volume); island zoom fix
  for mismatched display scaling.

### Skills Manager (agent skills per project)
- Drives Vercel's open-source `skills` CLI 1.7.2 (`npx -y skills@1.7.2`) for
  Claude Code, Codex, Cursor, OpenCode, Gemini CLI and GitHub Copilot:
  no shell, argument lists validated, `DISABLE_TELEMETRY=1` always, `--copy`
  on Windows, only an optional GitHub token passed. Needs Node.js ≥ 22.20;
  an “Install Node.js” button (winget) appears otherwise.
- Project analysis (manifests, dependencies, languages, tests, linters,
  CLAUDE.md / AGENTS.md, git activity), skills.sh search, ranking by the local
  AI (it can only rank real search results) with a rule-based fallback, and
  “could remove” suggestions.
- **Nothing installs without approval**: an install needs a preview the user
  was shown (full SKILL.md, source, install count, allowed tools and scripts
  highlighted). Removals are backed up to `.groovy/skills-trash/`; undo;
  updates skip skills you edited (fork to keep your own copy); SKILL.md
  editor that checks the frontmatter; new skills with `init`.
- « OK Groovy » can search, list and recommend skills; install and remove
  only open the Skills page to confirm. A quiet pill bubble suggests skills
  when an agent session starts in a project Groovy already manages.
- Vercel's licence and third-party notices: `windows/docs/licenses/skills-cli/`.

### Phase 7 — release and checks
- CI also runs the Skills CLI end to end (add, list, remove with backup) and
  reports startup time and idle memory in the smoke tests.
- `windows.yml` publishes the `Groovy-AI-…` installer names; Node 24.
- CI cargo cache holds only downloaded crates and is never saved after a
  cancelled run (a half-written cache broke run 8).

### Known limitations
- No GGUF model or llama.cpp runtime ships inside the installer: both are
  downloaded on request from Settings → Models (sizes shown before download).
- Small local models are far weaker than cloud models and slow on a laptop CPU.
- The Ollama installer download (pre-existing) is not checksum-verified.
- If Groovy is killed (not quit), a running `llama-server` may outlive it.
- Text-to-speech uses the voices installed in Windows (offline ones work
  without internet); Piper voices are not integrated yet.
- Wake-word detection runs speech-to-text on short segments (no dedicated
  keyword model); custom wake words and voice identification are not implemented.
- With Whisper as the dictation engine, word timings in notes are spread
  evenly over each stretch (Whisper gives none), so speaker turns are rougher.
- Flow dictation, the Notetaker and « OK Groovy » have unit tests and build on
  Windows, but have not been exercised with a real microphone on Windows yet.
- The Skills Manager needs Node.js ≥ 22.20 and network access to npm,
  skills.sh and GitHub; it is off otherwise.
