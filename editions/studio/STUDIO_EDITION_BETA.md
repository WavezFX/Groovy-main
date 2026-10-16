# Groovy Studio Edition (Beta)

**Status:** a separate, paid edition, released as a beta. **Start it after Fixes round 5.** Work on the branch `studio-edition-beta`, which is cut from `groovy-ai-phase1`, and merge `groovy-ai-phase1` into it regularly. Commit each item separately with "Studio beta, item N: …" as the subject.

The **standard Groovy edition must not change**. It keeps the same memory, CPU and installer size. Everything Studio-only goes behind a Cargo feature `studio` and a Vite build flag. CI builds a second set of installers named `Groovy-Studio-Beta-Windows-Setup / Portable / MSI`.

> **Naming.** Groovy already has a **Coding Studio** window (`src/studio/`, `studio.rs`). Leave it alone. This edition's window is the **Creator Studio** (`src/creator/`, `creator.rs`), so the two are never mixed up.

## What it is

You tell Groovy what to make, for example "make me a 20-second motion intro for my channel", "make a thumbnail" or "post this to YouTube". His **planner** instantly decides how to make it: free and local, a free cloud tier, or a paid service such as Claude, Veo or Kling. He shows you the plan and its cost, makes the content, and only posts with your yes.

---

## Item 1: The Planner, Groovy's instant brain

This is the heart of the edition. It is **not** an AI call: it's a deterministic scorer in Rust (`creator/planner.rs`) that answers in under 10 ms, so choosing a resource never costs anything.

### 1a. Understanding the request

- Turn the request into a **Job** using local rules (keywords, plus what's attached or selected):
  - `motion` (animated text, intros, lyric videos, explainers);
  - `image` (pictures, thumbnails);
  - `video_clip` (AI-filmed footage);
  - `voiceover`;
  - `edit` (cut, caption or resize an existing video);
  - `post` (publish).
- Pull out the details: length, aspect ratio (16:9 / 9:16 / 1:1), style words and the target platform.
- **Only if the rules can't tell** does it ask the active brain one short classification question, using the existing `Brain` interface and the cheapest available model. The plan says when this happened.

### 1b. The resource catalogue

Each resource that can do a job is a small manifest: `creator/resources/*.json`, built in, plus user and plugin ones. Fields:

- `id`, `name`, `jobs` (the jobs it can do);
- `runs`: local or cloud;
- `cost`: per image, per second of video or per 1k tokens, in GBP;
- `free_quota` (per day or month, if any);
- `quality` (1–5 per job), `speed` (seconds per unit);
- `needs`: an API key, a GPU with N GB of VRAM, or an installed tool;
- `licence_ok`: true only if Groovy may use it in a paid product;
- `max_length`, `aspect_ratios`.

The planner skips any resource that isn't available right now: missing key, missing hardware, quota used up, or offline.

### 1c. Choosing

- The user picks a **mode** in Creator settings and can override it per request ("do this one properly"):
  - **Free only:** £0. Local tools and free tiers only.
  - **Balanced (default):** the cheapest option that meets the job's quality bar (motion 3, thumbnail 3, video clip 4).
  - **Best:** the highest quality, within the budget cap.
- **Score:** `quality_weight × quality − cost_weight × estimated_cost − speed_weight × estimated_time`. The weights come from the mode.
- **Plan card.** Before anything runs, the island shows the plan, for example "Motion intro · Claude writes it, made on your PC · about £0.04 · 1 min", with **Go**, **Cheaper** and **Better** buttons. Cheaper and Better jump to the next resource in the ranking.
- **Spending limits:**
  - Runs **under £0.10** (configurable, default £0.10) start straight away in Balanced and Free modes.
  - Anything above that always asks first.
  - There's a **monthly cap** (default £5) that the planner never goes over.
  - Spending is recorded locally with each provider's reported cost, where available.
- **Limit-based switching stays.** If a resource hits its quota or limit mid-job, the planner moves to the next-ranked one and tells you. This follows the same rule as `brain_ai.rs`.
- **Learning.** When a result is kept, redone or deleted, that resource's quality score for that job is nudged up or down (stored locally, with a small step). Groovy gets better at choosing for *you* over time.

### 1d. The text brains

When a job needs writing (motion code, scripts, titles), the planner asks for a *model choice per job* through the existing `Brain` interface. It doesn't pick a provider itself. Claude, Gemini's free tier, OpenAI-compatible APIs and Ollama/LM Studio all appear as resources, in the usual order: cloud and subscription first, local last. When the owner's **router plugin** arrives, it plugs in here and decides the model, and the planner keeps deciding everything else. **Don't remove anything** from the existing brain switching.

---

## Item 2: Motion designs, made by Groovy

This is the main Studio feature. It's fully in-house, cheap and legally clean.

- **Making it.** The chosen brain writes the animation as **self-contained HTML/CSS/JS on a canvas**, using a small Groovy motion kit: timeline helpers, easing, text effects and shape presets. The kit is written from scratch in this repo, with no copied animation code.
  - Don't use Remotion. It needs a company licence for paid products.
- **Preview.** It plays in the Creator Studio window, with a scrubber and an "ask for a change" box, where each change is a new version.
- **Export.** It renders frame by frame in an off-screen WebView and is encoded with **ffmpeg** (downloaded on first use, LGPL build) to MP4 or WebM, with optional transparent WebM and a GIF.
- **Sizes:** 16:9, 9:16 and 1:1.
- **Audio:** an optional Kokoro voiceover (already in Groovy) and music from a file you supply.
- **Templates:** intro, outro, lower third, lyric video, countdown, quote card and explainer.
- **Memory.** The Creator window is created only when it's opened and destroyed when it's closed, following round 3 item 9. Rendering runs in its own window and process, which closes afterwards.

## Item 3: Images and video clips

- **Free resources:**
  - **Local, optional, only offered if the hardware fits:** a ComfyUI install that the user points Groovy at. Groovy talks to it over its HTTP API as a separate program and never bundles it. Built-in workflows cover FLUX.1 [schnell] (Apache-2.0) for images and LTX-Video or Wan for short clips. The planner checks VRAM first and won't offer them on a laptop without a suitable GPU.
  - **Free cloud tiers** where a provider offers one. Check the current terms at build time, and only add a resource as `licence_ok` if commercial use is allowed.
- **Paid resources (BYOK, the user's own key):** Veo, Kling, Flux Pro, GPT Image and similar, plus **Open Higgsfield-style aggregators** as an *external* plugin.
  - Open Higgsfield has no licence file, so **don't copy its code**. Talk only to the services it calls, or to a running instance of it, over HTTP.
- **Thumbnails** are made with the motion kit (text and shapes, free) or with an image model plus a text overlay.

## Item 4: Posting, starting with YouTube

- **Accounts.** In Creator settings, sign in with OAuth 2.0 using the platform's own sign-in page in the default browser (DuckDuckGo, as in round 4). Tokens are stored in Windows Credential Manager, never in settings.
- **YouTube first:** `youtube.upload` scope, resumable upload, then title, description, tags, thumbnail, privacy and schedule time.
  - Until SSD's Google Cloud project passes the YouTube API audit, uploads can only be **private**. Show "Uploaded as private: make it public in YouTube Studio" and link to the video.
  - The owner will apply for the audit before the paid launch.
- **The flow:** "Groovy, post this to YouTube" → he drafts the title, description and tags with the planner's text brain → a preview card on the island → **Post** only after the user taps yes. Nothing is ever posted without that tap, the same rule as round 4's permissions.
- **Later:** TikTok (Content Posting API, needs app review), Instagram (Business or Creator accounts, Graph API) and X (paid API). Each one is its own resource manifest and its own item when it's added.

## Item 5: The paid tier

- **Licence check.** A licence key is a signed file: an Ed25519 signature over email, edition, expiry and a beta flag. It's verified **offline** against a public key built into the app.
  - No account server is needed.
  - Without a valid key, the Studio features are hidden and the standard edition works as normal.
- **Selling.** Use a merchant-of-record shop that handles UK VAT and issues the keys, for example Lemon Squeezy or Paddle. The owner picks one. Add a "Get Studio" button that opens the shop page.
- **Beta.** Keys are flagged `beta`. Show "Studio Beta" in the island's quick settings, with a feedback button that opens a pre-filled GitHub issue or email.
- **Generation costs belong to the user.** Groovy never resells API calls in the beta: users bring their own keys or use free resources. Groovy credits may come later.

> **Licence note for the owner (not a code task).** Groovy is published under **GPL-2.0**, and Coucou's code inside it is MIT. You can charge for GPL software, but anyone who receives a GPL program can share its source. Two common ways to handle this:
> 1. SSD owns the Studio code, so it can be released under a **different licence** as a separate add-on program that talks to Groovy over IPC.
> 2. Sell the **builds, updates and support** while the code stays GPL.
>
> Decide before the paid launch, and get proper legal advice. Until then, keep Studio code in `editions/studio` and `src/creator`, with its own licence header left as `TBD`.

## Item 6: Tests and checks

- **Planner unit tests:**
  - mode × job → expected resource, using fixture catalogues;
  - quota exhaustion moves to the next resource;
  - the budget cap is never exceeded;
  - learning nudges are bounded;
  - ambiguous requests go to a brain call; clear ones don't.
- **Motion export:** a 3-second template renders to MP4 in CI (headless) and the frame count is checked.
- **YouTube:** a mocked upload, private by default, posting only after confirmation (also tested in the UI).
- **Licence checks:** valid, expired, wrong signature, and the standard edition when there's no key.
- **Memory:**
  - the standard edition's idle smoke test stays **under 300 MB**;
  - the Studio edition's idle memory with the Creator window closed is also under 300 MB;
  - while rendering it can go higher, but must drop back after the window closes.
