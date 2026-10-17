# Fixes round 3D: a professional Settings window, and a local AI that runs smoothly

**Do this after round 3C and before round 5.** Commit each item separately with "Fixes round 3D, item N: …" as the subject, tick it in `WORK_QUEUE.md` in the same commit, push, and keep the build green.

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

## Item 6: Checks

- All tests pass.
- Settings screenshots are uploaded for every page.
- Idle memory stays under 300 MB and CPU near 0 with Settings closed, and Settings closes fully (round 3B item 10).
- No setting was lost: a test loads an old settings file and checks that every key is still reachable in the new layout.
