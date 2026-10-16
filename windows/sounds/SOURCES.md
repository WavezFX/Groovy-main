# Where Groovy's sounds come from

**Every sound is made in code, at start-up, by `src/core/sounds-synth.ts`.** No
audio file ships with Groovy, so there is nothing to license and nothing taken
from another app.

## Audit (16 Oct 2026, FIXES_ROUND3 item 7)

This folder used to hold 29 WAVs. Each was checked by regenerating it with the
script that made it, `scripts/gen-sounds.py`. **All 29 came out byte-identical,**
so they were Groovy's own synthesised sounds, not Coucou's. A stale comment in
`src/core/sound.ts` had called them "the macOS app's own files". Even so, the
WAVs and the Python script have now been replaced by the TypeScript module,
which uses the same recipes. Nothing of Coucou's (`greeting.wav` included) is in
the repository or the installer.

## How they are made

Four building blocks, all plain maths on samples (48 kHz, mono, peak
normalised to 0.8):

- **bell**: a sine with a quieter overtone that fades faster, plus a soft
  octave. Its timbre follows Groovy's look:
  - Night: wood, a marimba bar (overtone ×3.93, fast fade, a short mallet tap);
  - Ember: warm (overtone ×2.76, slower fade, a fuller octave);
  - Frost: glass (overtones ×2.76 and ×5.4, the longest ring).
- **tone**: a sine, soft triangle or rounded square, with optional vibrato.
- **glide**: a pitch sweep from one frequency to another.
- **noise**: seeded white noise through a one-pole low-pass (always the same
  puff), for taps and breaths.

| Sound | Made of |
|---|---|
| `pop` | glide 900 → 380 Hz, 90 ms |
| `blip` | tone 1320 Hz, 50 ms |
| `tick` | tone 2200 Hz + noise tap, 18 ms |
| `hover` | soft tone 760 Hz, 60 ms |
| `open` | bells C5 → G5 |
| `close` | bells G5 → C5 |
| `peek` | glide 500 → 1100 Hz |
| `greet` | bells C5 E5 G5 |
| `greeting` | **the Seed intro**: a low glide and a puff as the seed cracks, then a rising chime of bells C5 E5 G5 C6 (under 1 s) |
| `finish` | bells C5 E5 G5, then a long C6 |
| `approval` | two pairs of bells E5 B5 |
| `approve` | triangle G5, then bell C6 |
| `question` | bell C5, then a glide C5 → F5 |
| `error` | rounded squares E4 → C4 |
| `rate` | bells G4 E4 C4 |
| `attach` | triangle glide 300 → 1400 Hz, then a 1800 Hz tick |
| `send` | glide 400 → 1600 Hz with a breath of noise |
| `gulp` | glide 700 → 220 Hz, then a low 110 Hz drop |
| `search` | a long bell at 1046 Hz and its echo |
| `think` | tones 660, 784, 880 Hz |
| `work` | four little taps at 1500 / 1620 Hz |
| `slap` | a burst of noise with a 180 Hz thump |
| `dizzy` | wobbly glide 900 → 300 Hz |
| `annoyed` | triangles D4 → C#4 |
| `love` | tones E5 → A5 with a little vibrato |
| `proud` | bells G4 C5 E5 G5, then C6 |
| `wink` | glide 1100 → 1700 Hz |
| `sleep` | soft tones E4 → C4 |
| `yawn` | slow triangle glide 520 → 240 Hz |

## Your own sounds

A file named after one of these (`finish.wav`, `approval.mp3`, `greet.m4a`…)
in the sounds folder replaces that sound after **Settings → General → Reload
sounds** (`%APPDATA%\Groovy\sounds` on Windows, `~/.config/groovy/sounds` on
Linux).
