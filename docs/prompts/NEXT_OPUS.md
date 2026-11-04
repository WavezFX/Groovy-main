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

## Optional token savers (owner's call; test first, keep only if it clearly helps)
- `npx ccusage` (offline, reads local logs): shows where tokens go per session. Measuring only.
- `rtk` (github.com/rtk-ai/rtk, MIT): shrinks shell output (git/ls/find) before the model sees it. Real saving is ~10 to 30% on mixed work; it can truncate an error trace (`rtk proxy <cmd>` gives raw). Needs the Rust toolchain or its install script; Windows support is unconfirmed. Not for CI.
