# Prompt for the Sonnet build session: rounds 4A–4F

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

Order: 4A, 4F, 4D, 4B, 4C, 4E. Inside a round follow the queue's order.

Commits: ONE commit per round, every finished item ticked in WORK_QUEUE.md in
it, [build] in its subject ("Round 4A: ... [build]"). Read the Windows run's
summary. If a number misses a "done when" line, make a fix commit, also with
[build]. Don't start the next round on a red build.

Skip every item already ticked, every item still marked (Opus) and every
item blocked on the owner: leave it as it is, write what is missing in
STATUS.md, carry on. The list under this prompt says what is already built.

Hard limits (private working set, all Groovy processes):
- idle under 150 MB, with Settings open too
- never above 300 MB, in a voice call and with Settings open too
Stretch targets S1-S4 (spec section 8) are reported by CI and become gates
once met: idle under 50 MB, normal use and cloud calls under 150 MB.
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

## Already built by the Opus/Fable session (11 Nov): skip these
- **4F item 1** (the island sleeps at idle): built; see `STATUS.md` for what CI measured. Don't rewrite `island_sleep.rs`; the Settings switch "Reduce memory at idle" (today the escape hatch is the `GROOVY_NO_SLEEP` environment variable) is yours to add with 4F item 7.
- **4A item 3** (Settings open idle): built (`hub_idle.rs`, the CI step "Idle with Settings open…"). 4F item 4 goes further if the number is still over 150 MB.
- **4B item 1** (one wake listener, three wake words): built. Waking to his given name was left as it is: remove it only if the owner confirms (spec section 4).
- **4D item 1** (the `Brain` trait and registry, `brains/`) and **4D item 4** (tools on subscription brains, `brains/cli.rs`): built. 4D item 2 now moves `chat.rs` onto `brains::` and deletes `Who`; 4D item 3 adds the Gemini tool as one more `cli::Cli` in the registry and real sign-in detection in `Cli::auth`.
- **4C item 1** (behaviour notes): `docs/plan/SCREEN_BUDDY_NOTES.md`.
- **4E item 1**: steps 1 and 2 only (`docs/plan/CHARACTER_MOTION.md`); the rewrite waits for the owner.

So build, in this order: **4A** items 1, 2, 5, 6, 8 · **4F** items 2 to 10 (all of them are yours now) · **4D** items 2, 3, 5, 6 · **4B** items 2 to 7 (and 4A item 4) · **4C** items 2, 3, 4, 6, 7 · **4E** items 2 to 7.

## Still for an Opus or Fable session
- **4C item 5** (pointing and drawing on screen): not started. It needs 4C items 2 and 3 first (the capture's scale and origin, and the tool path).
- **4E item 1 steps 3 to 5** (the motion rewrite), once the owner approves the design.

## Waiting on the owner
- Approve the motion design in `docs/plan/CHARACTER_MOTION.md` section 2.
- Listen to the voice and speech samples from round 4F item 2 and pick.
- Confirm that waking to his given name is removed (spec section 4).
- Run "Hey Groovy, pause the music" once with only Claude Code or Codex signed in (4D item 4 could not be run against a real sign-in in CI).
- The Android app's source (smooth phone link; renaming the link's wire names).
