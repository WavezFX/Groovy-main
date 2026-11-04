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

## Session B: Sonnet 5.5 (3L wardrobe fixes first, then the rest, cheaper)

```
Pull groovy-ai-phase1 and read WORK_QUEUE.md. Work in this order: round 3L (FIXES_ROUND3L.md, the wardrobe fit fixes, one [build] at its end), then round 3D item 11 (FIXES_ROUND3D.md), then 3I (FIXES_ROUND3I.md), then 3J (FIXES_ROUND3J.md). Same rules and the same token rules as Session A. One [build] commit per round at most. Stop after 3J. Memory budget is now 150 MB idle / 300 MB peak (WORK_QUEUE.md).
```

## Optional token savers (owner's call; test first, keep only if it clearly helps)
- `npx ccusage` (offline, reads local logs): shows where tokens go per session. Measuring only.
- `rtk` (github.com/rtk-ai/rtk, MIT): shrinks shell output (git/ls/find) before the model sees it. Real saving is ~10 to 30% on mixed work; it can truncate an error trace (`rtk proxy <cmd>` gives raw). Needs the Rust toolchain or its install script; Windows support is unconfirmed. Not for CI.
