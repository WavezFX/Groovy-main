# Fixes round 3D: a professional Settings window, and a local AI that runs smoothly

**Do this after round 3C and before round 5.** Commit each item separately with "Fixes round 3D, item N: …" as the subject, tick it in `WORK_QUEUE.md` in the same commit, push, and keep the build green.

> **Approved designs:** `docs/design/settings-v2/`. Open the `.dc.html` files in a browser.
> - `Sidebar.dc.html`: the shared sidebar (Groovy card, search, 15 pages in 8 groups).
> - The pages: `Main` (Home), `Brains`, `Voice`, `Groovy` (character and wardrobe), `Island` and `System`.
> - `IslandTiles` and its variants: the expanding island tiles.
>
> Build Settings and the island to match these exactly: layout, spacing, colours (#0A0C0E ground, glass cards, #46EB78 accent for Night, following the current look), type sizes and copy. The pages not drawn follow the same template.

## Item 1: Reorganise Settings (information architecture)

The owner says some Settings pages "aren't professional" and their layout needs smoothing out on every tab. Today the sidebar is a flat list of **26 pages**: 18 Groovy pages (`GROOVY_PAGES`, `src/hub/shell.ts`) plus 8 Flow pages. Related settings are split across pages:

- **AI is on three pages:** Assistant, Chat providers and Models.
- **The island is on two:** My island, and Pills and integrations.
- **Voice is on five:** Voice calls, OK Groovy, Dictation, Dictation settings and Notetaker, with Speech setup hidden.

Regroup the sidebar into **collapsible sections**, using the same page ids where possible so deep links keep working:

1. **Home:** a new overview page.
   - Groovy's look and stage.
   - Which brain is active.
   - Voice status.
   - Memory use.
   - Quick toggles: OK Groovy, Dictation, Do not disturb.
   - Any warnings, each with a "Fix" button.
2. **Groovy:** Character & wardrobe (wardrobe, growth, name), Sounds.
3. **AI:**
   - **Brains:** Assistant + Chat providers + Models merged into one page, with tabs **Cloud**, **Subscriptions** and **On this PC**, plus the Auto order and the limit-based switching. The `Brain` interface and the router-plugin slot stay.
   - **Memory.**
   - **Skills.**
4. **Voice:**
   - **Speaking:** Fish Audio, Kokoro, voice test.
   - **Listening:** OK Groovy, wake words, microphone.
   - **Live calls.**
   - **Dictation:** dictation settings, dictionary, snippets and styles as tabs.
   - **Notetaker.**
5. **Island:**
   - **Layout & tiles:** My island + Pills merged.
   - **Accounts & integrations:** mail, Spotify, GitHub, Vercel, Neon, Resend and the rest. Connect and disconnect in one place.
   - **Music.**
6. **Productivity:** Reminders, Breaks, Statistics.
7. **Computer:** Browser, Permissions, Coding agents, Android phone.
8. **App:**
   - **General:** language, theme, shortcuts, startup.
   - **System:** performance, logs, diagnostics.
   - **About & updates:** version, licences, credits.

Other navigation:
- Add a **search box** at the top of the sidebar that searches every setting by name and keyword, jumps to its row and highlights it briefly.
- Remember the last open page and which sections are collapsed.

## Item 2: One page template, used by every page

Every page uses the same building blocks (`src/hub/ui.ts`), in the style of Windows 11 Settings with Groovy's glass look:

- **Page header:** title (24 px, semibold), one-line subtitle (13 px, dim), and optional actions on the right. No long paragraphs at the top.
- **Cards** group related settings, with a small uppercase group label above each card. Rows sit inside the cards, separated by hairlines.
- **Each setting row:**
  - on the left, the label (14 px) and an optional one-line description (12 px, dim);
  - on the right, the control: toggle, dropdown, button, slider or value.
  - Everything is vertically centred, with a minimum height of 56 px.
  - Long help text goes behind an "ⓘ" tooltip or a "Learn more" expander, not in always-visible hint paragraphs. Today there are 37 `hint` blocks in `settings/main.ts` alone.
- **Spacing:** an 8 px grid; 24 px page padding, 16 px between cards and 12 px inside rows. One maximum content width (about 760 px) and centred, so wide windows don't stretch the rows.
- **Buttons:** one style each for primary, secondary, subtle and danger, and at most one primary per card. Danger actions (delete memory, disconnect, reset) always ask first.
- **Status:** pills for Connected / Not set up / Error / Downloading (with progress), with the same colours everywhere.
- **Empty states:** an icon, one sentence and the action button. Never a blank card.
- **Forms:** API keys and passwords use a masked field with show/hide, a **Test** button and a clear result ("Connected as …" or the error in plain words).
- **Saving:** every change saves automatically, with a small "Saved" toast. No Save buttons, except in multi-field forms like adding an account.
- **Text:** plain, friendly, consistent sentence case. No internal names (file names, ids, "pills").
- **Accessibility:** works with the keyboard alone, visible focus rings, 4.5:1 contrast, a screen-reader label on every control, and works at 100–200% scaling. Light and dark themes both checked.

## Item 3: Rebuild every page on the template

Go through **every** page (Groovy, Flow and the hidden ones) and rebuild it with item 2's components, merging pages as item 1 describes. For each page:

- Remove duplicates: a setting appears in exactly one place, and other places link to it.
- Put the most-used settings at the top, and move advanced ones into an **Advanced** expander at the bottom.
- Write the page's purpose as its subtitle.
- Add one CI screenshot per page in dark and light, uploaded as an artifact (`settings-screens`) so the owner can review them.

Keep every existing feature and setting key. This is a layout change, not a removal. Migrate any renamed keys.

## Item 4: Animation and feel

- Use the motion module from round 3B item 8 throughout: pages slide and fade, sections collapse smoothly, toggles animate, and the toast slides in.
- The Settings window opens out of the island and closes back into it (round 3C item 6).
- **Performance:** a page switch takes under 100 ms. Build heavy pages (Models, Statistics) on demand. Idle CPU while Settings is open and untouched stays near 0.

## Item 5: Local AI that runs as smoothly as Butler did

The owner's earlier assistant, Butler, ran **Qwen through Ollama**, and it ran smoothly on his laptop. Groovy's built-in local model (Gemma 3 1B) struggles.

- **When Ollama is installed and running**, it becomes the **first local option**: on this PC, try Ollama, then LM Studio, then the built-in model. Cloud and subscription brains still come first overall, and the Auto order and limit switching are unchanged.
- **Default Ollama model:** a Qwen model sized for the PC. Use qwen2.5:3b (or the current Qwen 3 small equivalent) on 16 GB RAM, and a 7B or 8B model only with a capable GPU or 32 GB or more.
  - Show the choice under AI → Brains → On this PC, with a **"Use Ollama + Qwen"** one-click setup: if Ollama is missing, link to its installer; if the model is missing, pull it with a progress bar.
  - Detect installed models with `/api/tags` and let the user pick any of them.
- **Keep it light:** Groovy calls Ollama's HTTP API and never loads the model itself. Use Ollama's `keep_alive` (about 5 minutes) so the model is unloaded when idle, in line with round 3B item 10.
- **Speed:** stream tokens to the island and to TTS (sentence by sentence), so replies start quickly.
- **The built-in model** stays as a fallback for people without Ollama, running out of process (round 3B item 10).
- **Tests:** mocked `/api/tags` and `/api/chat` check the order, model selection, the keep-alive, and the fallback when Ollama is down.

## Item 7: A hard 400 MB ceiling, even at peak

The owner's rule: **Groovy must never use more than 400 MB of RAM, even at peak use.** The idle budget (300 MB, round 3B item 10) stays as well.

**What counts:** every process Groovy owns, measured as the private working set (the "Memory" column in Task Manager):
- the main process;
- all its WebView2 children (island, Settings, renderer, GPU, utilities);
- any helper processes Groovy starts, such as speech, ffmpeg or a built-in model.

Apps the user installed themselves (Ollama, LM Studio, Spotify, ArtCraft) are their own processes and don't count. Settings → System should say so plainly.

**Peak scenario test in CI** (fails if the total is ever above 400 MB, sampled every 250 ms). Run all of this at once:
- the island open with the music layout;
- Settings open on the heaviest page;
- a dictation, a live voice call (with the speaking voice), and an OK Groovy command;
- a mail refresh, Reminders open in the island, and the wardrobe open.

Also report the peak and the idle numbers in `STATUS.md`.

How to stay under the ceiling:
- **At most one extra WebView at a time** besides the island. Opening Settings closes any other panel window, and the wardrobe and detail cards live inside the island page, not in new windows.
- **Local speech recognition must fit.** Parakeet (about 670 MB) can't fit under 400 MB.
  - Make the default offline engine a small model in a helper process: for example Whisper base or small (int8), or Moonshine, at about 60–250 MB. Cloud speech recognition still comes first.
  - Keep Parakeet as an optional "High accuracy (uses more memory)" choice, **off by default**. When it's chosen, Settings clearly says it goes over Groovy's 400 MB limit.
  - Free the speech model after idle (round 3B item 10).
- **Only one model loaded at a time:** never speech recognition and the voice model together if that would cross the limit. Queue or stream instead.
- **Local AI:**
  - Ollama or LM Studio (round 3D item 5) does the thinking in its own app, so Groovy stays under 400 MB.
  - The **built-in local model can't fit** (even a 0.5B model needs about 400 MB on its own). Keep it, but **off by default**, labelled "Uses extra memory: over Groovy's 400 MB limit". The CI peak test runs with it off.
- **Caches:** cap the image, sprite and mail caches, and use bounded queues everywhere. Watch WebView2 memory: when the total nears 380 MB, Groovy drops its caches, sets `MemoryUsageTargetLevel` to Low on hidden views and closes the least-recently-used panel.
- **Kokoro voice (local TTS):** load it on demand and free it when idle. It must fit within the peak scenario, or run in a helper process that's freed straight away.

## Item 8: Island tiles that expand (approved design: `IslandTiles.dc.html`)

The island's tile area is a 4 × 2 grid. Tiles grow **out of their own spot**, the other tiles slide aside, and a **Back** button returns them to one tile. All of this uses the motion module.

- **Spotify:** 1 tile becomes **4 (2 × 2)**, showing the art, song, artist and a progress bar you can scrub. Previous and next are **40 px**, and play/pause is **52 px**. The other four tiles stay visible.
- **Mail:** 1 tile becomes **6** (three columns). While music is playing, Spotify becomes a **2-tile vertical strip** in the fourth column, with art, title, a 44 px play/pause and 34 px previous/next.
  - An **"All 8" / "6 tiles"** switch lets Mail take the whole grid.
  - Inside Mail: the inbox list (sender, subject, time, unread), then reading an email (subject and body; **Reply**, **Archive**, **Open in …**), then a reply box (**Send**, plus **"Groovy, polish it"**, which rewrites it with the active brain).
  - Back goes from reply to email to inbox to tiles. Uses round 3C item 5's accounts.
- **This PC:** 1 tile becomes **6**, with the same music strip. It shows memory, CPU, disk, **Groovy against its 400 MB limit**, battery and Wi-Fi, plus "Top apps by memory" with **End** (which asks first, behind round 4 item 5's permissions). It updates every 2 s only while open.
- **Other tiles** (Reminders and the rest) follow the same pattern: grow to 6 with the music strip, or to 4 if that suits them.
- Only one tile is expanded at a time. Esc or a click away collapses it. Everything stays inside the island window: no new windows, and it respects the 400 MB peak (item 7).

## Item 9: Lazy-load everything: nothing loads until it's needed

The owner's rule: **every component loads only when it's actually needed, and unloads when it's no longer used.** For example, **in cloud mode the offline language model must never be loaded at all**: not at startup, not "just in case", not as a warm-up.

- **AI brains:**
  - Load a local model (built-in, or a request to Ollama or LM Studio to load) only when a local brain is actually chosen for a request, meaning the cloud brains are unavailable or out of limit, or the user picked local.
  - In cloud mode, the local runtime, its weights and its helper process don't exist.
- **Speech:**
  - The offline speech recognition model loads only when cloud speech recognition isn't available or fails, and only when the user is actually speaking.
  - Kokoro loads only when Fish Audio can't answer.
  - The wake-word spotter runs only while OK Groovy is on.
- **UI:**
  - Each Settings page's code is a separate dynamic import, loaded the first time the page opens. Heavy pages (Brains/Models, Statistics, Wardrobe previews, the Coding Studio editor and CodeMirror) are never in the startup bundle.
  - Island panels (Mail, This PC, the full Spotify player, Reminders) load their code and data when first opened.
  - Settings, Studio, the wardrobe and the desktop Groovy windows are created on first use and destroyed when closed.
- **Integrations:**
  - Mail, calendar, GitHub and the other integrations don't connect or poll until their tile is visible or their panel is open, or a notification setting needs them. Then they poll at the slowest rate that still works.
  - Disconnected integrations load nothing.
- **Rust side:**
  - Group heavy modules (whisper, sherpa-onnx, local LLM, ffmpeg, the image tools) behind runtime initialisation, using `OnceCell` or a lazy constructor, so nothing is initialised at startup.
  - Where possible, put rarely used, heavy native libraries in a **helper process** started on demand, so their memory is fully returned when they stop.
- **Startup:**
  - Startup does only what's needed to show the island and Groovy.
  - Measure cold start to island visible (target **under 1.5 s**) and idle memory right after startup. Put both in `STATUS.md`.
- **Tests:**
  - In cloud mode with models downloaded: start Groovy, chat a few times and run a dictation with cloud STT. Assert that **no local LLM or offline STT model was ever loaded** (log or counter check) and that no helper process for them exists.
  - Assert that opening a Settings page loads its chunk on first visit only.
  - Add these to the peak/idle memory tests (item 7).

## Item 10: Three operating states: Standby, Conversation, Offline

Efficiency must **not** cost features, interface quality or voice responsiveness. Groovy runs in exactly one of three states, managed by one state machine in Rust (`runtime_state.rs`). The island shows the state subtly, and Settings → System shows it in full.

| State | What's loaded | What's not loaded | Budget |
|---|---|---|---|
| **Standby** (default) | The island WebView (visible, animated only when something changes), the **wake-word spotter** (a few MB, VAD-gated), integrations visible on the island (polling slowly), a cloud connection ready but idle | Full speech recognition, every local model, Kokoro, the Settings and Studio windows, closed panels | **≤ 300 MB, under 1% CPU** |
| **Conversation** (wake word, hotkey, dictation or a live call) | Everything in Standby, plus **cloud speech recognition streaming**, the cloud brain, Fish Audio streaming TTS and the panels in use | Local models (unless cloud fails) | **≤ 400 MB peak** |
| **Offline** (no network, cloud limits hit with no cloud fallback, or the user chose local) | Standby, plus the **small offline STT** and **Kokoro** on demand, with local thinking through **Ollama** (its own app; Groovy only calls it) | The built-in model, unless the user turned it on | **≤ 400 MB peak** for Groovy's own processes |

**Moving between states:**

- **Standby → Conversation:** triggered by the wake word, the hotkey, the island's Talk button or a live call.
  - When the wake word fires, **pre-warm** at once and in parallel: open the cloud STT stream, send a warm-up to the brain (keep-alive HTTP/2 connection), and get the TTS stream ready.
  - Buffer the audio from the wake word onwards, so **no words are lost** while things connect.
  - Targets: first sound of feedback (chime and island glow) **under 150 ms**, words on screen **under 500 ms**, and the start of the spoken reply **under 1.5 s** after the user stops.
- **Conversation → Standby:** after the reply finishes and about **8 s** of silence (follow-up window), or at once when the user says "thanks" or "stop".
  - Then **release** STT buffers and streams, TTS audio buffers, panel data no longer shown and any temporary caches.
  - Memory must return close to the Standby figure within about 10 s. Test this.
- **→ Offline:** entered automatically when the network is down or cloud limits are hit with no cloud brain left. Groovy shows it on the island ("Offline: thinking on this PC") and loads only what's needed per request. It returns to Standby when the cloud is reachable again. If the offline engines aren't downloaded, Groovy says so and the basic commands still work.
- **The island stays visible and live in every state.** Groovy's animations, hover-to-open and the tiles work in Standby. Only invisible work stops.

**WebView2 audit:** the owner's Task Manager earlier showed about 9 WebView2 processes. Check whether Tauri creates extra WebViews or keeps hidden windows alive.

- List every window and WebView created at runtime (log them with their purpose).
- Target in Standby: **one** WebView (the island) plus WebView2's own shared processes (browser, GPU, utility). That means no hidden Settings, Studio, desktop-Groovy or helper WebViews kept alive.
- Every window other than the island is created on first use and **destroyed** on close (not hidden). Confirm in Task Manager terms that its renderer process exits.
- All windows share **one** WebView2 environment and user-data folder, so they share the browser, GPU and network processes instead of spawning new sets.
- Turn off WebView2 features Groovy doesn't use (for example, unused extensions or devtools in release builds, and spell-check where not needed), and set `MemoryUsageTargetLevel` Low whenever a WebView is hidden.
- In CI, record the process list in Standby and fail if any extra renderer process exists.

**Tests:**
- State-machine unit tests for every transition.
- An end-to-end CI test: Standby → wake word (simulated audio) → Conversation using mocked cloud STT, brain and TTS → back to Standby. Assert the latency targets (mocked), that no local model was loaded, the memory after release, and Standby ≤ 300 MB / peak ≤ 400 MB.
- An Offline test with the network cut: the offline engines load on demand and unload after.

## Item 11: Match Coucou's idle footprint (memory audit)

**What the owner measured** (Task Manager, idle):
- **Coucou:** about **7.6 MB**.
- **Groovy:** about **80 MB** for the main process, plus about **140 MB** of WebView2, so about **220 MB** in total.
- **The goal:** Groovy at idle should be **no heavier than Coucou**, and modest at peak.

**What we know:**
- Coucou's Windows app is **also Tauri 2 + WebView2**: the same stack as Groovy.
- So Coucou's 7.6 MB is almost certainly **only its main Rust process**. Its WebView2 processes are listed separately ("WebView2: Coucou" entries, or under the "Microsoft Edge WebView2" group), so they are easy to miss.
- The fair comparison is main process vs main process (**7.6 MB vs 80 MB**), and WebView2 vs WebView2.
- The 10× gap in the main process is real and must be closed.

**Step 1: measure both apps the same way.**
- In CI, build **upstream Coucou** (`Louis-CFM/coucou`, its `windows/` app, latest release tag) next to Groovy, using the same runner and the same `smoke-test.ps1`.
- Record the idle **private working set** of the **main process** and of **every WebView2 child**, for both apps, after 60 s idle and after 10 min idle.
- Put the side-by-side table in `STATUS.md` and the CI summary.
- This gives honest baseline numbers for the owner.

**Step 2: find what Groovy's main process holds at idle.**
- Use a heap profiler (`dhat`, or Windows heap snapshots with VMMap or `HeapWalk` in a diagnostic build).
- List the top allocations by module.
- Count threads (each reserves stack) and timers.
- Today, `lib.rs` setup starts all of these in every run:
  - cursor poll;
  - tasks;
  - mail (and the mail watch);
  - the brain_ai init;
  - music;
  - two idle reapers;
  - the named pipe;
  - phone link (only when enabled);
  - integrations;
  - Spotify sync;
  - shortcuts;
  - the composition backdrop;
  - the glass watch.
- Check each one: is it needed at idle, and how much does it hold?
- The usual suspects:
  - sherpa-onnx or onnxruntime environments created at startup, plus the wake-word or VAD model;
  - a tokio runtime with many worker threads (use a small `current_thread` or 1–2 worker runtime);
  - the `reqwest` client and TLS roots built eagerly;
  - `sysinfo` refreshing everything;
  - the rodio output stream kept open;
  - large `include_bytes!` assets;
  - the SQLite page cache;
  - logging buffers.

**Step 3: cut it.**
- **The main process does only:** window and tray, the hotkey, the state machine (item 10), settings and IPC. Everything else is lazy (item 9).
- **Native engines go into helper processes,** started on demand and exiting when idle: speech (sherpa-onnx, whisper), wake word, TTS (Kokoro), local LLM, ffmpeg and image tools.
  - The **wake-word helper** runs only while OK Groovy is on. Its few MB are reported separately in System, so the owner can see exactly what OK Groovy costs.
  - When OK Groovy is **off**, nothing audio-related is resident.
- **Background work:**
  - One small async runtime.
  - Timers coalesced into one scheduler tick that **stops** when nothing needs it.
  - No polling loops at idle: use events (the cursor via hit-test or mouse-leave events instead of a poll; GSMTC events for music; push or IDLE for mail).
- **Dependencies and features:**
  - Remove dead code and unused dependencies and features. Run `cargo udeps` or `cargo machete` and a feature audit.
  - Put rarely used modules (phone link, studio, livekit, portal, codex plan) behind runtime enable flags, so they don't initialise unless switched on. Don't delete them.
- **The frontend:**
  - Keep the island's startup bundle small, split per window and per panel (item 9).
  - No large libraries in the island bundle (no CodeMirror, no markdown engine).
  - Measure the JS heap of the island at idle.
- **WebView2:**
  - One island WebView at idle (item 10).
  - Turn off unused browser features.
  - Use `MemoryUsageTargetLevel` Low whenever the island is collapsed.
  - Check whether the WebView can be **suspended** (`TrySuspend`) when the island is fully hidden or Groovy is idle in the tray, then resumed instantly on hover or the hotkey.

**Targets (CI-enforced, private working set):**

| Measure | Target |
|---|---|
| Main process idle, OK Groovy off | **≤ 15 MB**, about Coucou's level. Report the exact Coucou figure next to it. |
| Main process idle, OK Groovy on | ≤ 15 MB, plus a wake-word helper of ≤ 25 MB |
| Island WebView2 total idle | ≤ Coucou's WebView2 total + 10% |
| Whole Groovy idle | Standby budget ≤ 300 MB stays the hard cap. Aim for **under 150 MB**. |
| Peak | ≤ 400 MB (item 7) |

- Fail CI if a target is missed.
- Add a "Groovy vs Coucou" line to the CI summary.
- **Nothing is removed from Groovy's features to get there**: features become lazy, not gone.

## Item 6: Checks

- All tests pass.
- Settings screenshots are uploaded for every page.
- Idle memory stays under 300 MB, the **peak stays under 400 MB** (item 7), and CPU near 0 with Settings closed, and Settings closes fully (round 3B item 10).
- No setting was lost: a test loads an old settings file and checks that every key is still reachable in the new layout.
