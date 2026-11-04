# Groovy licensing: what was decided, why, and what is still open (4 Nov 2026)

This is a practical review by Claude, not legal advice and not a solicitor's opinion. It is meant to
get the project onto sensible footing without a solicitor. Nothing here can be guaranteed in court;
the open items at the end are the ones where a mistake could cost something.

## 1. Who owns what
| Part | Owner | Terms |
|---|---|---|
| Groovy's own code (104+ commits by WavezFX, plus work written by Claude at the owner's direction) | SSD / WavezFX | Free to relicense. No outside contributors yet, so no one else's permission is needed. (AI-written code: UK law (CDPA s9(3)) treats the person who arranged its creation as the author; the law here is not fully tested, but the practical risk is low.) |
| Code inherited from Coucou | Louis Raillé | MIT. Can go inside a project under any licence, **if the notice stays** (`LICENSE-COUCOU-MIT`). |
| Phone Link and the Android app | Rhytam Biswas | MIT (`LICENSE-PHONELINK-MIT`, to be checked against his original file). |
| Coucou / Mochi names, the Mochi character, icons, sounds, media | Louis Raillé | **Not MIT.** Reserved. No commercial use, no distribution under those names or with those assets without written permission (`LICENSE-ASSETS.md` in the Coucou repo). |
| Dependencies (Rust crates, npm, models) | many | Each has its own terms. Not yet audited (see open items). |

## 2. The code licence: Apache-2.0
Options considered, against the owner's goals (user growth, scalability, upgrades, plugging in other
projects, a paid Studio):

- **GPL-2.0 (what the repo said):** a random pick. It blocks mixing in Apache-2.0 code (GPL-2.0-only is incompatible with it), forces anything linked in to be open, and makes a paid closed Studio awkward. Rejected.
- **AGPL / GPL-3.0:** strongest protection against closed copies, but worst for plugins, contributors and selling Studio. Rejected.
- **MIT:** simplest and matches Coucou. No patent grant, no explicit trademark clause.
- **Apache-2.0 (chosen):** as permissive as MIT, plus an explicit patent grant, a rule that the licence gives **no trademark rights** (so "Groovy" stays yours), and it is a widely trusted default for projects that expect plug-ins. Compatible with MIT, BSD, Unlicense and Apache code; not with GPL-2.0-only code.

How you stay protected: the code is open, the **brand and character are not** (`LICENSE-ASSETS.md`). A copy has to rename itself and redraw the character, the same model Handy and Coucou use.

Studio: build it as a separate, closed add-on that calls Groovy's open parts. That needs no licence from anyone, and nothing in it may include AGPL/GPL code (it can run such tools as separate programs the user installs).

History: copies already downloaded under GPL-2.0 stay under GPL-2.0 for those versions. The README says so.

Before accepting a pull request from anyone else, add a contributor agreement (or a "Developer Certificate of Origin" sign-off) so the licence can be kept or changed later.

## 3. The bigger risk: the Mochi-derived body
`HANDOVER.md` records that Groovy's body is Mochi-derived and that the owner chose to keep it. Coucou's asset terms reserve the Mochi character's design, look, expressions and animations, and forbid commercial use of them and shipping an app "with" them. A Windows build with a Mochi-derived body, and above all a **paid Studio**, is therefore the most likely thing to cause a legal problem, more than the code licence.

Two free ways out; do one before Studio is sold:
1. **Ask for permission in writing.** The Coucou repo says to email raillelouis@gmail.com or open an issue. Keep the reply. (A short, polite message: what Groovy is, that it will be free, that Studio will be paid, what you would like to keep.)
2. **Redraw the body** so it is clearly original (different silhouette, proportions, face construction), then delete the old art. The wardrobe, growth looks and sounds are already Groovy's own.

Also: the internal code names `mochi` (folder `src/mochi/`, `mochi.html`) are not shown to users, but the Coucou terms list "Mochi" as a reserved name. Renaming them to `groovy` is a safe tidy-up for a quiet moment.

## 4. Open items
- [ ] Dependency licence check in CI (`cargo deny`, npm licence check) and a generated `THIRD_PARTY_NOTICES` shipped with the installer (FIXES_ROUND3J item 1).
- [ ] Model licences (speech, wake word, voice): list each with its terms in `windows/docs/licenses/MODELS.md` (Round 3J item 1). Some require credit.
- [ ] FFmpeg and similar for Studio: use as a separate program and never ship a GPL build inside the installer.
- [ ] Mochi-derived body: permission or redraw (section 3).
- [ ] Trademark: "Groovy" is a common word. A UK trademark search and, if wanted, a registration costs money but can be done by yourself; at minimum search for existing apps called Groovy before spending on branding.
- [ ] Privacy: the app handles mail, speech and screen content. A short privacy notice (what stays local, what goes to which provider) is expected by users and by Google when verifying Gmail/YouTube access.
- [ ] When money starts coming in: a sole trader or limited company, and terms of sale for Studio (refunds, support, no warranty). A consumer-law check for UK sales is worth a proper read of GOV.UK guidance.
