# Fixes round 4B: voice pipeline (10 Nov)

Read `docs/plan/ARCHITECTURE_MAP.md` section 2.4 first. Same rules as round 4A (one item per commit, tick the queue, no feature removed, Linux kept, 150 / 300 MB, ten languages, `command-access.mjs`).

**Today:** two listeners. The page (`src/voice/controller.ts`, mic in the WebView) wakes to "Groovy / Hey Groovy / Yo Groovy" and opens chat or a call. Rust (`okgroovy/mod.rs`, mic through `cpal`, spotter in `speech/kws.rs`) wakes to "OK / Okay / Hey Groovy" and runs commands. Speech-to-text is cloud first whenever a key exists.

**Target:** one listener in Rust, the three wake words "Groovy", "Hey Groovy", "Yo Groovy", speech-to-text on this PC by default, LiveKit as an option, under 1 % CPU while listening.

## Item 1: One wake-word listener, in Rust (Opus recommended)
**Goal:** only `groovy.exe` holds the microphone while waiting for the wake word. The page opens its own microphone only during a call (it needs the browser's echo cancellation).
**Files:** `src-tauri/src/okgroovy/{mod,wake}.rs`, `speech/kws.rs`, `calls.rs`, `src/voice/controller.ts`, `src/island/voice.ts`, `src/island/okgroovy.ts`, `src/voice/wake.ts`, `tests/voice*.test.mjs`.
**Steps:**
1. In `kws.rs` `KEYWORDS`, add `▁YO ▁G RO O V Y @YO_GROOVY` and a bare `▁G RO O V Y @GROOVY`. Keep "OK Groovy" and "Okay Groovy" as they are (no feature removed) and keep his given name. Check each piece exists in the model's `tokens.txt` the way `pieces()` does.
2. A bare "Groovy" false-fires easily. Give it its own stricter score and threshold (sherpa-onnx keyword lines take `:score` and `#threshold`), and accept it only when it is the **first word** of the stretch of speech (the rule in `src/voice/wake.ts`). Port that rule into `okgroovy/wake.rs` `after_wake` with tests; `is_ok` already accepts "hey" and "yo".
3. After a wake, Rust decides where the words go, with one setting `wakeAction`: `command` (today's « OK Groovy » tools), `chat` (ask as a typed question), `call` (start a voice call), or `livekit`. Default: `command`, falling to `chat` when the reply needs no tool. Emit one event to the island, `wake {phrase, text, action}`.
4. The page's `VoiceController` stops opening the microphone for the wake phase: it keeps its `command` and call phases and is driven by the `wake` event. Delete the page's per-segment Whisper wake check only after the Rust path passes its tests.
5. While a call has the microphone, Rust's listener pauses (`calls.rs` `ACTIVE` already does this); it resumes at hang-up.
**Test:** Rust tests with `tests/audio/ok-groovy-time.wav` and `not-wake.wav`, plus new synthetic clips for "Groovy, …", "Hey Groovy", "Yo Groovy" and three sentences that only mention "groovy" (must not wake). Generate clips with Kokoro in the test, don't commit recordings of people. Page tests updated.
**Done when:** with the wake word on and no call, the island WebView has no `AudioContext` and no `getUserMedia` stream, and all three phrases wake him in the Rust tests with none of the decoys.

## Item 2: Speech-to-text on this PC by default
**Goal:** audio leaves the PC only if the user chose a cloud service.
**Files:** `src-tauri/src/speech/{mod,cloud,helper}.rs`, `voice.rs`, `flow/mod.rs`, `settings.rs`, `src/hub/pages/voice.ts`, `src/hub/pages/dictation.ts`.
**Steps:**
1. Setting `speechMode`: `local` (default), `cloud-first` (today's behaviour: cloud when a key exists, local fallback). Migration: an install that has a speech key **and** has used cloud dictation keeps `cloud-first`; everyone else gets `local`. Show a one-time notice either way.
2. `local`: Whisper base through the `stt` helper (Parakeet stays opt-in). No cloud call is made, including for the wake check and for calls.
3. The first use offers the model download (57 MB) in the island; until it is there, say so. Never silent.
4. Settings → Voice shows which engine heard the last sentence ("On this PC: Whisper base" or the service's name).
**Test:** a Rust test that in `local` mode no request is built for any `cloud::Service` even with keys present; settings-keys test.
**Done when:** a new install with a Deepgram key in the keychain transcribes with Whisper until the user switches to cloud-first.

## Item 3: Low CPU while listening
**Goal:** under 1 % of the machine over 60 s of quiet listening, under 3 % while someone talks without saying the wake word.
**Files:** `src-tauri/src/okgroovy/{mod,wake}.rs`, `speech/{audio,kws}.rs`, `scripts/smoke-test.ps1`.
**Steps:**
1. Measure today's numbers with the new "listening idle" CI step (round 4A item 4) before changing anything.
2. Capture at 16 kHz mono if the device allows it; otherwise resample in 30 ms frames, not per sample. The stream callback does only the energy VAD (`wake::Vad`); the spotter runs on its own thread and only on stretches the VAD passed.
3. The spotter is freed after the idle setting with no speech and reloaded on the next stretch (it is a few MB; reload must be under 100 ms, else keep it loaded).
4. Game Mode and "while he speaks" keep pausing the listener as today.
**Test:** the CI step's CPU numbers.
**Done when:** the CI step reads under 1 % quiet and under 3 % with `not-wake.wav` looped, written in `STATUS.md`.

## Item 4: LiveKit as a wake action
**Goal:** LiveKit keeps working exactly as upstream; choosing it is one switch.
**Files:** `src/voice/livekit.ts`, `src/island/voice.ts`, `src-tauri/src/livekit.rs`, `src/settings/voice.ts`.
**Steps:**
1. `wakeAction: livekit` joins the user's room on a wake (item 1's event), as the page does today. `livekit-client` stays a lazy import; nothing of it loads in any other mode.
2. Hang-up releases the room first (the 6 Nov fix): add a page test for it.
3. Settings → Voice: URL, key and secret (keychain), a "Test connection" button that mints a token and joins for 3 s.
4. The CSP in `tauri.conf.json` allows only `*.livekit.cloud`, and it is fixed at build time. Don't widen it to all `wss:`. Say in Settings that a self-hosted LiveKit server needs a custom build, and reject a URL outside `*.livekit.cloud` with that message.
**Test:** page tests with a mocked `livekit-client`; `scripts/csp-check.mjs`.
**Done when:** a wake with `livekit` chosen joins the room, "okay bye" leaves it, and no LiveKit code is fetched in the other modes (check the network log in the frame test).

## Item 5: Better end of speech (finishes 3J item 2)
**Goal:** he stops listening when the user has finished, not on a mid-sentence pause.
**Files:** `src-tauri/src/okgroovy/wake.rs` (`Vad`), `src/voice/vad.ts`.
**Steps:**
1. Benchmark the current energy VAD against Silero VAD (sherpa-onnx ships it, about 2 MB) on 20 synthetic utterances with pauses: count early cut-offs and late endings.
2. Adopt Silero only if it has fewer early cut-offs and costs under 5 MB and under 0.5 % CPU. Otherwise keep the current code and write the numbers down.
**Done when:** the benchmark table is in `STATUS.md` and 3J item 2 is closed either way.

## Item 6: Checks
- All three wake phrases, "OK Groovy" and his name on Windows CI. Peak and idle unchanged or lower. One `[build]`. "Ready for the owner" in `STATUS.md`: what to say, and where the wake action is set.
