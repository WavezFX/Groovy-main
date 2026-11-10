# Prompt for the Sonnet build session: rounds 4A–4E

Paste everything in the block.

```
Pull groovy-ai-phase1 from WavezFX/Groovy-main, merge fable-plan into it if it
isn't there yet, and work on groovy-ai-phase1.

Read, in this order and nothing else up front:
1. docs/GROOVY_SPEC.md   (what Groovy must be; it wins over every other file)
2. graphify-out/GRAPH_REPORT.md   (run "graphify update ." if it is missing)
3. WORK_QUEUE.md section 4
4. The round file you are on (FIXES_ROUND4A.md first). Open
   docs/plan/ARCHITECTURE_MAP.md only for the section the item names.

Order: 4A, 4D, 4B, 4C, 4E. Inside a round follow the queue's order.

Commits: ONE commit per round, every finished item ticked in WORK_QUEUE.md in
it, [build] in its subject ("Round 4A: ... [build]"). Read the Windows run's
summary. If a number misses a "done when" line, make a fix commit, also with
[build]. Don't start the next round on a red build.

Skip every item marked (Opus) and every item blocked on the owner: leave it
unticked, write "left for Opus" or what is missing in STATUS.md, carry on.

Hard limits (private working set, all Groovy processes):
- idle under 150 MB, with Settings open too
- never above 300 MB, in a voice call and with Settings open too
Measured on Windows CI only. Write before and after in STATUS.md. Never lower
a gate blind, never raise one.

Rules:
- Wake words are only "Groovy", "Hey Groovy", "Yo Groovy", in one listener.
- Speech-to-text and his voice run on the PC by default.
- Brains: cloud and subscription first, local last. Keep switching.
- Keep the growth art exactly as it is. Keep all Linux code. Studio is out.
- Add only MIT, BSD, ISC or Apache-2.0 code. All art and sound is Groovy's own.
- Don't write the names of other companion projects or their characters
  anywhere.
- New Tauri command: node scripts/command-access.mjs. New visible text: ten
  languages. New setting: tests/settings-keys.test.mjs. Never commit keys,
  CLAUDE.md, .claude/ or graphify-out/.
- Save tokens: find code with grep or the graph, read by line range
  (island.ts, lib.rs, settings/main.ts, spotify.rs are over 1500 lines), cap
  command output.

Don't tick an item until its "Done when" line is true; say in STATUS.md how
you checked. If a measurement says a change isn't worth it, write the numbers
and tick it "measured, not changed".

Stop when section 4 has only (Opus) and owner-blocked items left. Write
"ready for the owner" in STATUS.md: what to test, what is waiting.
```

## Items for an Opus or Fable session
4A item 3 · 4D items 1 and 4 · 4B item 1 · 4C items 1 and 5 · 4E item 1.
Sonnet can't start 4D item 2 before 4D item 1, or 4C before 4D items 1, 2 and 4, so run Opus on 4D item 1 first.

## Waiting on the owner
- Approve the PROPOSED targets P1–P11 in `docs/GROOVY_SPEC.md` section 8.
- Confirm that waking to his given name is removed (spec section 4).
- The Android app's source (smooth phone link; renaming the link's wire names).
