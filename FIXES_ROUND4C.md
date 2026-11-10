# Fixes round 4C: screen buddy (10 Nov)

Read `docs/plan/ARCHITECTURE_MAP.md` sections 2.4, 2.5 and 2.11 first. Same rules as round 4A.

**Today:** nothing of this exists. No screen capture, no screen image sent to a brain, no drawing on screen. The nearest piece is the desktop Groovy window (`src-tauri/src/desktop.rs`, `mochi.html`): transparent, click-through except over his body, made only while he is out.

**Target:** like dotpals' Blu. The user talks to Groovy about what is on screen: he looks when asked, explains, and points or draws on the screen.

**Blocker:** the dotpals source was not available when this plan was written, so the behaviour below is a best reading, not a checked port. Item 1 fixes that before any code.

**Hard limits for every item:** a screenshot is taken only when the user asks or approves, is held in memory only, is never written to disk or logs, and is dropped after the reply. No new WebView: the overlay reuses the desktop Groovy window (one extra WebView at most). Round 4D's provider interface must be in place first (vision goes through it).

## Item 1: Parity table from dotpals (Opus; no code)
**Goal:** a checked list of what Blu does, so the port is faithful.
**Files:** new `docs/plan/SCREEN_BUDDY_PARITY.md`.
**Steps:**
1. Get the dotpals repository from the owner (link or `add_repo`). Read how Blu captures, when it captures, what it sends, how it talks back, and how it draws.
2. Write a table: Blu feature, how dotpals does it, Groovy's equivalent in items 2–6, differences forced by Windows or by the memory budget.
3. Where this file's items disagree with dotpals, **dotpals wins**: edit items 2–6 in the same commit and say what changed.
**Done when:** the table is committed and items 2–6 match it. If the owner cannot supply the source, write that in `STATUS.md` and build items 2–6 as written.

## Item 2: Screen capture, on request only
**Goal:** one Rust function returns a JPEG of the screen, a monitor, or the active window.
**Files:** new `src-tauri/src/screen.rs`, `platform/windows.rs`, `portal.rs` (Linux: the xdg-desktop-portal Screenshot call), `Cargo.toml` (`windows` features only; no new crate unless it is MIT or Apache-2.0 and passes `deny.toml`), `settings.rs`.
**Steps:**
1. Windows: capture with GDI (`BitBlt` from the monitor's DC) for a monitor, and `PrintWindow` or a cropped monitor capture for the active window. Skip Groovy's own windows (hide the overlay strokes for the grab, or exclude them with `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`).
2. Linux: the portal's Screenshot request (the user approves in the system dialog). GNOME on Wayland without a portal: return a clear error.
3. Downscale so the long side is at most 1568 px, encode as JPEG quality 80 in Rust, return bytes plus the scale and origin (needed to map a brain's coordinates back to the screen). Free the full-size bitmap before returning.
4. Setting `screenLook`: `ask` (default: every look shows an Allow button in the island), `allowed` (looks when asked by voice or chat, no button), `off`. A small eye badge shows on the island for 2 s whenever a capture happened.
5. Game Mode and a locked screen refuse the capture.
**Test:** Rust unit tests for the scaling maths and the setting gate; a Windows CI step that captures the magenta desktop of the corner test and checks the JPEG's dominant colour.
**Done when:** `screen::capture` returns an image under 400 KB for a 4K monitor, and `groovy.exe` is back to its previous memory within 2 s (logged).

## Item 3: "Look at my screen" for chat and voice
**Goal:** the user asks about what is on screen and gets an answer that used the picture.
**Files:** `src-tauri/src/okgroovy/brain.rs` (`tools()`), `tools.rs`, `chat.rs`, round 4D's provider module, `src/views/chat.ts`, `src/island/okgroovy.ts`.
**Steps:**
1. Add a tool `look_at_screen {target: "screen" | "window"}` to « OK Groovy »'s tools. It is a tool that reads outside (`reads_outside`), so approval is decided in Rust per the `screenLook` setting, never by the page.
2. The tool result is an image block. Send it only to a provider whose capabilities say `vision` (round 4D item 1). If the brain in use has none, the switcher tries the next brain that does; if none, he says "I can't see with the brain I'm using" and names one that can.
3. Chat: a "Look at my screen" button beside the attach button adds one capture to the next message, with a thumbnail and a remove button.
4. The image is treated as untrusted content: text inside a screenshot cannot trigger an action without approval (the 3G item 5 rule; reuse `as_data`).
5. Drop the image from the conversation history after the reply; keep only the text.
**Test:** Rust tests: approval is required in `ask` mode; no image is sent to a provider without `vision`; the history holds no image after the turn.
**Done when:** "Hey Groovy, what's this error on my screen?" produces an answer about the visible window, with the eye badge shown.

## Item 4: Live talk-back about the screen
**Goal:** in a voice call, the user can keep asking about the screen and he keeps answering aloud.
**Files:** `src-tauri/src/calls.rs`, `src/island/voice.ts`, `src/voice/instant.ts`.
**Steps:**
1. Normal call pipeline: `look_at_screen` works mid-call exactly as in item 3. A new capture is taken for each question that needs one; frames are not streamed.
2. Instant voice (OpenAI Realtime, Gemini Live): when the model asks to look, send one frame through the same Rust path. No continuous screen stream by default.
3. Optional switch "Keep watching during a call" (off by default): at most one frame every 5 s, only while the call is active and only when the frame changed (compare a 32×32 thumbnail). The eye badge stays lit while it is on.
4. Memory: a call with looks must stay under the peak gate. Measure it in the peak test (one call with two looks).
**Test:** page tests for the call state machine with a mocked look; the peak test.
**Done when:** the peak test's call with two looks stays under the gate and both answers refer to the screen.

## Item 5: Pointing and drawing on screen (Opus recommended)
**Goal:** Groovy can point at a place on screen and draw simple marks (arrow, circle, box, underline, short label).
**Files:** `src-tauri/src/desktop.rs`, `platform/windows.rs`, `src/desktop/main.ts`, `src/mochi/desktop-logic.ts`, new `src/desktop/draw.ts`, `okgroovy/brain.rs`.
**Steps:**
1. Reuse the desktop Groovy window. While drawing, grow it to cover the monitor, fully click-through (`WS_EX_TRANSPARENT`); shrink it back when the marks clear. On Linux use the existing layer-shell overlay mode (`set_layer_overlay`); where the desktop Groovy is off (GNOME on Wayland), say he can't draw there.
2. Tool `draw_on_screen {marks: [{type, x, y, w, h, text}], seconds}` with coordinates in the screenshot's pixel space. Rust maps them to physical screen pixels with item 2's scale and origin, per monitor DPI. Marks are data only: no HTML from the brain reaches the page (draw on a canvas, text through `fillText`).
3. He flies to the mark (existing flight code) and points with his wisp; marks draw in with the motion module and fade after `seconds` (default 6, max 30) or on Escape, a click, or "clear that".
4. The canvas loop runs only while marks are visible; nothing ticks otherwise.
**Test:** unit tests for the coordinate mapping at 100 %, 150 % and 200 % scale and on a second monitor; a CI screenshot with one arrow and one box at known positions.
**Done when:** "show me where to click" draws a mark within 10 px of the brain's coordinates in the CI screenshot, and the window returns to its small size after the marks clear.

## Item 6: Settings and privacy
**Files:** `src/hub/pages/*` (a "Screen" card on the Assistant or Privacy page), `settings.rs`, i18n.
**Steps:** the `screenLook` setting, the keep-watching switch, the list of brains that can see, and one sentence on where a capture goes (only to the brain answering, never stored). Add the same sentence to the privacy notice when it exists (`docs/LICENSING.md` open item).
**Done when:** `settings-keys.test.mjs` passes and the text is in ten languages.

## Item 7: Checks
- Idle unchanged (nothing of the screen buddy is loaded until first use). Peak under the gate. One `[build]`. "Ready for the owner" with three things to try: ask about an error, ask where to click, ask during a call.
