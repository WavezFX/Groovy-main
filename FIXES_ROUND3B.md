# Fixes round 3B: the owner tested the round 3 build

**Do these now, before continuing round 4.** If a round 4 item is half done, finish that commit first. Commit each item separately with "Fixes round 3B, item N: …" as the subject. These are round 3 items that don't work on the owner's PC yet (Windows 11 laptop).

## Item 1: The grey square behind the island (round 3 items 2 and 8)

The island still sits on a **grey rectangular block**: the window's corners show grey instead of the desktop. Round 3 item 8 added `glass.rs::set_region` (`CreateRoundRectRgn`), but on Windows 11 `Effect::Acrylic` is the DWM system backdrop (`DWMWA_SYSTEMBACKDROP_TYPE`), which fills the **whole window rectangle** and ignores the window region and the page's transparency.

Fix it so that nothing outside the island's rounded shape is ever drawn, in every state (F, E, the music tile, the detail card, quick settings, and during resizes):

1. Stop using the DWM system backdrop for the island. Use the legacy accent blur, `SetWindowCompositionAttribute` with `ACCENT_ENABLE_ACRYLICBLURBEHIND` and a gradient colour that has alpha (the `window-vibrancy` crate's `apply_acrylic` / `apply_blur` do this). That one respects `SetWindowRgn` on Windows 10 and 11.
2. Re-apply the rounded region **every time** the island changes size, using the same radius as the CSS (scaled by the monitor DPI). Do it in the same frame as the resize, so a rectangle never flashes during an animation.
3. Check that the WebView2 default background is fully transparent (`DefaultBackgroundColor` alpha 0, `transparent: true`) and that no element outside the island (`html`, `body`, wrappers) has a background.
4. **If the accent blur can't be made clean on some build**, fall back automatically to a CSS glass look: a rounded, semi-transparent dark tint with a soft border, on a fully transparent window. A clean rounded island matters more than real blur. The Glass/Solid setting stays.
5. **Test it in CI.** The smoke test sets a bright solid wallpaper colour, shows the island, takes a screenshot of its window rectangle (`CopyFromScreen`), and fails if any **corner** pixel isn't the wallpaper colour. Do this for the F layout and the E (music) layout.

## Item 2: Music layout: smoother, and back to normal quicker (round 3 item 2)

When Spotify starts, the island grows to the E layout, but after the music stops it takes too long to shrink back, and the change isn't smooth.

- `PLAYER_HOLD_MS` (`core/home.ts:14`) is 5000. Change it to **1500**. That's still long enough that skipping a track doesn't make it jump.
- **React to events, not polling:** use the Windows media session's `PlaybackInfoChanged` (GSMTC) for Spotify's play/pause, so it reacts as soon as the music changes.
- **One smooth animation each way** (about 280 ms, ease-out):
  - grow the window to the target size **first**, invisibly behind the transparent region;
  - animate the island's shape and contents in CSS;
  - shrink the window only **after** the shrink animation ends.
- No size jumps, no layout jumping mid-animation, no flicker. Keep it under 60 fps CPU cost and pause it when hidden (round 3 item 9).
- Apply the same smoothing to every island change: opening and closing the detail card and quick settings, and the voice states.

## Item 3: Night must look green (round 3 item 6)

Ember looks orange and Frost looks blue, but **Night looks plain white**. Its green is only in the rim glow and eyes, and those wash out on light backgrounds and at night. Night is the main default look, so its green has to be obvious.

- Give Night a **soft green body tint and glow**, as clear as Ember's warmth and Frost's coolness. For example, `bodyTop` around [0.93, 0.99, 0.94], `bodyBottom` around [0.70, 0.90, 0.76], and a stronger rim glow. Tune it by eye next to the other two looks so all three are equally coloured.
- Find and remove anything that **recolours the character** by time of day, the Windows theme or dark mode, Night light, the accent colour, or an island tile's colour. The three default looks must always show their own colours. Only the user's own body-colour setting may change them.

## Item 4: Choosing a look doesn't stick, and the wardrobe lags (round 3 items 5 and 6)

When the owner switched between looks, **Ember stayed** and the wardrobe stopped responding when things got laggy.

- `island.ts:1411` and `desktop.ts:180` draw `State.lookPreview ?? settings.groovyLook`. A preview that's never cleared (the wardrobe closed mid-hover, or a lost event) pins the old look forever. **Clear `lookPreview`** whenever the wardrobe closes, loses focus or a look is chosen, and on a timer as a safety net.
- **One source of truth.** A click applies at once in the island (optimistic), saves once (debounced at 300 ms), and every window listens for the same `settings-changed` event. A slower save must never overwrite a newer choice. Use a version or sequence number.
- **Speed up the wardrobe:** render previews from cached sprites instead of a full engine per tile, and keep at most one live engine (the selected item).
- **Test:** switch Night → Ember → Frost → Night 20 times quickly. The final look must be Night everywhere, after a restart too.

## Item 5: No stage picking

The owner doesn't like picking the stage himself. Groovy should **always show the stage he has actually grown to**.

- Remove the "Show stage" select (`settings/wardrobe.ts:145–155`) and the `shown` override. Migrate saved `shown` values to `null`.
- Keep the progress line ("Stage 2 of 5 · next in about 3 days"). Show the stages he's already passed as small read-only pictures, but don't let them change his look.
- The new stage art (and stage 6) comes in round 5. Don't redraw it here.

## Item 6: Dictation gets names and models wrong

**The problem:** "Asus VivoBook S14" came out as "A book S fourteen". **Don't fix that phrase on its own.** Dictation has to get names, brands, products, model numbers and technical words right *in general*.

1. **Cloud first, like the brains.** Add cloud speech-to-text as the first choice when the user has a key or free tier, with Parakeet and Whisper as the offline fallback. Candidates include OpenAI `gpt-4o-transcribe`, Deepgram Nova-3 (keyterm prompting), ElevenLabs Scribe, and Gemini audio transcription on the free tier. Use the same Auto order and limit-based switching as `brain_ai.rs`, and add it to quick settings' voice status.
2. **Personal vocabulary, learned automatically.**
   - When the user corrects dictated text within about 60 s (the text Flow inserted changes in the same field), diff it and remember the corrected words.
   - The user can also add words in Settings → Voice.
   - Feed this vocabulary to **every** engine: cloud keyterms or prompts, Whisper's `initial_prompt` (today it's only "Hey Groovy.", `voice.rs:240`), and sherpa-onnx **hotwords** for Parakeet (needs `modified_beam_search`; check that it works with the TDT model, and if it doesn't, rely on step 3 for Parakeet).
3. **Smarter clean-up** (`flow/cleanup.rs`). Besides removing the "um"s, the clean-up brain also gets:
   - the personal vocabulary;
   - the **active window's title and app name** (for example, a shop page about laptops);
   - an instruction to fix misheard brand, product and model names to the correct spelling, and to write model numbers as digits ("S fourteen" → "S14").
   It must never change the meaning or add words. If no brain is available, fall back to vocabulary-only replacement.
4. **Number formatting offline too:** convert spoken numbers to digits when they're next to letters or model-like words ("S fourteen", "RTX forty sixty", "iPhone fourteen pro" → "S14", "RTX 4060", "iPhone 14 Pro").
5. **Accuracy test set:** at least 40 short recorded or synthesised (Kokoro) clips of product names, brands, model numbers, mixed accents and everyday sentences, with expected text. Report the word error rate per engine in CI, and fail if the default pipeline's proper-noun accuracy gets worse.

## Item 7: Checks

- The Rust and TS tests stay green on Windows CI.
- The idle smoke test stays **under 300 MB**.
- The new corner-pixel test passes.
