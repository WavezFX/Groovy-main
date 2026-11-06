# Groovy round 4: OK Groovy that always answers, quick settings, live voice chat, DuckDuckGo and more PC control

Do this **after round 3**: it depends on the bottom pill being gone and on the glass island. Same rules as before:
- work top to bottom;
- build, test and commit after each item;
- update STATUS.md;
- test on the owner's 8 GB Windows PC.

## 1. "OK Groovy" must never be silent
**Why it says nothing today (found in the code):**
- `okgroovy/brain.rs:152`: when the AI call fails, it only writes `voice: Claude turn failed` to the log and returns nothing. That happens with no API key, no subscription CLI, and local AI not chosen (`brain_ai.rs` only uses local if it was chosen). Nothing is said and nothing is shown.
- Everything OK Groovy shows and says goes to the **bottom pill** (`okgroovy/mod.rs:103` `pill::update`, `:253` `emit_to(pill::LABEL, "voice-say")`). Round 3 removes the pill, and even today, if the pill isn't showing, the answer is lost.
- It also needs:
  - OK Groovy switched **on** (off by default, `flow/store.rs:124`);
  - the **speech model** downloaded (Parakeet/Whisper);
  - a working microphone.
  
  None of these is checked out loud.

**Fixes:**
- Move all of OK Groovy's states (listening → heard "…" → thinking → answer) and its voice output into the **top island**, with the voice-state badge and live strip from round 3. Use `speak()` from the island, Kokoro or the system voice.
- **Basic commands work with no AI at all:**
  - open an app or website;
  - play / pause / next;
  - volume up, down and mute;
  - "what time is it";
  - set a timer or reminder.
  
  Groovy already recognises these from plain sentences (`tools.rs`). Try that first, and only send to the AI what it can't handle. So with zero keys, "OK Groovy, play music" still works.
- **When something's missing, say so, every time, out loud and in the island:**
  - No AI: "I heard you, but I need an AI to answer that. Add a free Gemini key in Settings", with a Set up button.
  - No speech model: a Download button.
  - Mic off: say which mic Groovy is listening to.
  - A limit was reached (from the fallback chain): say it.
- **Status in quick settings (item 2):** OK Groovy on/off, plus ✓ / ✗ for microphone, speech model and AI.
- **"Test OK Groovy" button:** say the phrase, and Groovy shows each step (heard the wake word ✓, words ✓, AI answer ✓, spoken ✓).

## 2. The gear: quick settings first, full settings one click away
Today, clicking the gear opens the Control Centre, and the quick panel is hidden behind a long press or small arrow (`src/views/views.ts:133-150`). Swap that round.
- **Click the gear → a small glass quick-settings panel** under the island:
  - **Sound:** on/off switch and volume slider.
  - **Island closes:** *When I leave* · 1 s · 2 s · 3 s · 5 s · *Click away* (stays open until you click outside it).
  - **AI:** the active brain and its state, e.g. "Gemini ✓ connected", "Claude: no key", "Local: off", plus a link to change it.
  - **Voice:** OK Groovy on/off, with ✓/✗ for mic and speech model (item 1).
  - **All settings →** opens the Control Centre.
- The panel closes when you click away. Keep it small, with no scrolling.

## 3. Talk to Groovy live, like ChatGPT Voice
"Call" currently means two things. Split them:
- **Record a call** (the Notetaker): records you and the other people, then writes notes. Keep as is.
- **Talk to Groovy:** the island's **Call** button and "OK Groovy, let's talk" start a **live, back-and-forth spoken conversation with Groovy**:
  - **Hands-free and continuous:** voice-activity detection takes your turns, with no push-to-talk. Groovy keeps listening until you say "bye", press Stop, or stay silent for about 30 s.
  - **Barge-in:** if you talk over him, he stops at once and listens.
  - **Fast:**
    - show live words as you speak;
    - stream the AI's reply and start speaking at the first sentence;
    - target **under 1.5 s** from you finishing to Groovy starting, on a cloud brain.
  - **He can act mid-conversation:** the same tools as OK Groovy (music, apps, browser, timers) and, once built, the screen buddy ("look at my screen…").
  - **Memory within the conversation**, plus a short summary saved to chat history afterwards. Saving is optional and on by default.
  - **Voice:** Kokoro (natural, local) by default, the system voice as a fallback, ElevenLabs if a key is added.
  - **Optional fast path:** if the user has an OpenAI or Gemini key, offer a **real-time voice model** (OpenAI Realtime / Gemini Live) as "Instant voice". It's lower latency and more natural. Use it only when chosen, and fall back to the normal pipeline.
  - Island state: the round 3 badge or live strip with "Talking with Groovy", the voice bars, Mute and End.
- **Phone link (calls from the owner's phone): not in this round.** It's a separate edition, to be designed later.

## 4. DuckDuckGo browser, not just Chrome
The owner's main browser is **DuckDuckGo for Windows**.
- **Settings → Browser:**
  - detect installed browsers (DuckDuckGo, Edge, Chrome, Brave, Firefox, Opera);
  - let the user pick **which browser Groovy uses**, defaulting to the Windows default browser;
  - add a separate **search engine** choice, defaulting to DuckDuckGo (`https://duckduckgo.com/?q=`).
- **Launch DuckDuckGo properly:** it's a Microsoft Store (MSIX) app, so start it through its AppUserModelID (`explorer.exe shell:AppsFolder\<AUMID>`, found with `Get-StartApps`) and pass the URL. Test on the owner's PC.
- **Control inside the browser:** DuckDuckGo for Windows has **no extension or automation interface** (unlike Chrome's DevTools protocol), so use what Windows offers:
  - **keyboard:**
    - new tab Ctrl+T, address bar Ctrl+L, then type the address and Enter;
    - back/forward Alt+←/→, reload F5, close tab Ctrl+W;
    - next/previous tab Ctrl+Tab;
  - **Windows UI Automation** to read the current tab's title and address, and to find and press visible buttons and links by name;
  - the **screen buddy's eyes**, once built, for anything else ("click the sign-in button").
- Say this plainly in Settings: "Groovy can open, search and navigate in DuckDuckGo. Filling in forms on websites works best in Chrome or Edge."
- **Every "open website" or "search" request uses the chosen browser and search engine,** whether it comes from OK Groovy, the chat, tools or the screen buddy. Remove any hard-coded Chrome.

## 5. More control of the PC (all behind permissions)
Extend Groovy's tools (`tools.rs`, the allow / ask / deny permissions and the audit log). Each action is a tool the AI and the no-AI command parser can both use:
- **Apps and windows:**
  - open, switch to and close apps (close always asks);
  - minimise, maximise, snap left or right;
  - move a window to the other monitor.
- **Media and sound:**
  - play, pause and skip for any app (Windows media session);
  - master volume, plus **per-app volume** ("turn Discord down");
  - pick the output device.
- **Discord:**
  - open it;
  - mute / unmute (Ctrl+Shift+M) and deafen (Ctrl+Shift+D) through its own shortcuts;
  - open a server or DM by `discord://` link.
  - Note: Discord has no official control interface, so only these are reliable.
- **Spotify:** as now, plus "play [song / artist / playlist]" via a `spotify:search:` link.
- **Files:** open a folder, find a file by name (Windows Search), open a recent file. Never delete without asking.
- **Windows settings**, through `ms-settings:` links or the official settings APIs:
  - dark / light mode;
  - Do Not Disturb (Focus);
  - Night light;
  - Bluetooth and Wi-Fi on/off;
  - brightness.
- **Clipboard and typing:** read or set the clipboard (asks first), type text where the cursor is.
- **System info:** battery, storage, what's using memory (helpful on an 8 GB PC).
- **Safety:**
  - Anything that closes, deletes, sends, buys or changes system settings **asks first** by default.
  - Every action goes in the audit log.
  - Groovy never acts on instructions it reads on screen or in web pages, only on the user's words.

## 6. Sounds: replace every Coucou sound
`LICENSE-ASSETS.md` reserves Coucou's sounds, and Groovy still ships them (`windows/sounds/*.wav`), including the old launch greeting.
- **Replace all of them** with Groovy's own. Preferred approach: **generate them in code** with the Web Audio API (short synth blips, chimes, pops), so there are no audio files to license, they're tiny, and they're easy to tweak. One small module, `src/core/sounds-synth.ts`, gives every event its own sound. Match the existing event names, so callers don't change.
- Keep a "Custom sound" option for users who want their own files.
- **Delete** the Coucou `.wav` files from the repo and installer.
- Write `windows/sounds/SOURCES.md` saying how each sound is made.
- Groovy's sounds should feel soft and organic (wood, leaf, water), matching the growing looks: Night more woody, Ember warmer, Frost glassier.

## Done means
- "OK Groovy, play music" works with **no AI set up**.
- With no AI, Groovy *says* what's missing, never nothing.
- The gear opens quick settings, with an "All settings" button.
- A live spoken conversation with Groovy works with barge-in and speaks within about 1.5 s on a cloud brain.
- Websites and searches open in DuckDuckGo, with keyboard and UI Automation navigation.
- The new PC controls work with permissions and the audit log.
- No Coucou sound files are left.
- STATUS.md lists what was tested and how.
