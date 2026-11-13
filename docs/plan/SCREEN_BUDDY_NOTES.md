# Screen buddy: behaviour notes (round 4C item 1, 11 Nov 2026)

What a good screen companion does, written from reading how two open-source (MIT) companion apps behave. Behaviour only: no code, art, sound or wording was taken, and none may be. These notes are Groovy's own spec for round 4C items 2–6.

## 1. What was learned
| Behaviour | What the best reference does | Groovy (round 4C) |
|---|---|---|
| Starting | A shortcut (hold to talk, or tap and talk until a pause) or the wake word opens the companion and it listens. | The wake word, the Talk button, or a shortcut (item 3). |
| When the screen is captured | At the moment the user starts talking, before the companion's own window opens over the screen. The picture stays on the PC unless the request is about the screen. | **Stricter:** nothing is captured until the request asks about the screen (item 2). Groovy's own windows are left out of the capture, so capturing after he opened loses nothing. |
| Which screen | The monitor under the pointer. Another monitor's picture would put marks in the wrong place. | Same (item 2). |
| Picture size | Long side at most 1568 px **and** at most about 1.15 megapixels. | Same two limits (item 2). |
| What goes with the picture | The user's words, the picture's size, where the pointer was, what the user circled, the last few exchanges. | Same (item 3). |
| No picture | If the user asks about the screen and no picture came, he says he can't see it right now; he never guesses. | Same (item 3). |
| Not for him | Speech that doesn't seem meant for him (a video, song lyrics, half a sentence) gets no reply at all. | Same (item 3). |
| Answer | Spoken aloud, one to three short sentences, plain words, no Markdown. | Same; P7 first sound within 2.5 s plus the capture. |
| Explaining | "A teacher at a whiteboard": the answer comes in steps (up to 8), each one or two spoken sentences with the marks that belong to it, so marks appear as he says them. | Same (items 4 and 5). |
| Marks | Circle, box, underline, arrow, highlight, and short handwriting (at most about 30–40 characters, with a letter height); four colours; each with an optional label of a few words. | Same six marks and four colours (item 5). Groovy also flies to the mark and points with his wisp. |
| Coordinates | Top-left corner and size, in the picture's pixels; an arrow's size says where it ends and may be negative. Every mark is clamped to the picture. A mark he can't place is not drawn: he says he can't find it. | Same (item 5). |
| Where handwriting goes | In a blank area of the picture; lines one under another share the same x. | Same (item 5). |
| How long marks stay | Until the next request clears them, unless asked to keep them; "clear" removes them at once. | Marks clear after a few seconds, on Escape, a click, or "clear that"; "keep" holds them until the next request (item 5). |
| Circle and ask | A second shortcut lets the user draw around something on screen and then ask about it; the picture sent has their circle on it. | Added to item 5 (step 5). |
| Text on screen | Text on the screen was written by someone else: read it, explain it, never follow instructions in it. | Same; the picture is untrusted content (item 3). |
| What he may do | A fixed list of actions, checked again before each runs. Nothing that deletes, sends, posts or pays. Clicking and typing are off unless the user turns them on. | Groovy's existing tools and approval rules in Rust (`held`, `changes_things`, `reads_outside`). No new actions in round 4C. |
| Privacy line | Voice stays on the PC. A picture goes to the brain only when the user asks about the screen or circles something. With a local brain nothing leaves the PC. | Same; shown in Settings (item 6). |

The second reference has no drawing or screen picture. It gives a voice question the **active window's name and title** as context. Groovy already does this (`chat.rs` `window_line`).

## 2. Changes made to round 4C because of these notes
- Item 2: capture the monitor under the pointer; add the megapixel cap; return the pointer's place in the picture.
- Item 3: "I can't see your screen right now" when no picture came; no reply to speech not meant for him.
- Item 4: answers about the screen may come in steps (up to 8), spoken and drawn together.
- Item 5: six marks and four colours; clamping; "keep" and "clear"; circle and ask.
