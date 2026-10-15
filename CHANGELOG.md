# Changelog

Status words used below: **implemented** (code merged, unit-tested),
**CI-verified** (built and smoke-tested on a Windows runner), **unverified**
(written, not exercised end to end yet).

## 0.6.0 — Groovy AI, phase 1 + 2 foundation (in progress)

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

### Known limitations
- No GGUF model or llama.cpp runtime ships inside the installer: both are
  downloaded on request from Settings → Models (sizes shown before download).
- Small local models are far weaker than cloud models and slow on a laptop CPU.
- The Ollama installer download (pre-existing) is not checksum-verified.
- If Groovy is killed (not quit), a running `llama-server` may outlive it.
