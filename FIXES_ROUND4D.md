# Fixes round 4D: provider and router layer (10 Nov)

Read `docs/plan/ARCHITECTURE_MAP.md` section 2.5 first. Same rules as round 4A.

**Today:** two dispatchers. `chat.rs` `send_to` picks an adapter by provider string, with `assistant.rs` `route` for the local fallback. `brain_ai.rs` has its own `Who` enum, `order`, `arrange`, limit switching and `CloudTally` for background jobs and « OK Groovy ». Subscriptions are reached by running the `claude` and `codex` CLIs for one text answer (no tools, no streaming). Gemini works with a key only. There is no `Brain` trait in Rust, although the docs speak of one.

**Target:** one interface every brain implements, account sign-in first with API keys optional, the same switching for chat and background jobs, and a slot a router plugin can fill later. **No behaviour change for the user** except what items 3 and 4 add. Cloud and subscription stay first, local last.

**Do this round before 4C** (vision needs the capability flags).

## Item 1: One `Provider` interface and a registry (Opus recommended)
**Goal:** every brain is one type behind one trait; adding a brain is one file.
**Files:** new `src-tauri/src/providers/{mod,anthropic,openai_compat,cli,local}.rs`; move code from `claude.rs`, `openai_compat.rs`, `local_chat.rs`, `brain_ai.rs` (`cli_complete`, `openai_turn`, the format converters). Keep the old modules as thin re-exports until item 2 is done.
**Steps:**
1. Define, in `providers/mod.rs`:
   - `struct Caps { tools, vision, streaming, web_search, local }`.
   - `enum Auth { Account { signed_in: bool }, ApiKey { present: bool }, None }` and `fn auth(&self) -> Vec<Auth>` (a brain can offer both).
   - `trait Provider { fn id(&self) -> &'static str; fn name(&self) -> &'static str; fn caps(&self, model: &str) -> Caps; fn auth(&self) -> Vec<Auth>; async fn models(&self) -> Result<Vec<ModelInfo>, String>; async fn turn(&self, req: Request, on_delta: Option<Delta>) -> Result<Reply, ProviderError>; }`.
   - `Request { model, system, messages, tools, max_tokens, timeout }` and `Reply { content, stop_reason, usage }` in Anthropic's Messages shape, which the callers already use. Each adapter converts inside itself (move `to_openai_messages`, `to_openai_tools`, `from_openai`).
   - `enum ProviderError { Limit, Network, Auth, Refused, Other(String) }` built from today's `is_limit`, `is_network` and `worth_falling_back`, so string matching lives in one place.
2. `fn registry() -> &'static [&'static dyn Provider]`: anthropic, google, openai, openrouter, claude-code, codex, ollama, lmstudio, llamacpp, custom. Ids stay exactly as stored in settings today.
3. Unit-test each adapter's request and response conversion with the fixtures the current tests use. No network in tests.
**Test:** all existing Rust tests for `claude`, `openai_compat`, `local_chat`, `brain_ai` pass unchanged or moved.
**Done when:** every adapter implements `Provider`, and `cargo test` is green on Linux and Windows.

## Item 2: One switcher for chat and background jobs
**Goal:** `chat.rs` and `brain_ai.rs` both call one function; limit switching and the Offline state behave the same everywhere.
**Files:** new `src-tauri/src/providers/switch.rs`; `chat.rs`, `brain_ai.rs`, `assistant.rs`, `phone_link/chat_backend.rs`, `calls.rs`, `okgroovy/mod.rs`.
**Steps:**
1. `async fn run(job: Job, req: Request, on_delta) -> Result<(Reply, &'static str), String>` where `Job` is `Chat | Writing | Commands | Vision`. It builds the candidate list (item 5's router, default = today's `order` + `arrange` + model per job), filters by `Caps` (a `Vision` job needs `vision`; a request with tools prefers `tools`), tries each, and moves on only on `Limit`, `Network` or `Auth` when "Switch when a limit is hit" allows (`may_switch`). `CloudTally` moves here.
2. `chat.rs` `send`, `complete`, `send_for_phone` and `brain_ai.rs` `complete`, `tool_turn` become thin callers. The chat keeps its rule that the provider picked above the chat box goes first.
3. Delete `Who` and the duplicated match arms once nothing uses them. `CLAUDE_FAST` becomes the anthropic adapter's default "fast" model, overridable by model per job.
4. Keep every user-visible message and every settings key. `tests/settings-keys.test.mjs` must pass untouched.
**Test:** port `brain_ai`'s `order` tests to the switcher; add tests: a `Limit` on the first brain reaches the second; a `Refused` does not; local-only mode never builds a cloud request.
**Done when:** `grep -n "match who\|provider == ANTHROPIC" src-tauri/src` finds nothing outside `providers/`.

## Item 3: Account sign-in first, API keys optional
**Goal:** the Brains page leads with "Sign in" for Claude, ChatGPT (Codex) and Google (Gemini); a key field is the secondary choice.
**Files:** `src-tauri/src/providers/cli.rs`, `brain_ai.rs` (`cli_path`, `ai_clis`), `agents.rs` (already knows the Gemini CLI's paths), `src/settings/brains.ts`, `src/hub/pages/*`, i18n.
**Steps:**
1. Add the **Gemini CLI** as a third subscription brain (`gemini`), beside `claude-code` and `codex`: detect it, run it for one prompt without a shell, in an empty temp folder, read-only, the way `cli_complete` does. Check its current flags with `--help` in the build; don't guess them.
2. `Auth::Account { signed_in }` for each CLI: detect a signed-in state without reading token contents (the CLI's own status command if it has one, else the presence of its credentials file). Never copy, log or send those tokens.
3. Sign in: a button that opens the CLI's own login flow in the user's browser (`claude`'s and `codex`'s login commands, Gemini's Google sign-in). Groovy does not implement its own OAuth for these and never asks for an account password. If the CLI is not installed, the row offers the official install command with a Copy button.
4. Brains page order per brain: Sign in (primary), then "Use an API key instead" (collapsed). Status pill: Signed in / Key set / Not set up. The Auto order puts a signed-in account before a key for the same brain unless the user reorders.
5. Streaming for subscriptions: read the CLI's streaming output (`claude -p --output-format stream-json`, Codex's JSON events) and feed `on_delta`, so chat no longer waits for the whole answer.
**Test:** Rust tests with fake CLI executables (a script that prints fixtures) for detect, signed-in, one answer, streaming, timeout and kill.
**Done when:** on a PC with only the Gemini CLI signed in and no keys, chat answers through it, and the Brains page shows "Signed in".

## Item 4: Tools for subscription brains (Opus; only if item 3 is green)
**Goal:** « OK Groovy » commands work on a subscription sign-in, not just text.
**Files:** `src-tauri/src/providers/cli.rs`, `okgroovy/brain.rs`.
**Steps:**
1. Give the CLI adapter `Caps.tools = true` through a strict JSON protocol: the system prompt lists Groovy's tools and asks for either plain text or one line `{"tool": name, "input": {...}}`. Parse strictly; anything else is text. The approval rules in Rust (`held`, `changes_things`, `reads_outside`) apply exactly as for a key brain.
2. The CLI itself still runs with its own tools off (`--max-turns 1`, read-only sandbox): it only writes the request, Groovy executes it.
3. If parsing fails twice in a turn, answer in text and log it.
**Test:** fixtures for a tool request, a malformed request, and a prompt-injection attempt inside a tool result (must not run).
**Done when:** "OK Groovy, pause the music" works with only Claude Code signed in.

## Item 5: The router slot
**Goal:** a router plugin can choose the brain per job later, with Groovy's switching as the backup. No plugin ships in this round.
**Files:** new `src-tauri/src/providers/router.rs`, `src/settings/brains.ts` (the disabled "Router plugin" row), new `docs/ROUTER_PLUGIN.md`.
**Steps:**
1. `trait Router { fn rank(&self, job: Job, need: Caps, candidates: &[Candidate]) -> Vec<usize>; }` where `Candidate { provider_id, model, auth, caps, limited_until, last_latency_ms }`. `DefaultRouter` is today's order. The switcher always appends any candidate the router left out, so a bad plugin cannot leave the user with no brain.
2. Plugin transport, defined but **not enabled**: an executable the user picks in Settings, started on demand, one JSON line in (`job`, `need`, `candidates`; never the prompt text, never keys) and one JSON line out (ranked ids), 300 ms timeout, falling back to `DefaultRouter`. Runs with no console and inside the job object (`platform::contain`).
3. Write `docs/ROUTER_PLUGIN.md`: the two JSON shapes, the timeout, what the plugin never receives. Keep the Settings row disabled until the owner sends the plugin; add the setting key now so it needs no migration later.
**Test:** unit tests: `DefaultRouter` equals today's order for every case in the existing `order` tests; a router that returns nothing or garbage still yields the default order.
**Done when:** the switcher takes its order from a `Router`, and the document is committed.

## Item 6: Checks
- No setting lost (`settings-keys.test.mjs`), no visible text changed except the new sign-in rows, ten languages.
- `STATUS.md`: which brains have sign-in, key, tools, vision, streaming (a table from `caps()`).
- Idle and peak unchanged. One `[build]`.
