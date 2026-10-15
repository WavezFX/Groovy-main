# Groovy fixes, round 2 (owner's second Windows test, 15 Oct 2026, evening)

Same rules as round 1:
- work top to bottom;
- build, test and commit after each item;
- update STATUS.md;
- test on a low-power PC (≈8 GB RAM) with **no API keys** and also with a Gemini key.

## 1. A settings button on the island itself
- Today the settings sit behind the small icon next to the sound button, and the right-click menu from round 1 is hidden too.
- Add a visible **gear button** to the island's top bar, next to the sound button. It opens the Control Centre directly (`Bridge.openSettingsWindow()`).
- Give it a tooltip: "Settings (Ctrl+Alt+,)".
- Keep the in-island quick settings (sound, volume, auto-close) as they are, reachable from the gear button's long-press or its small arrow.

## 2. The island closes when the mouse leaves
- The auto-close choices are `[10, 15, 30]` s in the island (`src/views/views.ts:773`), with a minimum of 5 s in the Control Centre (`src/settings/main.ts:953`). The default is 15 s. The owner finds 10 s too long.
- New choices, in both places:
  - **When I leave** (closes about 0.3 s after the pointer leaves the island, so it doesn't flicker);
  - 1 s, 3 s, 5 s, 10 s, 30 s.
- Make **"When I leave" the default** for new installs, and offer it to existing users once.
- In `src/island/fsm.ts`, allow `homeToPetitDelay = 0`, meaning collapse on leave with the short grace period. The pointer coming back cancels the collapse.
- These cases still stay open:
  - an alert waiting for an answer (already handled);
  - a pinned island;
  - a text field with focus (typing in the chat);
  - an active call or dictation.
- Hide the countdown bar when the delay is under 3 s.
- Add tests for 0 s and 1 s, and for the pointer coming back.

## 3. A truly light local model, for anyone who wants fully local
The catalogue already has small models: Qwen2.5 0.5B (~490 MB) and Llama 3.2 1B (~810 MB). They're still being refused, and the reason is this:
- `memory_refusal()` (`models.rs:506`) compares the model with **free memory right now** × the profile share (0.6 on Lightweight). On an 8 GB PC at 91% used, that's only about 400–700 MB, so even the 0.5B model gets refused.

Fixes:
- **Judge models by the PC, not by this second.** A model is *suitable* if it fits in about 40% of **total** RAM, which on 8 GB means up to ~3 GB.
  - If free memory is short at load time, say so and offer "Free up memory" (list the biggest apps) or "Load anyway".
  - Don't refuse a model that suits the PC.
- **One clear "Light local" option in the first-run AI choice and in Settings → AI:**
  - a single recommended conversation model, picked by RAM and shown with its size and expected speed;
  - one-click download.
  - Candidates to check: Gemma 3 1B Instruct (good conversation for its size), Qwen3 0.6B / 1.7B, Llama 3.2 1B, and the existing Qwen2.5 0.5B as the "tiny" fallback.
  - Verify each one's licence, file, SHA-256 and real RAM use on an 8 GB Windows PC before adding it. Mark every catalogue entry "Fits this PC" / "Too big for this PC".
- **Make light models feel fast for conversation:**
  - context 2048;
  - short spoken answers (system prompt: "reply in 1–2 sentences unless asked for more");
  - stream the reply and start speaking at the first sentence;
  - keep the model loaded during a call, then unload when idle (round 1).
- **One local engine.** Groovy currently has both its own llama.cpp (`models.rs`) and Ollama (`ollama.rs`), and the Task Manager screenshot showed both `llama-server.exe` and `ollama.exe` running.
  - Use Groovy's built-in llama.cpp by default.
  - Only use Ollama if the user already has it and picks it.
  - Never run both at once.

## 4. Voice calls: you can't talk to the AI
Pipeline today (`src/island/voice.ts`):
1. microphone →
2. **Whisper** (`voice.rs`, its own model download) →
3. the chat brain (local llama-server by default) →
4. `speak()`.

Any step can fail silently: no Whisper model, the brain refused for memory, or no TTS voice. Separately, dictation uses **Parakeet** (`speech/`, another download), so the user has to download two different speech models.

Fixes:
- **One speech engine for everything**: calls, dictation, Notetaker and OK Groovy.
  - Settings → Voice → Recognition offers "Parakeet (fast, 25 languages)" or "Whisper (more languages)", downloaded once and used everywhere.
  - Use **Whisper small/base** on low-power PCs if Parakeet is too heavy; measure both on 8 GB.
- **Show every stage of a call in the island:** Listening → Heard "…" → Thinking (brain: Gemini / Local Gemma 1B…) → Speaking. A failed stage shows the reason and a fix button (Download speech model, Choose an AI, Pick a voice).
- **"Test my voice setup" button** (Settings → Voice), which checks each stage in turn:
  1. the microphone level;
  2. a 3-second recording transcribed;
  3. "say hello" sent to the brain;
  4. the answer spoken.
  
  Report ✓ or ✗ with the reason for each.
- The call must work with:
  - a cloud brain (Gemini key);
  - a subscription brain;
  - the light local model from item 3.
- Write the voice log to `%LOCALAPPDATA%\Groovy\logs\voice.log` so failures can be diagnosed.

## 5. Dictation: make it work, and make its purpose obvious
- Dictation doesn't need an AI model. It's speech-to-text that **types what you say into any app** (Word, Discord, the browser…). Hold the key, speak, let go, and the text is pasted where the cursor is.
- The AI only does the *optional* clean-up (Styles).
- Fixes:
  - **Flow → Dictation page header:** one line explaining it, the hotkey shown big, and a "Try it here" text box to test in.
  - **Works without any AI:** if no AI is set, or the clean-up fails, paste the raw words anyway. Never block on the brain. Clean-up runs only when a brain is available and the style isn't "As said".
  - **Uses the shared speech engine** from item 4. If it's missing when the hotkey is pressed, the pill says so with a Download button (round 1 item 3).
  - Check the right-Ctrl hook on Windows. If the key isn't seen (some laptops, or another app holding it), say so and suggest another key.
  - Test it in Notepad, Word, Chrome and Discord, and record the results in STATUS.md.

## Done means
- Gear button on the island.
- The island closes when the mouse leaves (by default).
- A light local model downloads and holds a spoken conversation on an 8 GB PC.
- A voice call works end to end with Gemini and with the light local model.
- Dictation types into Notepad with no AI configured.
- The voice test reports every stage.
