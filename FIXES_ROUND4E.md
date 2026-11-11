# Fixes round 4E: licence and asset audit (10 Nov, revised to `docs/GROOVY_SPEC.md`)

Read `docs/GROOVY_SPEC.md` (sections 2, 9, 10) first. One commit for the round with `[build]`. **Not legal advice.**

**Position:** Apache-2.0, confirmed by the owner. Only MIT, BSD, ISC and Apache-2.0 code may be added; no GPL, AGPL or LGPL (`windows/deny.toml` already fails the build on them). Releases made before 4 Nov 2026 stay under the licence they were published with; that sentence stays in `README.md`.

**The names to remove** are listed once, here, so they can be searched for: `coucou`, `mochi`, `dotpals`, `blu` (as a character name), `NotchBuddy`. This list and round 4C's reference lines are deleted in item 2's last step.

**What can and can't be done at once:** Groovy began from an MIT-licensed code base, so a large share of files still contain copied code. The MIT licence requires its copyright line and text to stay with that code. So the names leave every file **except `THIRD_PARTY_NOTICES.md`**, which carries the credit until each copied file has been rewritten (item 3 keeps the list).

## Item 1: Original motion for the character (Opus)
**Finding:** `windows/src/mochi/engine.ts` says in its header that it is a direct port of another project's animation engine (same constants, tweens, easings, particles). The outline is already Groovy's own (`pillow.ts`); the motion is not.
**State (11 Nov):** steps 1 and 2 are done in `docs/plan/CHARACTER_MOTION.md` (the method-by-method table, and the design). Steps 3 and 4 wait for the owner's approval of the design.
**Goal:** every tween, easing, cycle, emote and particle is Groovy's own. **The growth art, the looks and the pillow outline don't change.**
**Files:** `windows/src/mochi/engine.ts`, `skins.ts` (eye and mouth drawing), `core/layout.ts` (state and emote names), `core/anim.ts`, `docs/plan/CHARACTER_MOTION.md`.
**Steps:**
1. List every state, emote, timing constant, easing and particle in `engine.ts` in `CHARACTER_MOTION.md`, each marked **ported** or **Groovy's own**. Read the original only to mark the list.
2. Design the replacement on paper first, in the same file: Night's sharp glowing eyes and halo pulse; a blink, breathing and idle cycle with different rhythm and shape; gestures made with his wisps (leaf flick, flame lick, droplet bob); look-specific particles. Contact sheet (`scripts/body-sheet.mjs`) for the owner.
3. Rewrite each **ported** row from the design, not from the old code. Keep the state names the island calls, so nothing else changes. Rewrite the file's header.
4. Loops still stop at rest (idle CPU) and springs still snap to 0.
**Test:** `body-sheet.mjs`, `items-check.mjs`, wardrobe and growth tests; frame test.
**Done when:** `CHARACTER_MOTION.md` has **no "ported" row**, frame time is **p95 at most 20 ms (P10)**, idle CPU **at most 0.3 % (P1)**, idle **under 150 MB**.

## Item 2: Remove every mention
**Files:** whole repo. Code: `windows/src/mochi/` → `windows/src/groovy/`, `windows/mochi.html` → `desktop.html`, commands `desktop_mochi_*` (`src-tauri/src/lib.rs:1483`, `desktop.rs`), settings key `desktop_mochi`, comments in about 34 files, `portal.rs:666` test strings. Docs: `README.md`, `NOTICE`, `LICENSE-ASSETS.md`, `LICENSE-COUCOU-MIT`, `HANDOVER.md`, `STATUS.md`, `CHANGELOG.md`, `WORK_QUEUE.md`, `FIXES_ROUND*.md`, `docs/**`, `windows/README.md`, `windows/docs/**`, `linux/arch/*`, workflow files.
**Steps:**
1. Code: `git mv` the folder and page, fix imports, rename the commands (`node scripts/command-access.mjs`), reword comments to say what the code does.
2. Settings key: read `desktop_mochi`, write `desktopGroovy`, keep reading the old key for one version (`#[serde(alias)]`); add a fixture test that the value survives. The alias is the one allowed leftover in code; remove it in the next release.
3. Phone link wire names (`_coucou._tcp`, `coucou://pair`, `coucou.v1` in `phone_link/{discovery,pairing,relay_client}.rs`): add Groovy-named equivalents and advertise both. Move the old ones into one file, `phone_link/compat.rs`, removed when the Android app is updated (**blocked on the Android source**).
4. Docs: `README.md` and `NOTICE` describe Groovy on its own; third-party credit is one line pointing to `THIRD_PARTY_NOTICES.md`. Fold `LICENSE-COUCOU-MIT` and `LICENSE-PHONELINK-MIT` into `THIRD_PARTY_NOTICES.md` (copyright line plus MIT text each) and delete the two files. Historical files (`HANDOVER.md`, old `FIXES_ROUND*.md`, `STATUS.md`, `CHANGELOG.md`, `docs/research`, `docs/LICENSING.md`): reword the mentions ("the original project", "the reference app") rather than deleting history.
5. Last: delete the name list at the top of this file and the reference lines in `FIXES_ROUND4C.md`.
**Test:** all tests; an old settings file loads; a new CI step fails the build if `grep -rIil` for the names finds any file other than `THIRD_PARTY_NOTICES.md`, `phone_link/compat.rs` and the settings alias line.
**Done when:** that CI step is green.

## Item 3: The copied-code ledger
**Goal:** know exactly which files still hold copied code, so the credit can be dropped when the list is empty.
**Files:** new `docs/plan/REWRITE_LEDGER.md`, `windows/THIRD_PARTY_NOTICES.md`.
**Steps:**
1. For each source file, from `git log --follow --diff-filter=A` and the first commit of the repo: **inherited** (existed at the start), **rewritten** (inherited, later replaced wholesale), **Groovy's own** (added later). One line per file, grouped by folder.
2. Rank the inherited files by how visible they are (character and island first, plumbing last) and give each a round to be rewritten in. Don't rewrite plumbing in this round.
3. `THIRD_PARTY_NOTICES.md` gets one section, "Code Groovy started from", with the copyright line, the MIT text, and "see `docs/plan/REWRITE_LEDGER.md`".
**Done when:** every file under `windows/src`, `windows/src-tauri/src` and `windows/hook` has a ledger line, and the character's files (`windows/src/groovy/*`) are all **rewritten** or **Groovy's own**.

## Item 4: Asset inventory
**Files:** new `docs/ASSETS.md`.
**Steps:** one row per shipped non-code file: origin, author, how to regenerate. Replace anything without a provable Groovy origin.
| Asset | Path | Check |
|---|---|---|
| App and tray icons | `windows/src-tauri/icons/*` | `git log --follow`; drawn for Groovy, or redraw |
| Sounds | made by `windows/src/core/sounds-synth.ts` | no audio file ships (`windows/sounds/SOURCES.md`) |
| Growth and wardrobe art | `windows/src/groovy/art/*` from `docs/design/wardrobe-v2` | every board is the owner's; **art unchanged** |
| Hatching seed | `windows/src/groovy/seed.ts` | nothing of an older intro left |
| Fonts | `windows/src/hub/fonts/manrope-*` | SIL OFL, `OFL.txt` beside them |
| Test audio | `windows/src-tauri/tests/audio/*.wav` | synthetic; not shipped (`pack.mjs`) |
| Strings | `windows/src/i18n/*.json` | no reserved name in visible text |
**Done when:** every non-code file from `git ls-files` has a row and none says "unknown".

## Item 5: Notices ship, and only allowed licences come in
**Files:** `windows/THIRD_PARTY_NOTICES.md`, `windows/scripts/third-party.mjs`, `windows/scripts/pack.mjs`, `windows/src-tauri/tauri.conf.json`, `windows/docs/licenses/MODELS.md`, `windows/deny.toml`, `/.github/workflows/check.yml`.
**Steps:**
1. `deny.toml` and the npm licence gate allow exactly MIT, BSD-2/3-Clause, ISC, Apache-2.0 and the small set already justified (Unicode, Zlib, MPL only if already present: list each with its reason). Anything else fails.
2. CI fails when `THIRD_PARTY_NOTICES.md` is out of date.
3. Setup, MSI and Portable each contain `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, `MODELS.md` (a CI step lists the installed folder).
4. Model credits visible in Settings → About (Parakeet is CC-BY-4.0; plus Whisper, the light voice, Kokoro, the keyword spotter).
**Done when:** the three CI steps are green and a fresh install shows the credits.

## Item 6: Close `docs/LICENSING.md`
**Steps:** rewrite it to the spec's position (short). Each open item gets an owner and a next step: trademark search for "Groovy"; `docs/PRIVACY.md` drafted (what stays on the PC, what goes to which brain, screen captures); `.github/pull_request_template.md` with a Developer Certificate of Origin line.
**Done when:** no open item lacks an owner and a next step.

## Item 7: Checks
- `cargo deny check`, npm licence gate, gitleaks and the name check green. Idle **under 150 MB**, peak **under 300 MB**. `STATUS.md`: what was rewritten, what remains in the ledger, what waits on the Android source.
