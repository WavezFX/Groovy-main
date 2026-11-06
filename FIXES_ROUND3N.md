# Fixes round 3N: finish the leftovers (4 Nov, late)

Everything still open, in the order to do it. The rules are the same as every round:
- One item per commit, ticked in `WORK_QUEUE.md` in the same commit. Push after each and keep the build green.
- Don't remove any user-facing feature, and keep all Linux code.
- Memory budget is under **150 MB idle and 300 MB peak**, with near-0 CPU at idle.
- New Tauri commands mean running `node scripts/command-access.mjs`. New visible text gets translated into all ten languages (`tests/i18n.test.mjs`).
- Before pushing outfit work, run `scripts/items-check.mjs` and the wardrobe tests.

## Item 1: The rest of the hats are worn, not pasted (owner: "looking better", 4 Nov 23:09)
`e5115fa` made the acorn, pumpkin and mushroom real caps: `CAP`, `CAP_FORM`, `capPath`, `capPoint` and `drawCap` in `windows/src/mochi/outfits.ts`. Each is a dome shaped from his own head, its edge following the front of a ring round him, with details lying on its surface. Nothing is drawn behind it and there's no fill under it.
- Do the same for **bucketHat, bobbleBeanie, eggshellHat, watermelonCap, backwardsCap and strawSunHat**. Add each to `CAP` with its own `CAP_FORM` and details:
  - **Bucket hat:** a short dome, with a brim drawn as a wider ring below the edge. The brim's front is drawn over him and its back is behind him, clipped to outside his outline.
  - **Beanie:** a knit dome with ribs on meridians, a folded cuff band, and a pompom on top.
  - **Eggshell:** a cracked-shell top with a zigzag edge.
  - **Watermelon:** a rind band at the edge with seeds on its surface.
  - **Backwards cap:** a dome with a peak at the **back**. The peak shows past his outline and turns with him.
  - **Straw sun hat:** a low crown with a wide brim, done the same way as the bucket hat.
- Remove these hats' old board-art path (`DOME`/`drawDome`) once all of them are done. Keep the wardrobe icons as they are.
- Also look at the beret, top hat, antlers, bunny ears and bat-wing band. They must sit **on** his crown and follow his turn, never floating.
- Add every hat to the CI contact sheet and the motion strips.

## Item 2: Peak memory down to 300 MB (round 3D item 11, the part left)
Measured in build #216 (see `STATUS.md`): idle is 108–112 MB, which is fine. The peak is **330 MB**: groovy.exe is about 198 MB at the peak, because Kokoro and sherpa are loaded with a call, plus WebView2 at about 132 MB.
1. Move the native speech engines (Kokoro TTS, sherpa-onnx STT/wake) into **on-demand helper processes**. They start when needed, talk over a pipe, and exit after being idle for a short while. The main process must not keep their models loaded.
2. Re-measure from the next build's smoke test and peak test (Task Manager private working set, all Groovy processes). Write the before and after numbers in `STATUS.md`.
3. Only after measuring, set the peak gate (now 360) and the idle gate (now 130) to the measured numbers plus a small margin, tightening toward 300 and 150. Never lower a gate blind and never turn a build red.
4. No feature removed. Speech, wake word and calls must work exactly as before (the peak test checks them).

## Item 3: Clipboard history island tile (3J item 4, the missing part)
The backend and the Settings list exist behind the clipboard switch. Add the island tile: latest few items, tap to copy back, clear button. It's opt-in, memory only, and shown only when the switch is on. Nothing runs when it's off.

## Item 4: Dialog Jump (3J item 6, optional)
Only if items 1 to 3 are done and green. If it needs Windows UI Automation that can't be checked in CI, build it behind its setting (off by default), say so in `STATUS.md`, and move on.

## Item 5: Checks
- Contact sheet and motion strips with every hat, and the memory numbers in the build summary.
- One `[build]` commit at the end. Confirm the Windows build job **ran** (not skipped) and that the "Groovy latest build" release updated.
- Write "ready for the owner" in `STATUS.md`, with what changed.
