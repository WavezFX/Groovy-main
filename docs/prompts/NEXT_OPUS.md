# Saved prompts for Claude Code (updated 4 Nov)

Two sessions. Opus only for the hard one; Sonnet is cheaper for the rest. `/clear` before each, `/model` as stated, then paste.

## Session A: Opus 5.5 (the new body + wardrobe)

```
Pull groovy-ai-phase1 and read WORK_QUEUE.md. Work in this order, no other rounds:
1. Round 3K items 1 and 2 (FIXES_ROUND3K.md: pillow outline, element wisps; design approved, see docs/design/body-v2/).
2. Round 3H item 1 (FIXES_ROUND3H.md: wardrobe fit) on the NEW body, same pass.
3. Round 3K item 4 (contact-sheet CI) and the 3H checks. Make ONE [build] commit at the very end so I get one installer.
Rules: don't remove any user-facing feature, keep all Linux code, one item per commit ticked in WORK_QUEUE.md, push after each, fix red builds first, stay under 300 MB idle / 400 MB peak.
Token rules (important, my usage is limited): read only the item you are on, not whole round files or whole source files; grep/glob first, then read line ranges; never re-read a file you just edited; no subagents; no narration between tool calls; cap command output (tail -n 40, --quiet); don't run full test/build suites more than once per item; final reply at most 3 lines. Stop and tell me if you are about to spend a lot on something not in the list.
```

## Session B: Sonnet 5.5 (one run: 3L, then a build, then the rest)

```
Pull groovy-ai-phase1 and read WORK_QUEUE.md. Work in this order, in one run:
1. Round 3L (FIXES_ROUND3L.md, the wardrobe fit fixes). End it with a [build] commit and push, so a Windows installer is built for me BEFORE you go on. Then carry straight on, don't wait for me.
2. Round 3D item 11 (FIXES_ROUND3D.md).
3. Round 3I (FIXES_ROUND3I.md).
4. Round 3J (FIXES_ROUND3J.md).
Make one [build] commit at the end of each of 3I and 3J (3D item 11 only if it changes the app). Stop after 3J.
Rules: don't remove any user-facing feature, keep all Linux code, one item per commit ticked in WORK_QUEUE.md, push after each, fix red builds first. Memory budget is 150 MB idle / 300 MB peak (see WORK_QUEUE.md).
Token rules (my usage is limited): read only the item you are on, not whole round files or whole source files; grep/glob first, then read line ranges; never re-read a file you just edited; no subagents; no narration between tool calls; cap command output (tail -n 40, --quiet); don't run full test/build suites more than once per item; final reply at most 3 lines. Stop and tell me if you are about to spend a lot on something not in the list.
```

## Session C: Sonnet 5.5 (finish what Session B left, 4 Nov 15:15)

```
Pull groovy-ai-phase1 and read WORK_QUEUE.md. Finish the leftovers, in this order, in one run:
1. FIX THE RED CHECK FIRST. The Check workflow fails at "Front-end tests": tests/i18n.test.mjs ("every string the code shows is translated in every language") lists new untranslated strings from rounds 3I and 3J (src/settings/main.ts "Quiet during games and presentations" and its description; src/settings/memory.ts "Links to", "Linked from", "Notes folder", "Move", the notes-folder sentence; src-tauri/src/chat.rs and memory.rs "forget" / "keep" / "ended note" messages). Add proper translations for all ten languages the way the test and the existing strings expect, run only that test until it passes, then push. Make sure the next Check run is green before going on.
2. Round 3J items 4 (clipboard history tile, opt-in, memory only) and 6 (Dialog Jump, optional) from FIXES_ROUND3J.md. If an item cannot be built or checked safely without Windows, build what you can behind its setting, say so in STATUS.md, and move on. Tick what is done in WORK_QUEUE.md.
3. Round 3D item 11 (FIXES_ROUND3D.md: match Coucou's idle footprint). Do the code-side savings you can verify; for anything that needs real Task Manager numbers, make the CI smoke test print the private working set and write what you need measured in STATUS.md. Do NOT lower the CI memory gates blind (see WORK_QUEUE.md memory rule: 150 MB idle / 300 MB peak is the target; set each gate to the current measured number plus a small margin).
4. One [build] commit at the very end (and only if the Check run is green). Stop.
Rules: don't remove any user-facing feature, keep all Linux code, one item per commit ticked in WORK_QUEUE.md, push after each, fix red builds first.
Token rules (my usage is limited): read only the item you are on, not whole round files or whole source files; grep/glob first, then read line ranges; never re-read a file you just edited; no subagents; no narration between tool calls; cap command output (tail -n 40, --quiet); don't run full test/build suites more than once per item; final reply at most 3 lines.
```

## Session D: Opus 5.5 (3D item 11, the idle footprint)

```
Pull groovy-ai-phase1. Read ONLY: WORK_QUEUE.md (memory rule), FIXES_ROUND3D.md item 11, and STATUS.md "Round 3D item 11, what to measure first". Goal: cut Groovy's real idle memory toward the owner's target of 150 MB idle / 300 MB peak (Task Manager private working set, all Groovy processes). Owner's earlier measure: groovy.exe about 80 MB plus WebView2 about 140 MB at idle; Coucou about 7.6 MB main process. Do it in this order:
1. Get real numbers: read the latest successful Windows build run summary (smoke-test prints "private: groovy.exe", one line per WebView2 process, TOTAL private working set, idle CPU) with gh. If you can't read them, add a step that publishes them as a build artifact and a line in STATUS.md, push one [build], and read them from that. Don't guess.
2. Find where the main process holds more than 15 MB (dhat diagnostic build or counting what is loaded at launch) and fix the biggest items first. Then move native engines (sherpa-onnx, Kokoro, ffmpeg helpers) to on-demand helper processes that exit when idle. Check every WebView2 window: nothing created until needed, destroyed when closed.
3. After each real saving, re-measure from the next build, and only then set each CI gate (smoke test, peak test) to the measured number plus a small margin, tightening toward 150 / 300. Never lower a gate blind and never turn a build red.
4. Don't remove any user-facing feature, keep all Linux code, one item per commit ticked in WORK_QUEUE.md, push after each, fix red builds first. Before pushing, if you added Tauri commands run `node scripts/command-access.mjs` and the capabilities test; if you added visible strings, translate all ten languages (tests/i18n.test.mjs). Finish with one [build] commit and write before/after numbers in STATUS.md.
Token rules (my usage is limited): read only what the step needs, grep first then line ranges, never re-read a file you just edited, no subagents, no narration between tool calls, cap command output (tail -n 40), run full builds/tests once per step, final reply at most 3 lines.
```

## Session E: Sonnet 5.5 (fix the red wardrobe-fit check, 4 Nov 17:35)

```
Pull groovy-ai-phase1. Windows build #216 fails only at the step "Settings screenshots (every page, dark and light)": the round 3L item 5 wardrobe-fit check prints "729 problems in all" (lenses "3 to 4 % off its eye" at yaw -0.5/-1 and pitch +-0.3, face items "reach past his body width" at yaw -0.35/-0.7/+0.35, a few "px off his eyes" at yaw -1). All memory and CPU gates pass, so touch nothing else. Do this:
1. Read the check's code (grep "off its eye" and "reaches past his body width" in windows/scripts and windows/src) and the 3L rules in FIXES_ROUND3L.md items 1 and 4. Don't read whole files.
2. Fix the real misfits in outfits.ts first: face items (glasses, shades, visor, monocle, freckles, stickers, eye patch) must stay on his eyes and inside the body outline for yaw -0.7..0.7 and pitch -0.3..0.3 (lens centre within 2 % of the eye; the item's outer edge, arms excluded, inside the outline). Items that follow the turn must use the same yaw/pitch the eyes use.
3. Then, only if the check is stricter than the 3L brief, set its tolerances to the brief's numbers. Check only yaw -0.7..0.7 for the "past body width" rule; outside that (yaw -1) report as warnings, not failures. Never loosen it just to go green: say in STATUS.md what changed and why.
4. Also fix the build summary so this step's report names the worst 10 problems, not 729 lines.
5. Run only the front-end/outfit tests locally, push, wait for the Windows build; fix red first. One [build] commit at the end. Final reply at most 3 lines.
Rules: don't remove any user-facing feature; if you add Tauri commands run node scripts/command-access.mjs; translate any new visible text in all ten languages.
Token rules: read only what the step needs, grep first then line ranges, never re-read a file you just edited, no subagents, no narration between tool calls, cap command output (tail -n 40).
```

## Session F: Opus 5.5 (round 3M, the wardrobe is worn)

```
Pull groovy-ai-phase1. Read WORK_QUEUE.md (memory rule) and FIXES_ROUND3M.md only, then do round 3M items 1 to 5 in order. The owner tested build 217 on his PC and the clothes still look stuck on: lids for hats, see-through bands, an eye-patch strap that stops mid-face, floating headphones, a tie drawn over his eyes, flat collars, and nothing moves like real clothing. Fix it properly with one fake-3D head model (an ellipsoid with the same yaw/pitch the eyes use) as the brief says. Build on outfits.ts, don't start over, and don't touch the wisps, growth art, eyes, glow or body.
Before each push: if you added Tauri commands run node scripts/command-access.mjs; translate any new visible text in all ten languages (tests/i18n.test.mjs); run the outfit tests and the item-fit check locally. Fix red builds first. One item per commit, ticked in WORK_QUEUE.md. One [build] commit at the very end, and confirm the Windows build job actually ran (not skipped) and the release updated.
Memory: under 150 MB idle, never over 300 MB peak; springs only while visible and still at rest (0 CPU).
Token rules (my usage is limited): read only the item you are on; grep first, then line ranges; never re-read a file you just edited; no subagents; no narration between tool calls; cap command output (tail -n 40); full builds/tests once per item; final reply at most 3 lines.
```

## Optional token savers (owner's call; test first, keep only if it clearly helps)
- `npx ccusage` (offline, reads local logs): shows where tokens go per session. Measuring only.
- `rtk` (github.com/rtk-ai/rtk, MIT): shrinks shell output (git/ls/find) before the model sees it. Real saving is ~10 to 30% on mixed work; it can truncate an error trace (`rtk proxy <cmd>` gives raw). Needs the Rust toolchain or its install script; Windows support is unconfirmed. Not for CI.
