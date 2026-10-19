# Groovy: handover for a new chat

Paste this whole file into a new chat to carry on where we left off. Written 19 Oct 2026.

## 1. The project in one paragraph

**Groovy** is SSD's (UK) Windows desktop companion app. It's a rebranded fork of the open-source **Coucou** app (Tauri 2: Rust in `windows/src-tauri/src`, TypeScript in `windows/src`).

The character lives in a frosted-glass **"island"** at the top of the screen, with tiles for Mail, Spotify, Reminders, This PC and Claude Code. He talks with the user ("OK Groovy", live voice calls, dictation), runs AI brains, and grows over time through three looks: Night → Grove, Ember → Blaze, Frost → Tide. He also has a wardrobe.

## 2. Where everything is

- **Repo:** `WavezFX/Groovy-main` (**public** since 16 Oct, to get free GitHub Actions minutes). Branch: **`groovy-ai-phase1`**. Commit as `WavezFX <clanbro26@gmail.com>`.
- **How work flows:**
  - The assistant in chat analyses, designs and writes **brief files** in the repo root, and pushes them.
  - **Claude Code** implements them by following **`WORK_QUEUE.md`**: one item per commit, each item ticked in that commit, a push after each.
  - The owner downloads installers from GitHub → Actions → the run → Artifacts (Setup, Portable, MSI).
- **Key files:**
  - `WORK_QUEUE.md`: the single ordered checklist. **Read it first.**
  - Briefs: `FIXES_ROUND3B.md`, `FIXES_ROUND3C.md`, `FIXES_ROUND3D.md`, `FIXES_ROUND4.md`, `FIXES_ROUND5.md`.
  - `STATUS.md`: Claude Code's own notes per item.
  - `editions/studio/STUDIO_EDITION_BETA.md`: the separate paid Studio edition. **Don't start it yet.**
- **Design files in the repo:**
  - `docs/design/settings-v2/`: the approved Settings redesign and the expanding island tiles.
  - `docs/design/wardrobe-v2/`: growth stages, stage 6, Wardrobe v2 and seasonal items.
  - `docs/design/island-round3/`: the island layouts and intro.
- **Design canvases on claude.ai** (private to the owner):
  - Groovy Settings Redesign: https://claude.ai/artifact/FbhbEhoUzWbvwtmnzkjsed
  - Groovy Character & Wardrobe: https://claude.ai/artifact/Grzgs5ieosC2zLPehSR1pq
  - Groovy Island Concepts: https://claude.ai/artifact/GqspECT63T7x2qAZjeZV7J
- **CI:** the workflow is `.github/workflows/windows-build.yml`, named "Windows". The cloud chat can't download build logs or artifacts. To see what failed, use `gh api repos/WavezFX/Groovy-main/actions/runs/<id>/jobs` and read the step conclusions, plus check-run annotations.

## 3. The owner's standing rules (keep these)

- **Port Coucou 0.5.0 and dotpals faithfully.** Don't remove features unless a brief says so ("don't just start removing stuff").
- **AI order:**
  - cloud and subscription brains first, local last;
  - **keep limit-based switching** between brains;
  - keep **one `Brain` interface** with a model per job, ready for a **router plugin** the owner will send later.
- **Local AI** should run like the owner's earlier app **Butler** did: **Ollama + Qwen**. The exact Qwen size isn't known yet; ask the owner for `ollama list` output.
- **Memory:** under **300 MB idle** and **never over 400 MB at peak**, counting all of Groovy's own processes (main app, WebView2 children, helper processes). Near 0% CPU when idle. Everything has to be smooth (60 fps) **and** efficient.
- **Look and feel:**
  - frosted **glass**: translucent with a background blur, clipped to the rounded shape, never a grey square;
  - **everything animated**: panels and Settings grow out of the island and fold back into it;
  - **soft settings** in the island, **hard settings** in the Settings window.
- **Browser:** DuckDuckGo is the main one.
- **Licensing:** everything should be the owner's own (legal worries). The body is Mochi-derived, and the owner chose to keep it. All Mochi/Coucou outfits and sounds must go.
- **Voice:** Fish Audio is the voice (the owner's own key and voice ID go in Settings; **never commit keys**), with Kokoro offline as the fallback.
- **Progress checks:** the owner sometimes asks for a check every 10 minutes using `send_later`. They were **stopped** on 17 Oct. Only restart them if asked.

## 4. What's done

- **Round 3, 3B and 4:** all done.
  - frosted glass;
  - hover to open;
  - bigger controls;
  - tiles open as panels in the island;
  - real mail (Gmail, Outlook, iCloud, IMAP);
  - Settings animates;
  - dictation improvements;
  - Fish Audio voice;
  - wake-word spotter and models loaded on demand;
  - DuckDuckGo;
  - PC control behind permissions;
  - no Coucou sounds left.
- **Round 3C:** items 1–6 done.
- **Round 5:** item 1 done (growth art from the canvas).

## 5. What's left (in queue order)

1. **The Windows build is RED: fix it first.**
   - The last commit (`2496695`, 3C item 6) fails the step **"Settings grows out of the island and fades in (no flash), and folds back"** (builds #128 and #130).
   - Claude Code's last commit was 17 Oct 22:05, so it has stopped.
2. **3C item 7:** checks.
3. **Round 3D** (`FIXES_ROUND3D.md`). Build to match `docs/design/settings-v2/`:
   - 1: grouped sidebar with search;
   - 2: one page template;
   - 3: rebuild every page, with CI screenshots;
   - 4: Settings animation and feel;
   - 5: **Ollama + Qwen first** for local AI, with the built-in model off by default;
   - 8: **expanding island tiles**:
     - Spotify grows 1 → 4 tiles, with 52 px play and 40 px previous/next;
     - Mail grows to 6 tiles with a vertical Spotify strip, with an "All 8" option and inbox → read → reply → send;
     - This PC grows to 6 tiles, showing RAM, CPU, disk and Groovy against 400 MB, plus top apps with End;
   - 7: **hard 400 MB peak ceiling** (a peak-scenario CI test, a small offline speech model by default with Parakeet optional, at most one extra WebView);
   - 6: checks.
4. **Round 5** (`FIXES_ROUND5.md`):
   - 2: stage 6 at six months (Sakura bonsai, Golden flame, Diamond sea);
   - 3: Wardrobe v2 (28 items plus a Diamond crown; delete every Mochi outfit);
   - 4: seasonal collection (Halloween, Winter & Christmas, Easter, Summer);
   - 6: **choose Night, Ember or Frost and name him at hatching**;
   - 5: tests.
5. **Later:**
   - **Studio Edition (Beta):** a separate paid edition on branch `studio-edition-beta`. It covers the instant cost-aware Planner, motion designs, YouTube posting, ArtCraft and free editing tools (item 3b), Open Higgsfield-style generators as external plugins, and licence keys.
   - **The phone edition:** Android smoothness plus iOS and Mac. It's waiting on the Android app's source.

## 6. The prompt to restart Claude Code

```
Pull groovy-ai-phase1 from WavezFX/Groovy-main and read WORK_QUEUE.md.

First make the Windows build green: builds #128 and #130 fail the step
"Settings grows out of the island and fades in (no flash), and folds back".
Fix the real cause, not the test.

Then finish every unticked item in order: round 3C item 7, round 3D items
1, 2, 3, 4, 5, 8, 7 and 6 (build Settings and the island tiles to match
docs/design/settings-v2/), then round 5 items 2, 3, 4, 6 and 5.

One item per commit. Tick it in WORK_QUEUE.md in the same commit, and push
after each. Keep every build green. Stay under 300 MB idle and 400 MB at
peak. Don't remove features. Keep going until every box is ticked, then
update STATUS.md with what the owner should test.
```

## 7. Open questions for the owner

- **Butler's local models:** run `ollama list` (or check Butler's code) and send the model names, so they can be set as Groovy's defaults.
- **"Free buff":** the free AI tool to add to Studio. Its name or link is needed; it couldn't be identified.
- **Fish Audio voice ID:** the voice the owner likes. It can become the default. The **key** goes in Settings only.
- **Android app source:** needed for the phone edition.
- **Licence before selling Studio:** `LICENSE` and the README say **GPL-2.0**, but `phone_link/mod.rs` says "MIT like the rest of Groovy". Decide which is right, and how Studio is licensed (a separate add-on, or selling builds and support). Get legal advice.
- **Google and YouTube:** Gmail and YouTube posting need the owner's Google Cloud project. It stays in testing mode, uploads are private until Google's audit, and Gmail is limited to 100 test users until verification.
