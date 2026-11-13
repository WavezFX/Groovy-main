# Fixes round 4B: voice pipeline (10 Nov, revised to `docs/GROOVY_SPEC.md`)

Read `docs/GROOVY_SPEC.md` (sections 4, 8, 10) and `docs/plan/ARCHITECTURE_MAP.md` section 2.4 first. One commit for the round with `[build]`.

**Today:** two listeners. The page (`src/voice/controller.ts`, microphone in the WebView) wakes to "Groovy / Hey Groovy / Yo Groovy". Rust (`okgroovy/mod.rs`, microphone through `cpal`, spotter in `speech/kws.rs`) wakes to an "OK / Okay / Hey" set and to his given name. Speech-to-text and his voice go to cloud services first whenever a key exists.

**Target:** one listener in Rust; only "Groovy", "Hey Groovy", "Yo Groovy"; speech-to-text and voice on this PC by default; LiveKit as an option.

## Item 1: One listener, three wake words (Opus)
**Goal:** only `groovy.exe` holds the microphone while waiting. The page opens a microphone only during a call (it needs the browser's echo cancellation).
**Files:** `src-tauri/src/okgroovy/{mod,wake}.rs`, `speech/kws.rs`, `calls.rs`, `settings.rs`, `src/voice/{controller,wake}.ts`, `src/island/{voice,okgroovy}.ts`, `src/hub/pages/voice.ts`, `src/i18n/*`, `tests/voice*.test.mjs`.
**Steps:**
1. `kws.rs` `KEYWORDS` becomes exactly three lines: `▁G RO O V Y`, `▁HE Y ▁G RO O V Y`, `▁YO ▁G RO O V Y`. Check each piece is in the model's token list the way `pieces()` does. **Remove** the "OK" and "Okay" lines.
2. **Remove** waking to his given name (`set_name`, `wakeToName`, its Settings switch). His name stays everywhere else. *The spec marks this "owner to confirm": if the owner has said to keep it, skip this step only.*
3. A bare "Groovy" gets its own stricter score and threshold (keyword lines take `:score` and `#threshold`) and counts only as the **first word** of a stretch of speech. Move that rule from `src/voice/wake.ts` into `okgroovy/wake.rs` `after_wake`; drop "ok", "okay", "oke", "okey" from `is_ok`.
4. One setting `wakeAction` decides where the words go: `command` (tools), `chat`, `call`, `livekit`. Default `command`. Rust emits one event, `wake {phrase, text, action}`.
5. The page's `VoiceController` no longer opens the microphone or checks for a wake phrase; it keeps the command and call phases, driven by the event. Delete the page's wake check and `voice/wake.ts`'s matcher once the Rust tests pass.
6. Every visible « OK Groovy » becomes "Hey Groovy" (ten languages). Settings keys keep their names (no migration); an install that had the old wake word on has the new one on, with a one-time notice naming the three phrases.
7. While a call has the microphone, the listener pauses (`calls.rs` `ACTIVE`); it resumes at hang-up.
**Test:** Rust tests on clips made in the test with the offline voice (no recordings of people): 10 wake phrases across the three, and 20 decoys including "OK Groovy", "okay groovy", "that's really groovy" and "…feeling groovy today".
**Done when:** **at least 9 of 10 wake phrases are heard and 0 of 20 decoys (P5)**; with the wake word on and no call, the island WebView has **no** `AudioContext` and **no** microphone stream; `grep -rn "OKAY_GROOVY\|OK_GROOVY" windows/src-tauri/src` finds nothing.

## Item 2: Speech-to-text on this PC by default
**Files:** `src-tauri/src/speech/{mod,cloud,helper}.rs`, `voice.rs`, `flow/mod.rs`, `settings.rs`, `src/hub/pages/{voice,dictation}.ts`.
**Steps:**
1. Setting `speechMode`: `local` (default) or `cloud-first` (cloud when a key exists, local fallback). An install that has already used a cloud speech service keeps `cloud-first`; everyone else is `local`. One-time notice.
2. `local`: Whisper base in the `stt` helper (Parakeet stays an optional download). No cloud speech request is built, for dictation, commands or calls.
3. First use offers the model download (57 MB) in the island. Never silent.
4. Settings → Voice shows what heard the last sentence.
**Test:** in `local` mode, with keys present, no request is built for any `cloud::Service`.
**Done when:** a new install with a speech key in the keychain still transcribes on the PC, and the `stt` helper stays within the **300 MB** peak (peak test).

## Item 3: His voice on this PC by default
**Needs round 4A item 1** (the light voice).
**Files:** `src-tauri/src/speech/tts.rs`, `settings.rs`, `src/hub/pages/voice.ts`, `src/voice/speak.ts`.
**Steps:**
1. Setting `voiceMode`: `local` (default: the light voice; Kokoro if downloaded and chosen) or `cloud` (Fish Audio or ElevenLabs with the user's key, local fallback). An install with a cloud voice already set up keeps `cloud`; new installs are `local`.
2. In `local` no request goes to a voice service. The fallback order inside `local` is light voice, then Windows' own voice.
**Test:** in `local` mode, with a voice key present, no voice request is built.
**Done when:** a new install speaks with the light voice with no network, and its helper is **at most 80 MB**.

## Item 4: CPU and response times
**Files:** `src-tauri/src/okgroovy/{mod,wake}.rs`, `speech/{audio,kws,helper}.rs`, `calls.rs`, `scripts/smoke-test.ps1`, `scripts/peak-test.ps1`.
**Steps:**
1. Capture at 16 kHz mono when the device allows; else resample per 30 ms frame. The stream callback runs only the energy detector; the spotter runs on its own thread, only on stretches that passed.
2. Log three times in every call and command: end of wake phrase → listening cue; end of speech → transcript; brain's first text → first sound.
3. Pre-warm on the wake: start the `stt` helper when the spotter fires and the `tts` helper when the transcript is sent to the brain. Speak the first clause as soon as it is complete, not the whole reply.
4. Game Mode and "while he speaks" keep pausing the listener.
**Done when:** CI reads **P2 at most 1.0 % CPU quiet, P3 at most 3.0 % with speech, P4 at most 500 ms, P6 at most 1.0 s of local time, P7 first sound within 2.5 s (p50) with a cloud brain** (use the test's stub brain with a fixed 500 ms delay so the number is repeatable); memory stays **under 150 MB** while listening.

## Item 5: LiveKit as an option
**Files:** `src/voice/livekit.ts`, `src/island/voice.ts`, `src-tauri/src/livekit.rs`, `src/settings/voice.ts`.
**Steps:**
1. `wakeAction: livekit` joins the user's room on the wake event. `livekit-client` stays a lazy import.
2. Hang-up and "okay bye" release the room first; add a page test.
3. Settings: URL, key and secret (keychain), and "Test connection" (mint a token, join for 3 s).
4. The CSP allows only `*.livekit.cloud` and is fixed at build time; reject other hosts with a message saying so. Don't widen it.
**Test:** page tests with a mocked `livekit-client`; `scripts/csp-check.mjs`.
**Done when:** a wake joins the room, goodbye leaves it, no LiveKit code is fetched in other modes, and a LiveKit call peaks **under 300 MB**.

## Item 6: Better end of speech
**Files:** `src-tauri/src/okgroovy/wake.rs` (`Vad`), `src/voice/vad.ts`.
**Steps:** benchmark the energy detector against Silero VAD (shipped with sherpa-onnx, MIT, about 2 MB) on 20 synthetic utterances with pauses. Adopt it only if it has fewer early cut-offs and costs under 5 MB and under 0.5 % CPU.
**Done when:** the table is in `STATUS.md`, and P2 and P3 still hold.

## Item 7: Checks
- The six numbers above in the run summary. Idle **under 150 MB**, peak **under 300 MB**. "Ready for the owner": the three phrases, and where the wake action, speech mode and voice mode are set.
