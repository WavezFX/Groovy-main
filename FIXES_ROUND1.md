# Groovy fixes, round 1 (from the first real test on Windows, 15 Oct 2026)

These come from the owner's first hands-on test of build #13. The test PC has about 8 GB RAM, and memory hit 91% with `llama-server.exe` at about 2 GB. Each item lists what's wrong, the cause found in the code, and the fix. Work top to bottom. After each item, build, test, commit, and update STATUS.md.

## 0. CI: "Rust tests" fails on Windows (builds #11 and #13)
- The installers are built, but the Rust tests step fails on the Windows runner only. Linux passes 538/538.
- Reproduce it on Windows (or with `cargo xwin test`), fix it, and keep the "Windows" run green.
- The logs are in the run's `test-logs` artifact, in `rust-tests.txt`.

## 1. Buttons that "do nothing" (wardrobe and many other places)
**Cause A, the wardrobe** (`src/settings/wardrobe.ts`, `drawSlots`): the outfit is "Auto", and in October Auto resolves to the **witch hat** (`seasonalOutfit`, `src/mochi/wardrobe.ts:99`). In `drawSlots`, every other **head** item is then `disabled` ("Something else is worn there"), and the witch hat itself is disabled too ("Worn as the outfit"). A pumpkin costume disables *all* accessories. The user clicks greyed-out buttons and nothing happens. December (Santa hat) and Easter do the same.
- Fix: never disable wardrobe items.
  - Clicking an item in an occupied slot **replaces** what's there.
  - Clicking the item that comes from the Auto/seasonal outfit switches the outfit to "none" and keeps that item as a layer.
  - A costume swaps out for the clicked accessory.
- Show a short hint in place of the disabled state. Add tests for October and December.

**Cause B, everywhere:** `call()` in `src/core/bridge.ts:15` catches every backend error, `console.error`s it, and returns `null`. So any failing command looks like a dead button.
- Fix: add a small toast/notice component (island and Control Centre) and show the error text whenever an action the user clicked fails.
  - Use `callOrThrow` (already there) for every user-initiated action, and keep `call()` for background polling.
- Then go through every page (Control Centre, Wardrobe Studio, hub pages, island settings, tray). Click each control on Windows and list the ones that still do nothing.

## 2. Local AI eats the PC (llama-server ~2 GB, memory 91%)
- `assistant.rs:62-67`: the default is `brain: "local"`, `ai_mode: "hybrid"`. So with no API keys, every chat, call and "OK Groovy" starts `llama-server` with a local model.
- Fixes:
  1. **First-run AI setup.** Before anything loads a local model, ask which AI to use and default to a cloud option:
     - Gemini (free key from Google AI Studio, explained in one line);
     - Claude / OpenAI / OpenRouter key;
     - Claude Code or Codex subscription (the CLI, if installed);
     - "Local (needs a strong PC)".
     
     Suggest local only when RAM ≥ 16 GB. On ≤ 8 GB, warn that it will slow the PC down.
  2. **Never start llama-server at launch.** Start it only when local is the chosen brain and a request needs it.
  3. **Idle unload by default:** Lightweight profile 5 min, Balanced 10 min. Performance profile stays at 0 ("never unload"), but only if the user picks it. Show "Local model loaded, using X GB: Unload" in the tray and the Control Centre.
  4. **Kill it with Groovy:** STATUS.md lists "a killed Groovy can leave llama-server running" as a known gap.
     - Put llama-server (and every child process: ollama, whisper, the skills CLI) in a Windows **Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so it dies even if Groovy crashes.
     - Also stop it on `RunEvent::Exit`.
  5. Pick the profile automatically from RAM: Lightweight on 8 GB, which is already suggested in STATUS.md, so make it the actual default.
- Follow the AI rules in `groovy-editions` (cloud and subscription first, local last, limit fallback, one `Brain` interface).

## 3. Dictation and speech-to-text didn't work
- Most likely cause: Flow dictation is **off by default**, and the Parakeet model (about 670 MB) has to be **downloaded first** (`flow/mod.rs:304`: "Download the voice model first, in Groovy → Settings → Recognition"). That message probably went unseen, or was swallowed (see item 1B).
- Fixes:
  - **Guided setup for speech, shown the first time the user turns on dictation, calls or OK Groovy:**
    1. choose the microphone, with a live level meter;
    2. download the speech model, with a progress bar and size;
    3. run a 5-second test ("say something"), showing the words.
  - If anything's missing when the hotkey is pressed, the pill says what and offers a button to fix it.
  - Make sure the hotkey (right Ctrl by default) is shown on the Flow page and in the pill's tooltip.
  - Then test: dictation, Notetaker, voice call and OK Groovy, each with Parakeet and with Whisper, and with a cloud brain and the local brain. Write the results in STATUS.md.

## 4. The island: make it the user's own launcher and dashboard
Today the island's Integrations view is a list of developer statuses ("Hooks installed", "Open Visual Studio Code", Claude Code integration…). The owner wants the island to be **a customisable home for their own apps and services**.
- **"My island" editor** (Control Centre → Island):
  - the user picks which tiles appear, drags them to reorder, and chooses compact or expanded for each;
  - presets: "Developer" (today's agents and hooks), "Everyday" (mail, calendar, music, apps), "Custom".
- Tile types:
  - **App launchers:** any installed app (Start-menu list), website or folder, with its icon.
  - **Services:** the existing Resend, Vercel, Neon, GitHub, Stripe pills etc. become optional tiles, not always-on rows.
  - **Mail:** unread count and the latest sender and subject; click opens the mail app.
  - **Music:** Spotify / the local player.
  - **Agents:** Claude Code, Codex etc. (today's view), for developers.
  - **Reminders / to-dos, calendar next event, system status.**
- **Email pop-ups on the island:** the simplest route needs no accounts.
  - Read new-mail notifications from **Windows' own notification feed** (`Windows.UI.Notifications.Management.UserNotificationListener`, with a one-time permission prompt).
  - Show them as an island pop-up: sender, subject, Open and Dismiss.
  - It works for Outlook, Mail, Gmail in the browser, and anything else that shows Windows notifications.
  - Let the user pick which apps' notifications appear on the island.
  - Later, an optional Gmail/Outlook sign-in (Google API / Microsoft Graph) for a full unread count and inbox preview.
- Keep the hooks and agents setup, but move it to Control Centre → Agents. It's set up once, so it doesn't need to sit on the island.

## 5. Spotify: explain it in the app
- On Windows the Spotify pill reads the system media controls (`spotify.rs`, the GlobalSystemMediaTransportControls part).
- It shows while the **Spotify desktop app** is playing: track, artwork, play/pause/skip, and Groovy dances to it.
- Voice and chat ("play some jazz") go through the music tools.
- Add an empty state to the Music tile and the Spotify pill: "Play something in the Spotify app and it appears here", plus a button that opens Spotify (or installs it from the Microsoft Store).
- Check that the pill appears with the Microsoft Store version of Spotify and with the spotify.com installer version.

## 6. Settings are hard to find
- Settings only appear once at the first start, and after that only by finding the icon.
- Add **Settings** and **Open Groovy** to the tray menu, a right-click menu on the island and the companion, and a shortcut (Ctrl+Alt+, by default).
- Make sure the first-run screen can be reopened from Settings → About ("Run setup again").

## Done means
- Each item works on a real Windows PC with **no API keys** and also with a Gemini key.
- No greyed-out or silent controls are left.
- llama-server never runs unless local AI is chosen, and it's gone when Groovy closes.
- STATUS.md is updated with what was tested and how.
