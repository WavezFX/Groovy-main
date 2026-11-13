# Fixes round 4D: brains — one interface, sign-in first, a router slot (10 Nov, revised to `docs/GROOVY_SPEC.md`)

Read `docs/GROOVY_SPEC.md` (sections 6, 8, 10) and `docs/plan/ARCHITECTURE_MAP.md` section 2.5 first. One commit for the round with `[build]`. **Do this round before 4C.**

**Today:** two dispatchers. `chat.rs` `send_to` picks an adapter by provider string, with `assistant.rs` `route` for the local fallback. `brain_ai.rs` has its own `Who` enum, `order`, `arrange`, limit switching and `CloudTally` for background jobs and voice commands. Subscriptions are reached by running the `claude` and `codex` command-line tools for one text answer: no tools, no streaming. Gemini works with a key only. There is no `Brain` trait in Rust.

**Target:** one `Brain` trait every provider implements; account sign-in first, keys optional; Gemini sign-in; tools on subscription brains; one switcher; a router slot. Order stays cloud and subscription first, local last. Nothing is redesigned around local Ollama.

**Memory:** nothing here loads at start. Idle stays **under 150 MB** and peak **under 300 MB**; a subscription tool process counts toward the peak while it runs.

## Item 1: The `Brain` trait and a registry (Opus)
**Files:** new `src-tauri/src/brains/{mod,anthropic,openai_compat,cli,local}.rs`; code moves from `claude.rs`, `openai_compat.rs`, `local_chat.rs`, `brain_ai.rs` (`cli_complete`, `openai_turn`, the format converters).
**Steps:**
1. In `brains/mod.rs`:
   - `struct Caps { tools, vision, streaming, web_search, local, tier }` where `tier` is `Small | Large` (for the router).
   - `enum Auth { Account { signed_in }, ApiKey { present } }`; `fn auth(&self) -> Vec<Auth>` (a brain may offer both).
   - `trait Brain { fn id(&self) -> &'static str; fn name(&self) -> &'static str; fn caps(&self, model: &str) -> Caps; fn auth(&self) -> Vec<Auth>; async fn models(&self) -> Result<Vec<ModelInfo>, String>; async fn turn(&self, req: Request, on_delta: Option<Delta>) -> Result<Reply, BrainError>; }`
   - `Request { model, system, messages, tools, max_tokens, timeout }`, `Reply { content, stop_reason, usage }` in the Messages shape the callers already use. Each adapter converts inside itself.
   - `enum BrainError { Limit, Network, Auth, Refused, Other(String) }`, built from today's `is_limit`, `is_network`, `worth_falling_back`.
2. `fn registry() -> &'static [&'static dyn Brain]`: anthropic, google, openai, openrouter, claude-code, codex, gemini-cli, ollama, lmstudio, llamacpp, custom. Stored ids don't change.
3. Unit-test each adapter's conversions with the current fixtures. No network in tests.
**Done when:** every provider implements `Brain`, `cargo test` is green on Linux and Windows, and no setting key changed (`tests/settings-keys.test.mjs`).

## Item 2: One switcher
**Files:** new `src-tauri/src/brains/switch.rs`; `chat.rs`, `brain_ai.rs`, `assistant.rs`, `phone_link/chat_backend.rs`, `calls.rs`, `okgroovy/mod.rs`.
**Steps:**
1. `async fn run(job: Job, req, on_delta) -> Result<(Reply, &'static str), String>`. `Job { kind: Chat | Writing | Commands | Vision, weight: Light | Heavy }`. It takes the order from the router (item 5), filters by `Caps`, tries each brain, and moves on only on `Limit`, `Network` or `Auth`, and only when "Switch when a limit is hit" allows. `CloudTally` moves here.
2. `chat.rs` (`send`, `complete`, `send_for_phone`) and `brain_ai.rs` (`complete`, `tool_turn`) become thin callers. The brain picked above the chat box still goes first.
3. Delete `Who` and the duplicated matches. The pinned fast-model constant becomes the adapter's default for light jobs, overridable by "Model per job".
**Test:** the `order` tests ported; a `Limit` reaches the next brain; a `Refused` doesn't; local-only mode builds no cloud request.
**Done when:** `grep -n "match who\|provider == ANTHROPIC" windows/src-tauri/src` finds nothing outside `brains/`, and every user-visible message is unchanged.

## Item 3: Sign in first; Gemini sign-in added; keys optional
**Files:** `src-tauri/src/brains/cli.rs`, `agents.rs` (already knows the Gemini tool's paths), `src/settings/brains.ts`, i18n.
**Steps:**
1. Add the Gemini command-line tool as the third subscription brain (`gemini-cli`): detect it, run it for one prompt with no shell, in an empty temp folder, read-only. Read its flags from `--help` in the build; don't guess.
2. `Auth::Account { signed_in }` for Claude, Codex and Gemini: use the tool's own status command, else the presence of its credentials file. Never read, copy, log or send a token.
3. "Sign in" opens the tool's own login in the user's browser. Groovy implements no OAuth of its own for these and never asks for a password. Tool not installed: the row shows the official install command with Copy.
4. Brains page, per brain: **Sign in** first, then "Use an API key instead" (collapsed). Pill: Signed in / Key set / Not set up. Auto order puts a signed-in account before a key for the same brain.
5. Streaming: read each tool's streaming output into `on_delta`.
**Test:** fake tool executables (a script printing fixtures): detect, signed-in, one answer, streaming, timeout, kill.
**Done when:** on a PC with **no API key at all**, chat answers through each of Claude, Codex and Gemini sign-in, and the first text appears **within 2.0 s** of the tool's own first output.

## Item 4: Tools on subscription brains (Opus)
**Files:** `src-tauri/src/brains/cli.rs`, `okgroovy/brain.rs`.
**Steps:**
1. The subscription adapter reports `Caps.tools = true` through a strict protocol: the system prompt lists Groovy's tools and asks for plain text or exactly one line `{"tool": name, "input": {...}}`. Parse strictly; anything else is text.
2. The tool itself runs with its own tools off (one turn, read-only): it only writes the request; Groovy runs it under the same Rust approval rules as a key brain (`held`, `changes_things`, `reads_outside`).
3. Tool results go back as data (`as_data`). Two parse failures in a turn: answer in text and log.
4. Vision for subscription brains: pass the capture as a temp file path only if the tool accepts image input; delete the file when the turn ends. Otherwise `Caps.vision = false`.
**Test:** fixtures for a tool request, a malformed request, and an injection attempt inside a tool result (must not run).
**Done when:** "Hey Groovy, pause the music" works with **only** a sign-in, on each of Claude, Codex and Gemini, and the tool process is gone within 2 s of the answer (peak **under 300 MB**).

## Item 5: The router slot
**Files:** new `src-tauri/src/brains/router.rs`, `src/settings/brains.ts` (the disabled "Router plugin" row), new `docs/ROUTER_PLUGIN.md`.
**Steps:**
1. `trait Router { fn rank(&self, job: Job, need: Caps, candidates: &[Candidate]) -> Vec<usize>; }`, `Candidate { brain_id, model, auth, caps, limited_until, last_latency_ms }`. `DefaultRouter` is today's order. `job.weight` and `caps.tier` are what a plugin needs to send light jobs to small models and heavy jobs to big ones; `DefaultRouter` ignores them.
2. The switcher appends any candidate the router left out, so a bad plugin can't leave the user with no brain.
3. Plugin transport, defined but **not enabled**: an executable chosen in Settings, started on demand; one JSON line in (`job`, `need`, `candidates`; never the prompt, never keys), one line out (ranked ids); 300 ms timeout, then `DefaultRouter`.
4. `docs/ROUTER_PLUGIN.md`: the two JSON shapes, the timeout, what the plugin never receives. Add the setting key now.
**Test:** `DefaultRouter` equals today's order on every existing `order` case; an empty or garbage answer yields the default order.
**Done when:** the switcher takes its order from a `Router`, the document is committed, and the Settings row is still disabled.

## Item 6: Checks
- `STATUS.md`: a table from `caps()` and `auth()`: brain, sign-in, key, tools, vision, streaming, tier.
- Idle **under 150 MB**, peak **under 300 MB**, ten languages, no setting lost.

## Item 7: Everyday commands never need an AI, and every brain can use Groovy's tools (owner, 13 Nov)
The owner wants "pause the music" and commands like it to work as standard: no Claude Code or Codex sign-in, no key, and the same with a local model (Ollama, LM Studio, Groovy's own local model).
1. **No AI first, everywhere:** `okgroovy/basic.rs` (`basic::parse`/`run`), `music.rs` and `tools.rs` already run before any brain, from voice, chat and calls. Keep that order on every path, and don't let 4D items 2–3 move any of these commands behind a brain.
2. **Wider phrasing:**
   - **Music:** play, pause, resume, stop, next, skip, back, previous, "play X", "play X on Spotify", like or save the song, shuffle, repeat, "what's playing".
   - **Volume:** up, down, set to N, mute, unmute.
   - **Timers, reminders and alarms.**
   - **Open:** apps, sites and folders.
   - **System:** brightness, lock, sleep, screenshot (the screenshot only when 4C's capture exists), battery, CPU and RAM.
   - **The time and date.**
   Word order and fillers ("please", "can you", "could you", "for me", "Groovy") must not matter. Add a table test of at least 60 phrasings in `basic.rs` and `music.rs`.
3. **Tools on every brain:** the one-line JSON tool request from item 4 (`brains/cli.rs`) works for local brains too (`brains/mod.rs` `Local`), so anything the plain parser misses can still be done by whichever brain is chosen. Do this with a short system prompt plus the same strict parser, and with no extra memory at idle.
4. **Checks:**
   - With **no brain set up at all** (no keys, no sign-ins, no local model), a CI test runs the voice path for "Hey Groovy, pause the music", "volume up", "what time is it" and "set a timer for 5 minutes", and they all succeed.
   - A unit test feeds a local brain's tool reply through the same parser.
