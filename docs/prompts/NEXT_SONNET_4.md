# Prompt for the Sonnet build session: rounds 4A–4E

Paste everything in the block.

```
Pull groovy-ai-phase1 from WavezFX/Groovy-main, merge fable-plan into it if it
isn't there yet, and work on groovy-ai-phase1.

Read, in this order and nothing else up front:
1. WORK_QUEUE.md (the rules at the top, then section 4)
2. docs/plan/ARCHITECTURE_MAP.md
3. The round file of the item you are on (FIXES_ROUND4A.md first)

Do section 4 in the order written: 4A, 4D, 4B, 4C, 4E. Inside a round, follow
the queue's order, not the item numbers.

Rules:
- One item per commit, ticked in WORK_QUEUE.md in the same commit, pushed.
  Subject: "Round 4A, item 1: ...". Fix a red build before the next item.
- Skip every item marked (Opus). Leave it unticked, note "left for Opus" in
  STATUS.md, and carry on. Skip an item that waits on the owner the same way.
- Memory: under 150 MB idle, never over 300 MB peak, all Groovy processes.
  Memory is measured only on Windows CI: put [build] in the subject of a
  commit that changes code, read the run summary, and write before and after
  in STATUS.md. Never lower a gate blind and never raise one.
- Don't remove features. Keep brain switching and all Linux code. Port Coucou
  and dotpals behaviour as it is upstream; cloud and subscription brains stay
  first, local last. Studio Edition is out of scope.
- All art, sounds and wardrobe items are Groovy's own. Add nothing from Coucou.
- New Tauri command: node scripts/command-access.mjs. New visible text: all
  ten languages (tests/i18n.test.mjs). New setting: tests/settings-keys.test.mjs.
- Never commit keys. Don't commit CLAUDE.md, .claude/ or graphify-out/.
- Save tokens: find code with grep or graphify-out/GRAPH_REPORT.md, read by
  line range (island.ts, lib.rs, settings/main.ts and spotify.rs are over
  1500 lines), cap command output.

Each item has a "Done when" line: don't tick it until that line is true, and
say in STATUS.md how you checked. If an item's measurement says the change
isn't worth it, write the numbers, tick it as "measured, not changed", move on.

Stop when section 4 has only (Opus) and owner-blocked items left. Then write
"ready for the owner" in STATUS.md with what to test and what is waiting.
```

## What the owner still has to supply
- The dotpals repository (round 4C item 1).
- Upstream Coucou at the forked commit (round 4E item 1), and a choice: permission from Louis Raillé or a redo of the animation.
- Confirmation that Apache-2.0 stays (round 4E).
- Rhytam Biswas's original Phone Link licence file, and the Android app's source.
