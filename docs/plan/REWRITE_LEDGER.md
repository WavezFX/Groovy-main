# Copied-code ledger (round 4E item 3, 20 Nov 2026)

Which source files still hold code from the MIT-licensed project Groovy's code started from, so its credit in `windows/THIRD_PARTY_NOTICES.md` can be dropped once no file here is **inherited**. Not legal advice. Regenerate with `python3 windows/scripts/rewrite-ledger.py` (full history needed).

**Method.** The repository's first commit holds only `LICENSE` and `README.md`; the code arrived in `d7a8eca` (15 Oct 2026, "Import Groovy 0.5.1 source"), the start used here. For each file under `windows/src`, `windows/src-tauri/src` and `windows/hook`: **Groovy's own** = added after the start (or a character file shown in `docs/plan/CHARACTER_MOTION.md` §1 to hold no line of the original); **rewritten** = there at the start, now under 20 % of its substantive lines (20+ characters, comments skipped) the same; **inherited** = there at the start and still 20 % or more the same. The 0.5.1 import already held Groovy changes, so the percentage over-states the original's share, never under-states it.

**Totals:** 122 inherited, 7 rewritten, 173 Groovy's own (302 files). Character (`windows/src/mochi/`, `core/anim.ts`): every file is rewritten or Groovy's own. Files rewritten with their interface kept show how many lines still match, so the reader can judge.

## Inherited files, most visible first, and the round that rewrites them
Character first, plumbing last; plumbing is not rewritten in round 4E. Phone link files also carry their own author's MIT credit and wait on the Android source.

| Rank | Area | File | Same as at the start | Round |
|---|---|---|---|---|
| 1 | island and views | `windows/src/core/layout.ts` | 89% | 6 |
| 2 | island and views | `windows/src/core/pills.ts` | 88% | 6 |
| 3 | island and views | `windows/src/desktop/main.ts` | 93% | 6 |
| 4 | island and views | `windows/src/island/agents.ts` | 100% | 6 |
| 5 | island and views | `windows/src/island/brain.ts` | 100% | 6 |
| 6 | island and views | `windows/src/island/breaks.ts` | 87% | 6 |
| 7 | island and views | `windows/src/island/desktop.ts` | 91% | 6 |
| 8 | island and views | `windows/src/island/fsm.ts` | 73% | 6 |
| 9 | island and views | `windows/src/island/hooks.ts` | 100% | 6 |
| 10 | island and views | `windows/src/island/integrations.ts` | 100% | 6 |
| 11 | island and views | `windows/src/island/island.ts` | 69% | 6 |
| 12 | island and views | `windows/src/island/phone-link.ts` | 98% | 6 |
| 13 | island and views | `windows/src/island/phone-services.ts` | 100% | 6 |
| 14 | island and views | `windows/src/island/shortcuts.ts` | 96% | 6 |
| 15 | island and views | `windows/src/island/spotify.ts` | 100% | 6 |
| 16 | island and views | `windows/src/island/voice.ts` | 47% | 6 |
| 17 | island and views | `windows/src/style.css` | 69% | 6 |
| 18 | island and views | `windows/src/views/chat.ts` | 71% | 6 |
| 19 | island and views | `windows/src/views/diff.ts` | 100% | 6 |
| 20 | island and views | `windows/src/views/dom.ts` | 99% | 6 |
| 21 | island and views | `windows/src/views/github.ts` | 100% | 6 |
| 22 | island and views | `windows/src/views/icons.ts` | 93% | 6 |
| 23 | island and views | `windows/src/views/integrations.ts` | 91% | 6 |
| 24 | island and views | `windows/src/views/markdown.ts` | 100% | 6 |
| 25 | island and views | `windows/src/views/recap.ts` | 100% | 6 |
| 26 | island and views | `windows/src/views/spotify.ts` | 94% | 6 |
| 27 | island and views | `windows/src/views/ticker.ts` | 100% | 6 |
| 28 | island and views | `windows/src/views/upload.ts` | 100% | 6 |
| 29 | island and views | `windows/src/views/usage.ts` | 100% | 6 |
| 30 | island and views | `windows/src/views/views.ts` | 76% | 6 |
| 31 | Settings and panels | `windows/src/recap/recap.ts` | 100% | 7 |
| 32 | Settings and panels | `windows/src/recap/share.ts` | 100% | 7 |
| 33 | Settings and panels | `windows/src/recap/summary.ts` | 100% | 7 |
| 34 | Settings and panels | `windows/src/settings/agents.ts` | 100% | 7 |
| 35 | Settings and panels | `windows/src/settings/breaks.ts` | 100% | 7 |
| 36 | Settings and panels | `windows/src/settings/colors.ts` | 100% | 7 |
| 37 | Settings and panels | `windows/src/settings/main.ts` | 78% | 7 |
| 38 | Settings and panels | `windows/src/settings/parts.ts` | 47% | 7 |
| 39 | Settings and panels | `windows/src/settings/phone.ts` | 100% | 7 |
| 40 | Settings and panels | `windows/src/settings/settings.css` | 47% | 7 |
| 41 | Settings and panels | `windows/src/settings/voice.ts` | 54% | 7 |
| 42 | voice and strings | `windows/src/i18n/extra.json` | 21% | 7 |
| 43 | voice and strings | `windows/src/i18n/i18n.ts` | 84% | 7 |
| 44 | voice and strings | `windows/src/i18n/strings.json` | 99% | 7 |
| 45 | voice and strings | `windows/src/voice/controller.ts` | 86% | 7 |
| 46 | voice and strings | `windows/src/voice/livekit.ts` | 100% | 7 |
| 47 | voice and strings | `windows/src/voice/mic.ts` | 52% | 7 |
| 48 | voice and strings | `windows/src/voice/speak.ts` | 59% | 7 |
| 49 | voice and strings | `windows/src/voice/vad.ts` | 100% | 7 |
| 50 | voice and strings | `windows/src/voice/wake.ts` | 92% | 7 |
| 51 | plumbing | `windows/hook/Cargo.toml` | 100% | 8 (not this round) |
| 52 | plumbing | `windows/hook/src/main.rs` | 100% | 8 (not this round) |
| 53 | plumbing | `windows/hook/src/normalize.rs` | 100% | 8 (not this round) |
| 54 | plumbing | `windows/hook/src/reply.rs` | 100% | 8 (not this round) |
| 55 | plumbing | `windows/hook/src/statusline.rs` | 100% | 8 (not this round) |
| 56 | plumbing | `windows/hook/src/unix.rs` | 100% | 8 (not this round) |
| 57 | plumbing | `windows/hook/src/win.rs` | 100% | 8 (not this round) |
| 58 | plumbing | `windows/src-tauri/src/agent_hooks.rs` | 100% | 8 (not this round) |
| 59 | plumbing | `windows/src-tauri/src/agents.rs` | 97% | 8 (not this round) |
| 60 | plumbing | `windows/src-tauri/src/chat.rs` | 50% | 8 (not this round) |
| 61 | plumbing | `windows/src-tauri/src/claude.rs` | 93% | 8 (not this round) |
| 62 | plumbing | `windows/src-tauri/src/codex_plan.rs` | 100% | 8 (not this round) |
| 63 | plumbing | `windows/src-tauri/src/config_file.rs` | 100% | 8 (not this round) |
| 64 | plumbing | `windows/src-tauri/src/desktop.rs` | 93% | 8 (not this round) |
| 65 | plumbing | `windows/src-tauri/src/files.rs` | 100% | 8 (not this round) |
| 66 | plumbing | `windows/src-tauri/src/github.rs` | 100% | 8 (not this round) |
| 67 | plumbing | `windows/src-tauri/src/hooks.rs` | 100% | 8 (not this round) |
| 68 | plumbing | `windows/src-tauri/src/i18n.rs` | 100% | 8 (not this round) |
| 69 | plumbing | `windows/src-tauri/src/identity.rs` | 100% | 8 (not this round) |
| 70 | plumbing | `windows/src-tauri/src/integrations.rs` | 87% | 8 (not this round) |
| 71 | plumbing | `windows/src-tauri/src/island.rs` | 74% | 8 (not this round) |
| 72 | plumbing | `windows/src-tauri/src/lib.rs` | 41% | 8 (not this round) |
| 73 | plumbing | `windows/src-tauri/src/livekit.rs` | 100% | 8 (not this round) |
| 74 | plumbing | `windows/src-tauri/src/local_chat.rs` | 73% | 8 (not this round) |
| 75 | plumbing | `windows/src-tauri/src/log.rs` | 33% | 8 (not this round) |
| 76 | plumbing | `windows/src-tauri/src/net.rs` | 81% | 8 (not this round) |
| 77 | plumbing | `windows/src-tauri/src/ollama.rs` | 59% | 8 (not this round) |
| 78 | plumbing | `windows/src-tauri/src/openai_compat.rs` | 99% | 8 (not this round) |
| 79 | plumbing | `windows/src-tauri/src/pipe.rs` | 67% | 8 (not this round) |
| 80 | plumbing | `windows/src-tauri/src/platform/linux.rs` | 98% | 8 (not this round) |
| 81 | plumbing | `windows/src-tauri/src/platform/linux_focus.rs` | 100% | 8 (not this round) |
| 82 | plumbing | `windows/src-tauri/src/platform/mod.rs` | 100% | 8 (not this round) |
| 83 | plumbing | `windows/src-tauri/src/platform/windows.rs` | 72% | 8 (not this round) |
| 84 | plumbing | `windows/src-tauri/src/portal.rs` | 99% | 8 (not this round) |
| 85 | plumbing | `windows/src-tauri/src/recap.rs` | 99% | 8 (not this round) |
| 86 | plumbing | `windows/src-tauri/src/secrets.rs` | 95% | 8 (not this round) |
| 87 | plumbing | `windows/src-tauri/src/session_window.rs` | 100% | 8 (not this round) |
| 88 | plumbing | `windows/src-tauri/src/settings.rs` | 64% | 8 (not this round) |
| 89 | plumbing | `windows/src-tauri/src/shortcuts.rs` | 98% | 8 (not this round) |
| 90 | plumbing | `windows/src-tauri/src/sounds.rs` | 100% | 8 (not this round) |
| 91 | plumbing | `windows/src-tauri/src/spotify.rs` | 69% | 8 (not this round) |
| 92 | plumbing | `windows/src-tauri/src/tray.rs` | 49% | 8 (not this round) |
| 93 | plumbing | `windows/src-tauri/src/voice.rs` | 24% | 8 (not this round) |
| 94 | plumbing | `windows/src-tauri/src/webview_drop.rs` | 100% | 8 (not this round) |
| 95 | plumbing | `windows/src/core/bridge.ts` | 52% | 8 (not this round) |
| 96 | plumbing | `windows/src/core/diff.ts` | 100% | 8 (not this round) |
| 97 | plumbing | `windows/src/core/fonts.ts` | 100% | 8 (not this round) |
| 98 | plumbing | `windows/src/core/github.ts` | 100% | 8 (not this round) |
| 99 | plumbing | `windows/src/core/phone-chat.ts` | 100% | 8 (not this round) |
| 100 | plumbing | `windows/src/core/pill-colors.ts` | 100% | 8 (not this round) |
| 101 | plumbing | `windows/src/core/plan.ts` | 100% | 8 (not this round) |
| 102 | plumbing | `windows/src/core/providers.ts` | 94% | 8 (not this round) |
| 103 | plumbing | `windows/src/core/shortcuts.ts` | 96% | 8 (not this round) |
| 104 | plumbing | `windows/src/core/sound.ts` | 66% | 8 (not this round) |
| 105 | plumbing | `windows/src/core/spotify.ts` | 100% | 8 (not this round) |
| 106 | plumbing | `windows/src/core/state.ts` | 76% | 8 (not this round) |
| 107 | plumbing | `windows/src/main.ts` | 32% | 8 (not this round) |
| 108 | plumbing | `windows/src/upload/canvas.ts` | 100% | 8 (not this round) |
| 109 | plumbing | `windows/src/upload/sequence.ts` | 100% | 8 (not this round) |
| 110 | phone link | `windows/src-tauri/src/phone_link/admission.rs` | 100% | after the Android source |
| 111 | phone link | `windows/src-tauri/src/phone_link/chat.rs` | 100% | after the Android source |
| 112 | phone link | `windows/src-tauri/src/phone_link/chat_backend.rs` | 100% | after the Android source |
| 113 | phone link | `windows/src-tauri/src/phone_link/discovery.rs` | 100% | after the Android source |
| 114 | phone link | `windows/src-tauri/src/phone_link/hub.rs` | 98% | after the Android source |
| 115 | phone link | `windows/src-tauri/src/phone_link/interop.rs` | 100% | after the Android source |
| 116 | phone link | `windows/src-tauri/src/phone_link/mod.rs` | 92% | after the Android source |
| 117 | phone link | `windows/src-tauri/src/phone_link/pairing.rs` | 100% | after the Android source |
| 118 | phone link | `windows/src-tauri/src/phone_link/relay-test-vectors.json` | 100% | after the Android source |
| 119 | phone link | `windows/src-tauri/src/phone_link/relay_client.rs` | 100% | after the Android source |
| 120 | phone link | `windows/src-tauri/src/phone_link/relay_crypto.rs` | 100% | after the Android source |
| 121 | phone link | `windows/src-tauri/src/phone_link/server.rs` | 100% | after the Android source |
| 122 | phone link | `windows/src-tauri/src/phone_link/tests.rs` | 99% | after the Android source |

## Every file

### `windows/hook/`

- `Cargo.toml`: **inherited** (100% same as at the start)

### `windows/hook/src/`

- `main.rs`: **inherited** (100% same as at the start)
- `normalize.rs`: **inherited** (100% same as at the start)
- `reply.rs`: **inherited** (100% same as at the start)
- `statusline.rs`: **inherited** (100% same as at the start)
- `unix.rs`: **inherited** (100% same as at the start)
- `win.rs`: **inherited** (100% same as at the start)

### `windows/src-tauri/src/`

- `agent_hooks.rs`: **inherited** (100% same as at the start)
- `agents.rs`: **inherited** (97% same as at the start)
- `assistant.rs`: **Groovy's own** (added after the start)
- `brain_ai.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/brains/`

- `cli.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `browser.rs`: **Groovy's own** (added after the start)
- `calls.rs`: **Groovy's own** (added after the start)
- `chat.rs`: **inherited** (50% same as at the start)
- `claude.rs`: **inherited** (93% same as at the start)
- `clipboard.rs`: **Groovy's own** (added after the start)
- `codex_plan.rs`: **inherited** (100% same as at the start)
- `config_file.rs`: **inherited** (100% same as at the start)
- `db.rs`: **Groovy's own** (added after the start)
- `desktop.rs`: **inherited** (93% same as at the start)
- `files.rs`: **inherited** (100% same as at the start)

### `windows/src-tauri/src/flow/`

- `accuracy.rs`: **Groovy's own** (added after the start)
- `cleanup.rs`: **Groovy's own** (added after the start)
- `live.rs`: **Groovy's own** (added after the start)
- `logic.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `os_linux.rs`: **Groovy's own** (added after the start)
- `os_windows.rs`: **Groovy's own** (added after the start)
- `paste.rs`: **Groovy's own** (added after the start)
- `store.rs`: **Groovy's own** (added after the start)
- `text.rs`: **Groovy's own** (added after the start)
- `vocab.rs`: **Groovy's own** (added after the start)
- `wayland.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `gamemode.rs`: **Groovy's own** (added after the start)
- `github.rs`: **inherited** (100% same as at the start)
- `glass.rs`: **Groovy's own** (added after the start)
- `growth_clock.rs`: **Groovy's own** (added after the start)
- `hooks.rs`: **inherited** (100% same as at the start)
- `hub_idle.rs`: **Groovy's own** (added after the start)
- `i18n.rs`: **inherited** (100% same as at the start)
- `identity.rs`: **inherited** (100% same as at the start)
- `integrations.rs`: **inherited** (87% same as at the start)
- `island.rs`: **inherited** (74% same as at the start)
- `island_sleep.rs`: **Groovy's own** (added after the start)
- `island_tiles.rs`: **Groovy's own** (added after the start)
- `lib.rs`: **inherited** (41% same as at the start)
- `livekit.rs`: **inherited** (100% same as at the start)
- `local_chat.rs`: **inherited** (73% same as at the start)
- `log.rs`: **inherited** (33% same as at the start)

### `windows/src-tauri/src/mail/`

- `api.rs`: **Groovy's own** (added after the start)
- `body.rs`: **Groovy's own** (added after the start)
- `imap.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `oauth.rs`: **Groovy's own** (added after the start)
- `smtp.rs`: **Groovy's own** (added after the start)
- `text.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `main.rs`: **rewritten** (0% same as at the start)
- `memnotes.rs`: **Groovy's own** (added after the start)
- `memory.rs`: **Groovy's own** (added after the start)
- `models.rs`: **Groovy's own** (added after the start)
- `music.rs`: **Groovy's own** (added after the start)
- `net.rs`: **inherited** (81% same as at the start)

### `windows/src-tauri/src/notes/`

- `builder.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/okgroovy/`

- `basic.rs`: **Groovy's own** (added after the start)
- `brain.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `wake.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `ollama.rs`: **inherited** (59% same as at the start)
- `openai_compat.rs`: **inherited** (99% same as at the start)
- `pc.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/phone_link/`

- `admission.rs`: **inherited** (100% same as at the start)
- `chat.rs`: **inherited** (100% same as at the start)
- `chat_backend.rs`: **inherited** (100% same as at the start)
- `discovery.rs`: **inherited** (100% same as at the start)
- `hub.rs`: **inherited** (98% same as at the start)
- `interop.rs`: **inherited** (100% same as at the start)
- `mod.rs`: **inherited** (92% same as at the start)
- `pairing.rs`: **inherited** (100% same as at the start)
- `relay-test-vectors.json`: **inherited** (100% same as at the start)
- `relay_client.rs`: **inherited** (100% same as at the start)
- `relay_crypto.rs`: **inherited** (100% same as at the start)
- `server.rs`: **inherited** (100% same as at the start)
- `tests.rs`: **inherited** (99% same as at the start)

### `windows/src-tauri/src/`

- `pipe.rs`: **inherited** (67% same as at the start)

### `windows/src-tauri/src/platform/`

- `linux.rs`: **inherited** (98% same as at the start)
- `linux_focus.rs`: **inherited** (100% same as at the start)
- `mod.rs`: **inherited** (100% same as at the start)
- `windows.rs`: **inherited** (72% same as at the start)

### `windows/src-tauri/src/`

- `portal.rs`: **inherited** (99% same as at the start)
- `recap.rs`: **inherited** (99% same as at the start)
- `redact.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/resources/`

- `ceiling.rs`: **Groovy's own** (added after the start)
- `idle.rs`: **Groovy's own** (added after the start)
- `loads.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `state.rs`: **Groovy's own** (added after the start)
- `webview.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `scheduler.rs`: **Groovy's own** (added after the start)
- `secrets.rs`: **inherited** (95% same as at the start)
- `session_window.rs`: **inherited** (100% same as at the start)
- `settings.rs`: **inherited** (64% same as at the start)
- `shortcuts.rs`: **inherited** (98% same as at the start)

### `windows/src-tauri/src/skills/`

- `analyze.rs`: **Groovy's own** (added after the start)
- `cli.rs`: **Groovy's own** (added after the start)
- `manage.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `recommend.rs`: **Groovy's own** (added after the start)
- `store.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `sounds.rs`: **inherited** (100% same as at the start)

### `windows/src-tauri/src/speech/`

- `audio.rs`: **Groovy's own** (added after the start)
- `check.rs`: **Groovy's own** (added after the start)
- `cloud.rs`: **Groovy's own** (added after the start)
- `helper.rs`: **Groovy's own** (added after the start)
- `kokoro_files.rs`: **Groovy's own** (added after the start)
- `kws.rs`: **Groovy's own** (added after the start)
- `mapped.rs`: **Groovy's own** (added after the start)
- `mod.rs`: **Groovy's own** (added after the start)
- `tts.rs`: **Groovy's own** (added after the start)
- `whisper_files.rs`: **Groovy's own** (added after the start)

### `windows/src-tauri/src/`

- `spotify.rs`: **inherited** (69% same as at the start)
- `studio.rs`: **Groovy's own** (added after the start)
- `system.rs`: **Groovy's own** (added after the start)
- `tasks.rs`: **Groovy's own** (added after the start)
- `tile_icons.rs`: **Groovy's own** (added after the start)
- `tools.rs`: **Groovy's own** (added after the start)
- `tray.rs`: **inherited** (49% same as at the start)
- `voice.rs`: **inherited** (24% same as at the start)
- `webview_drop.rs`: **inherited** (100% same as at the start)
- `worktest.rs`: **Groovy's own** (added after the start)

### `windows/src/core/`

- `anim.ts`: **rewritten** (bodies replaced in round 4E item 1, 20 Nov 2026; 41% of lines still match: the public signatures, event names and tested constants its callers use)
- `autoclose.ts`: **Groovy's own** (added after the start)
- `bridge.ts`: **inherited** (52% same as at the start)
- `diff.ts`: **inherited** (100% same as at the start)
- `flow.ts`: **Groovy's own** (added after the start)
- `fonts.ts`: **inherited** (100% same as at the start)
- `frametest.ts`: **Groovy's own** (added after the start)
- `github.ts`: **inherited** (100% same as at the start)
- `glass.ts`: **Groovy's own** (added after the start)
- `home.ts`: **Groovy's own** (added after the start)
- `layout.ts`: **inherited** (89% same as at the start)
- `lazy.ts`: **Groovy's own** (added after the start)
- `lookchoice.ts`: **Groovy's own** (added after the start)
- `motion.ts`: **Groovy's own** (added after the start)
- `phone-chat.ts`: **inherited** (100% same as at the start)
- `pill-colors.ts`: **inherited** (100% same as at the start)
- `pills.ts`: **inherited** (88% same as at the start)
- `plan.ts`: **inherited** (100% same as at the start)
- `providers.ts`: **inherited** (94% same as at the start)
- `shortcuts.ts`: **inherited** (96% same as at the start)
- `skills.ts`: **Groovy's own** (added after the start)
- `sound.ts`: **inherited** (66% same as at the start)
- `sounds-synth.ts`: **Groovy's own** (added after the start)
- `spotify.ts`: **inherited** (100% same as at the start)
- `state.ts`: **inherited** (76% same as at the start)
- `tiles.ts`: **Groovy's own** (added after the start)

### `windows/src/desktop/`

- `main.ts`: **inherited** (93% same as at the start)

### `windows/src/hub/`

- `busy.ts`: **Groovy's own** (added after the start)

### `windows/src/hub/fonts/`

- `OFL.txt`: **Groovy's own** (added after the start)
- `manrope-cyrillic-ext-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope-cyrillic-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope-greek-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope-latin-ext-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope-latin-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope-vietnamese-wght-normal.woff2`: **Groovy's own** (added after the start)
- `manrope.css`: **Groovy's own** (added after the start)

### `windows/src/hub/`

- `hero.ts`: **Groovy's own** (added after the start)
- `hub.css`: **Groovy's own** (added after the start)
- `nav.ts`: **Groovy's own** (added after the start)
- `pages.ts`: **Groovy's own** (added after the start)

### `windows/src/hub/pages/`

- `dictation.ts`: **Groovy's own** (added after the start)
- `flowsettings.ts`: **Groovy's own** (added after the start)
- `home.ts`: **Groovy's own** (added after the start)
- `insights.ts`: **Groovy's own** (added after the start)
- `notes.ts`: **Groovy's own** (added after the start)
- `skills.ts`: **Groovy's own** (added after the start)
- `speechsetup.ts`: **Groovy's own** (added after the start)
- `styles.ts`: **Groovy's own** (added after the start)
- `voice.ts`: **Groovy's own** (added after the start)
- `words.ts`: **Groovy's own** (added after the start)

### `windows/src/hub/`

- `shell.ts`: **Groovy's own** (added after the start)
- `template.ts`: **Groovy's own** (added after the start)
- `ui.ts`: **Groovy's own** (added after the start)

### `windows/src/i18n/`

- `extra.json`: **inherited** (21% same as at the start)
- `i18n.ts`: **inherited** (84% same as at the start)
- `langs.ts`: **Groovy's own** (added after the start)
- `strings.json`: **inherited** (99% same as at the start)

### `windows/src/island/`

- `agents.ts`: **inherited** (100% same as at the start)
- `brain.ts`: **inherited** (100% same as at the start)
- `breaks.ts`: **inherited** (87% same as at the start)
- `desktop.ts`: **inherited** (91% same as at the start)
- `fsm.ts`: **inherited** (73% same as at the start)
- `hooks.ts`: **inherited** (100% same as at the start)
- `integrations.ts`: **inherited** (100% same as at the start)
- `island.ts`: **inherited** (69% same as at the start)
- `live.ts`: **Groovy's own** (added after the start)
- `okgroovy.ts`: **Groovy's own** (added after the start)
- `phone-link.ts`: **inherited** (98% same as at the start)
- `phone-services.ts`: **inherited** (100% same as at the start)
- `shortcuts.ts`: **inherited** (96% same as at the start)
- `sleep.ts`: **Groovy's own** (added after the start)
- `spotify.ts`: **inherited** (100% same as at the start)
- `voice.ts`: **inherited** (47% same as at the start)

### `windows/src/`

- `main.ts`: **inherited** (32% same as at the start)

### `windows/src/mochi/`

- `appearance.ts`: **Groovy's own** (added after the start)

### `windows/src/mochi/art/`

- `draw.ts`: **Groovy's own** (added after the start)
- `growth.ts`: **Groovy's own** (added after the start)
- `seasonal.ts`: **Groovy's own** (added after the start)
- `stage6.ts`: **Groovy's own** (added after the start)
- `wardrobe.ts`: **Groovy's own** (added after the start)

### `windows/src/mochi/`

- `desktop-logic.ts`: **rewritten** (bodies replaced in round 4E item 1, 20 Nov 2026; 70% of lines still match: the public signatures, event names and tested constants its callers use)
- `engine.ts`: **rewritten** (round 4E item 1, 20 Nov 2026; its kept blocks are Groovy's own)
- `finish.ts`: **Groovy's own** (added after the start)
- `growth.ts`: **Groovy's own** (added after the start)
- `minibots.ts`: **rewritten** (bodies replaced in round 4E item 1, 20 Nov 2026; 44% of lines still match: the public signatures, event names and tested constants its callers use)
- `motion.ts`: **Groovy's own** (added after the start)
- `naming.ts`: **Groovy's own** (added after the start)
- `outfits.ts`: **Groovy's own** (no line of the original, CHARACTER_MOTION.md §1)
- `page.css`: **Groovy's own** (added after the start)
- `pillow.ts`: **Groovy's own** (added after the start)
- `seasons.ts`: **Groovy's own** (added after the start)
- `seed.ts`: **Groovy's own** (added after the start)
- `skins.ts`: **Groovy's own** (no line of the original, CHARACTER_MOTION.md §1)
- `sprites.ts`: **Groovy's own** (added after the start)
- `stage6.ts`: **Groovy's own** (added after the start)
- `wardrobe.ts`: **Groovy's own** (no line of the original, CHARACTER_MOTION.md §1)

### `windows/src/recap/`

- `recap.ts`: **inherited** (100% same as at the start)
- `share.ts`: **inherited** (100% same as at the start)
- `summary.ts`: **inherited** (100% same as at the start)

### `windows/src/settings/`

- `about.ts`: **Groovy's own** (added after the start)
- `agents.ts`: **inherited** (100% same as at the start)
- `aisetup.ts`: **Groovy's own** (added after the start)
- `assistant.ts`: **Groovy's own** (added after the start)
- `brains.ts`: **Groovy's own** (added after the start)
- `breaks.ts`: **inherited** (100% same as at the start)
- `colors.ts`: **inherited** (100% same as at the start)
- `first-frame.css`: **Groovy's own** (added after the start)
- `island.ts`: **Groovy's own** (added after the start)
- `islandpage.ts`: **Groovy's own** (added after the start)
- `lightlocal.ts`: **Groovy's own** (added after the start)
- `mail.ts`: **Groovy's own** (added after the start)
- `main.ts`: **inherited** (78% same as at the start)
- `memory.ts`: **Groovy's own** (added after the start)
- `models.ts`: **Groovy's own** (added after the start)
- `music.ts`: **Groovy's own** (added after the start)
- `ollama.ts`: **Groovy's own** (added after the start)
- `parts.ts`: **inherited** (47% same as at the start)
- `permissions.ts`: **Groovy's own** (added after the start)
- `phone.ts`: **inherited** (100% same as at the start)
- `preview.ts`: **Groovy's own** (added after the start)
- `settings.css`: **inherited** (47% same as at the start)
- `speechlabels.ts`: **Groovy's own** (added after the start)
- `system.ts`: **Groovy's own** (added after the start)
- `tasks.ts`: **Groovy's own** (added after the start)
- `tilesgrid.ts`: **Groovy's own** (added after the start)
- `voice.ts`: **inherited** (54% same as at the start)
- `voicetest.ts`: **Groovy's own** (added after the start)
- `wardrobe.ts`: **Groovy's own** (added after the start)

### `windows/src/studio/`

- `ai.ts`: **Groovy's own** (added after the start)
- `first-frame.css`: **Groovy's own** (added after the start)
- `main.ts`: **Groovy's own** (added after the start)
- `studio.css`: **Groovy's own** (added after the start)

### `windows/src/`

- `style.css`: **inherited** (69% same as at the start)

### `windows/src/upload/`

- `canvas.ts`: **inherited** (100% same as at the start)
- `sequence.ts`: **inherited** (100% same as at the start)

### `windows/src/views/`

- `brands.ts`: **Groovy's own** (added after the start)
- `chat.ts`: **inherited** (71% same as at the start)
- `diff.ts`: **inherited** (100% same as at the start)
- `dom.ts`: **inherited** (99% same as at the start)
- `expand.ts`: **Groovy's own** (added after the start)
- `github.ts`: **inherited** (100% same as at the start)
- `hatch.ts`: **Groovy's own** (added after the start)
- `home.ts`: **Groovy's own** (added after the start)
- `icons.ts`: **inherited** (93% same as at the start)
- `integrations.ts`: **inherited** (91% same as at the start)
- `livebar.ts`: **Groovy's own** (added after the start)
- `markdown.ts`: **inherited** (100% same as at the start)
- `panel.ts`: **Groovy's own** (added after the start)
- `recap.ts`: **inherited** (100% same as at the start)
- `spotify.ts`: **inherited** (94% same as at the start)
- `ticker.ts`: **inherited** (100% same as at the start)
- `tileicon.ts`: **Groovy's own** (added after the start)
- `tiles.ts`: **Groovy's own** (added after the start)
- `toast.css`: **Groovy's own** (added after the start)
- `toast.ts`: **Groovy's own** (added after the start)
- `upload.ts`: **inherited** (100% same as at the start)
- `usage.ts`: **inherited** (100% same as at the start)
- `views.ts`: **inherited** (76% same as at the start)
- `wardrobe.ts`: **rewritten** (19% same as at the start)

### `windows/src/voice/`

- `call-state.ts`: **Groovy's own** (added after the start)
- `call.ts`: **rewritten** (17% same as at the start)
- `controller.ts`: **inherited** (86% same as at the start)
- `instant.ts`: **Groovy's own** (added after the start)
- `livekit.ts`: **inherited** (100% same as at the start)
- `mic.ts`: **inherited** (52% same as at the start)
- `sentences.ts`: **Groovy's own** (added after the start)
- `speak.ts`: **inherited** (59% same as at the start)
- `stages.ts`: **Groovy's own** (added after the start)
- `vad.ts`: **inherited** (100% same as at the start)
- `wake.ts`: **inherited** (92% same as at the start)
