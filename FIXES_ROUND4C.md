# Fixes round 4C: screen buddy (10 Nov, revised to `docs/GROOVY_SPEC.md`)

Read `docs/GROOVY_SPEC.md` (sections 5, 8, 10) and `docs/plan/ARCHITECTURE_MAP.md` sections 2.4, 2.5, 2.11 first. One commit for the round with `[build]`. **Needs round 4D items 1, 2 and 4** (the `Brain` interface, the switcher, tools on subscription brains).

**Today:** nothing of this exists. The nearest piece is the desktop Groovy window (`src-tauri/src/desktop.rs`): transparent, click-through except over his body, made only while he is out.

**Target:** a live talk-back conversation about the screen. He looks only when asked, explains, and draws on the screen.

**References, read for behaviour only (both MIT):** `Rikinshah787/dotpals` and `Louis-CFM/coucou`. Copy no asset, no art, no sound, no text and no code. Do not name them anywhere else in the repo; this section is deleted when round 4E closes.

**Limits for every item:** a capture happens only when the user asks or approves; it stays in memory, is never written to disk or a log, goes only to the brain answering, and is dropped after the reply. No new WebView: drawing reuses the desktop Groovy window. Nothing of the screen buddy loads until first use, so idle stays **under 150 MB**.

## Item 1: Behaviour notes from the references (Opus; no code)
**Files:** new `docs/plan/SCREEN_BUDDY_NOTES.md` (behaviour in Groovy's own words; no names, no copied text).
**Steps:**
1. Attach the two reference repositories read-only. Note how a screen conversation starts, when a capture is taken, what is sent, how the reply is spoken, how marks are drawn and cleared.
2. Write the behaviour Groovy will have, in a table against items 2–6. Where items 2–6 fall short of the references, edit them in this round's commit.
**Done when:** the notes are committed and items 2–6 match them. If the repositories cannot be attached, say so in `STATUS.md` and build items 2–6 as written.

## Item 2: Screen capture, on request only
**Files:** new `src-tauri/src/screen.rs`, `platform/windows.rs`, `portal.rs` (Linux: the desktop portal's Screenshot call), `Cargo.toml` (`windows` features; any new crate must be MIT, BSD, ISC or Apache-2.0 and pass `deny.toml`), `settings.rs`.
**Steps:**
1. Windows: the monitor under the pointer (`docs/plan/SCREEN_BUDDY_NOTES.md`). GDI (`BitBlt` from the monitor's DC) for a monitor; `PrintWindow` or a cropped monitor capture for the active window. Groovy's own windows are left out (`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`).
2. Linux: the portal's Screenshot request. Where there is none, return a clear error.
3. Downscale so the long side is at most 1568 px **and** the picture at most 1.15 megapixels, JPEG quality 80, in Rust. Return bytes plus the scale, the origin and where the pointer is in the picture. Free the full bitmap before returning.
4. Setting `screenLook`: `ask` (default: an Allow button in the island for each look), `allowed`, `off`. An eye badge shows on the island for 2 s after every capture.
5. Game Mode and a locked screen refuse.
**Test:** unit tests for scaling and the setting gate; a Windows CI step captures the corner test's magenta desktop and checks the colour.
**Done when:** a 4K monitor capture is **at most 400 KB**, is ready **within 500 ms (P11)**, and `groovy.exe` is back to its earlier memory within 2 s.

## Item 3: "Look at my screen" in chat and by voice
**Files:** `src-tauri/src/okgroovy/brain.rs` (`tools()`), `tools.rs`, `chat.rs`, `brains/` (round 4D), `src/views/chat.ts`, `src/island/okgroovy.ts`.
**Steps:**
1. Tool `look_at_screen {target: "screen" | "window"}`. It reads outside (`reads_outside`), so approval is decided in Rust from `screenLook`, never by the page.
2. The result is an image block, sent only to a brain whose capabilities include `vision`. The switcher moves to the next brain that has it; if none, he says which brain can see.
3. Chat: a "Look at my screen" button adds one capture to the next message, with a thumbnail and a remove button.
4. The image is untrusted content: text in a screenshot cannot trigger an action without approval (reuse `as_data`).
5. After the reply the image is dropped from the history; the text stays.
6. Asked about the screen with no picture (refused, `off`, a locked screen): he says he can't see the screen right now and never guesses. Speech that isn't meant for him (a video, half a sentence) gets no reply.
**Test:** approval required in `ask`; no image goes to a brain without `vision`; no image in the history after the turn.
**Done when:** "Hey Groovy, what's this error on my screen?" is answered about the visible window with the eye badge shown, on a key brain **and** on a signed-in subscription brain.

## Item 4: Live talk-back about the screen
**Files:** `src-tauri/src/calls.rs`, `src/island/voice.ts`, `src/voice/instant.ts`.
**Steps:**
1. In a call, `look_at_screen` works as in item 3: one capture per question that needs one. No stream. An explanation may come in **steps** (up to 8): each is one or two spoken sentences with the marks that belong to it, drawn as he says them (item 5).
2. Real-time voice models: when the model asks to look, one frame goes through the same Rust path.
3. Switch "Keep watching during a call" (off by default): at most one frame every 5 s, only while the call is active and only when the screen changed (compare a 32×32 thumbnail). The eye badge stays lit.
4. Add "a call with two looks" to the peak test.
**Test:** call state-machine page tests with a mocked look; the peak test.
**Done when:** the call with two looks peaks **under 300 MB**, both answers refer to the screen, and the first sound of each answer keeps to **P7** plus the capture's 500 ms.

## Item 5: Pointing and drawing on the screen (Opus)
**Files:** `src-tauri/src/desktop.rs`, `platform/windows.rs`, `src/desktop/main.ts`, new `src/desktop/draw.ts`, `okgroovy/brain.rs`.
**Steps:**
1. While drawing, the desktop Groovy window covers the monitor, fully click-through; it shrinks back when the marks clear. Linux: the existing layer overlay mode; where the desktop Groovy is unavailable, he says he can't draw there.
2. Tool `draw_on_screen {marks: [{shape, x, y, w, h, text, label, color}], steps, keep, clear}` in the capture's pixel space. Shapes: circle, box, underline, arrow (its w and h say where it ends and may be negative), highlight, write (short handwriting, at most 40 characters, `h` the letters' height, placed in a blank area). Colours: red, green, yellow, blue. Every mark is clamped to the picture; one that can't be placed is dropped and he says so. Rust maps to physical pixels with item 2's scale and origin, per monitor DPI. Marks are data only: canvas shapes and `fillText`, no HTML from a brain.
3. He flies to the mark and points with his wisp. Marks draw in with the motion module and clear after 6 s, on Escape, a click, or "clear that"; `keep` holds them until the next request.
5. **Circle and ask:** a shortcut lets the user draw around something; the next question is asked with a capture that has their circle on it.
4. The canvas loop runs only while marks show.
**Test:** mapping tests at 100, 150 and 200 % and on a second monitor; a CI screenshot with an arrow and a box at known places.
**Done when:** marks land **within 10 px** in the CI screenshot, frame time while drawing is **p95 at most 20 ms (P10)**, the window shrinks back, and drawing during a call stays **under 300 MB**.

## Item 6: Settings and privacy
**Files:** a "Screen" card in Settings, `settings.rs`, i18n, `docs/PRIVACY.md` (round 4E item 6).
**Steps:** `screenLook`, the keep-watching switch, the brains that can see, and one sentence on where a capture goes.
**Done when:** `settings-keys.test.mjs` passes, ten languages, and Settings-open idle is still **under 150 MB**.

## Item 7: Checks
- Idle **under 150 MB** (unchanged), peak **under 300 MB** with looks and drawing. "Ready for the owner": ask about an error, ask where to click, ask during a call.
