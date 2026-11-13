# Repo scouting: what could make Groovy a must-have (4 Nov)

**Research only. Nothing here is queued or approved.** Purpose: find open-source projects whose ideas or code could lift Groovy (and the paid Studio edition) from "nice companion" to "I need this on my laptop".

**How to read the marks**
- **✔** = I read the project's own page or a catalogue page this session. The licence shown is what that page said.
- **○** = named by a catalogue, or from my own knowledge. **Licence not re-checked: verify before any code is used.**
- **Idea** = borrow the design, write our own code. **Use** = could be called or linked as it is. **Skip** = not for Groovy.

**Licence rule of thumb** (not legal advice; get a solicitor before the licence change): MIT, BSD, Apache-2.0 and the Unlicense are fine in a paid edition. AGPL and GPL code must not be bundled into Studio. **Apache-2.0 code cannot be combined with GPL-2.0-only code**, and the repo is GPL-2.0 today, so the licence decision matters for what can be reused. Rust crates and ONNX models each carry their own terms.

**Budget rule:** anything that runs inside Groovy must fit 300 MB idle / 400 MB peak, load on demand, and cost near 0 % CPU when idle. Heavy things run as an optional helper the user installs, never bundled.

---

## 1. Voice that feels instant

| Repo | Licence | What it is | Fit for Groovy |
|---|---|---|---|
| pipecat-ai/smart-turn ✔ | BSD-2-Clause | An ~8M-parameter model that decides when you have *finished* speaking, from the audio itself. CPU build about 8 MB; 10-100 ms. 23 languages. | **Strong.** Live voice chat (round 4 item 3) otherwise cuts people off or waits too long. Small enough to load only during a conversation. Check it exports to ONNX so it runs beside sherpa-onnx; its page doesn't say. |
| moonshine-ai/moonshine ✔ | MIT (some old non-English models non-commercial) | Streaming speech-to-text with tiny models, works while you are still talking, Windows listed. | **Strong, may already be reachable** through sherpa-onnx. Streaming gives faster first words. Compare against the current Parakeet default for RAM and accuracy. |
| cjpais/Handy ✔ | MIT (name and logo not free) | Tauri v2 + Rust push-to-talk dictation; Whisper/Parakeet, Silero VAD. | **Idea.** Same stack as Groovy. Borrow: single-instance + CLI flags to control a running copy, model auto-discovery in a shared folder, paste-delay safeguards, "hold" vs "tap" shortcut modes. |
| snakers4/silero-vad ○ | MIT (check) | Tiny voice-activity detector. | Handy and sherpa-onnx already use it. Confirm Groovy uses it before adding anything. |
| kyutai-labs/pocket-tts ○, supertone-inc/supertonic ○, OpenMOSS/MOSS-TTS-Nano ○ (Apache-2.0 ✔ per list) | check | Small CPU voices. | Lighter fallback than Kokoro when RAM is tight. Optional; Kokoro already works. |
| pipecat-ai/pipecat ○, livekit/agents ○ | check | Voice-agent frameworks (Python). | **Idea only.** How they handle barge-in and streaming order. Groovy already has LiveKit support. |

## 2. Doing things in your apps (the "message this person" problem)

| Repo | Licence | What it is | Fit |
|---|---|---|---|
| trycua/cua ✔ (Cua Driver) | MIT for SDK, CLI, **Cua Driver**; some parts FSL or **AGPL** (cua-som, cua-perception) | Inspects and operates native desktop apps and browsers; CLI, MCP server and SDK; Windows install by PowerShell; Rust SDK exists. | **Best candidate to study first** for Discord / Instagram / any installed app without IDs (see `docs/plans/MESSAGING.md`). Use only the MIT Driver; avoid the AGPL perception parts. Run as an optional helper. |
| bytedance/UI-TARS-desktop ✔ | Apache-2.0 | A GUI agent for Windows/macOS using a vision model (7B local option); Agent TARS mounts MCP servers. | **Idea.** The model is too big for the RAM budget; borrow the event-stream log (every action recorded and replayable), hybrid "GUI or DOM" control, MCP mounting. Apache-2.0 vs GPL-2.0 caveat applies. |
| simular-ai/Agent-S ✔ | Apache-2.0 | Open computer-use agent, Windows listed. | Idea: planning loop and verification steps. Python, heavy. |
| web-infra-dev/midscene ✔ | MIT | Vision-based UI automation. | Idea: describe-the-target prompts. Windows desktop not stated. |
| OpenAdaptAI/OpenAdapt ✔ | MIT | Records what you do, learns to repeat it. | **Idea worth a closer look:** "teach Groovy by showing it once" for repeated jobs. |
| OthersideAI/self-operating-computer ✔ | MIT | Early screenshot-and-click agent. | Skip (superseded). |
| microsoft/OmniParser ✔ | Page inconsistent (MIT badge, CC-BY-4.0 sidebar); icon model historically AGPL, v3 says MIT-based | Turns a screenshot into labelled clickable elements. | **Do not adopt until the licence is settled** and the CPU cost measured. |
| *(no repo)* Windows UI Automation | OS feature | The accessibility tree of every app, free and instant. | **The cheapest route.** Try it before any vision model: names, buttons and text come for free, no GPU, no model. Screenpipe does "accessibility first, OCR as fallback" for the same reason. |
| e2b-dev/open-computer-use ○, Surf ○ | Apache-2.0 ✔ | Cloud desktops for agents. | Skip (cloud, Linux). |

**Safety lesson from OpenClaw (MIT ✔):** a popular local agent was criticised for broad default permissions, unvetted third-party skills, unencrypted secrets, sandbox off by default and agents acting beyond what the user asked. Groovy's round 3G rules (Rust-side approvals, per-window capabilities, untrusted content cannot trigger actions) already answer most of this. Keep them for every future skill and plugin.

## 3. Memory that actually knows you

| Repo | Licence | What it is | Fit |
|---|---|---|---|
| letta-ai/letta ✔ | Apache-2.0 | Agents edit their own tiered memory. | **Idea:** let Groovy merge duplicates and rewrite stale memories (with approval). Whole runtime is too heavy to embed. |
| mem0ai/mem0 ✔ | not stated on page | Extracts and de-duplicates memories; graph part is paid-only. | Idea: extraction prompts and de-duplication rules. |
| getzep/graphiti ✔ | not stated | **Temporal** knowledge graph: every fact has a "valid from / until". | **Best idea to borrow:** "I moved to X" should retire "I live in Y" rather than keep both. Store a `valid_until` field in the note frontmatter. Needs a graph database, so don't embed. |
| topoteretes/cognee ✔ | not stated | Graph + vector + SQLite store, runs offline via Ollama. | Idea; Python and heavy. |
| supermemoryai/supermemory ✔ | not stated | MCP-native memory with an explicit "forget" mechanism. | Idea: forgetting as a first-class command ("forget that"). |
| NirDiamant/Agent_Memory_Techniques ✔ | not checked | 30 notebooks on memory patterns. | Reading list for the 3I follow-up. |
| cyberlife-coder/VelesDB (Tauri plugin) ✔ | not stated | Embedded vector + keyword database plugin for Tauri, offline-first. | **Later option** if keyword search (FTS5, round 3I) proves too literal. Measure RAM first. |
| sentence-transformers/all-MiniLM-L6-v2 ○, BAAI/bge-small-en-v1.5 ○ | check | 22M-33M parameter text-embedding models. | Same "later" option: meaning-based recall at a few tens of MB, loaded only during recall. |
| screenpipe/screenpipe ✔ | "source-available", licence unclear; paid plans from $25/month | Records screen and mic, searchable, plus an API. | **Do not bundle** (licence, privacy, RAM). Idea: opt-in "what was I doing at 3 pm?" using accessibility text rather than video. A Studio-only maybe. |
| yuka-friends/Windrecorder ✔ (named) | not checked | Open Recall-style rewind. | Same idea; check licence and storage cost first. |

## 4. Launcher and everyday Windows power

| Repo | Licence | What it is | Fit |
|---|---|---|---|
| Flow-Launcher/Flow.Launcher ✔ | MIT | Windows launcher: file search through Everything/Windows Index, calculator, web search, clipboard history, plugin store. | **Most useful reference for "needed every day".** Borrow: **Game Mode** (pause global hotkeys during full-screen games), **Dialog Jump** (jump any Open/Save dialog to the folder you are working in), plugin store with per-plugin priority, preview panel. |
| microsoft/PowerToys: Command Palette ✔ (search results) | MIT (check) | Microsoft's launcher with an extension gallery. | Idea: extension gallery with install and update in one place. |
| CrossCopy/tauri-plugin-clipboard ✔ | not stated | Clipboard text/images/files with change monitoring. | A clipboard-history tile for the island. Event-driven. |
| ayangweb/tauri-plugin-screenshots ✔ | not stated | Window and monitor screenshots. | Check against what Groovy already has for the screen buddy. |
| Charlie-XIAO/tauri-plugin-desktop-underlay ✔ | not stated | Puts a window between desktop icons and wallpaper. | A Groovy who lives on the desktop (no always-on-top window). |
| fltsci/tauri-plugin-tracing ✔ | not stated | Structured logging with rotation. | Only if current logging is lacking. |
| tauri-apps/awesome-tauri ✔ | list MIT | Catalogue of Tauri plugins and apps. | Where to look before writing a new plugin. |

## 5. Skills, plugins and tools

| Repo | Licence | What it is | Fit |
|---|---|---|---|
| openclaw (MIT ✔) | MIT | Local agent with skills as `SKILL.md` folders (workspace over global over bundled). | Idea: same skill layout Groovy's skills manager already targets. Take the override order. Take the security warnings seriously. |
| szczyglis-dev/py-gpt ✔ | page doesn't say | Python desktop assistant: presets, editable model list, scheduler, loop step cap. | **Idea only** (Python, heavy). Borrow: presets (model + prompt bundles), editable `models.json`, a step cap on autonomous loops. |
| wong2/awesome-mcp-servers ○ and similar lists ✔ | n/a | Catalogues of MCP servers. | Where Studio and the skills manager find ready-made tools (files, browser, calendar). Vet each before listing. |
| hypothesi/mcp-server-tauri ✔ | not stated | MCP server for debugging Tauri apps. | A developer tool for Claude Code and Opus. Possibly useful for the screenshot tests. |
| kajisho5/ffmpeg-skill ✔ | MIT | 42 FFmpeg tools with an MCP server; cuts, captions, reframe, loudness; "no cloud, no API keys". | **Studio core tool**: fits the free-tools rule exactly. |

## 6. Studio: video, captions, shorts (free and local first)

| Repo | Licence | What it is | Fit |
|---|---|---|---|
| WyattBlue/auto-editor ✔ | Unlicense | Automatic silence-based video editing. | Studio: first-pass cut before anything clever. |
| modelscope/FunClip ✔ | MIT | Transcribe, LLM-assisted clipping, subtitles. | Studio: "find the best 30 seconds". LLM step is optional. |
| mutonby/openshorts ✔ | MIT | Long video into 9:16 shorts with moment detection, face tracking, subtitles, dubbing; MCP server and API. | **Studio: closest to a ready pipeline.** Heavy; run as a helper. |
| mutonby/shortcast ✔ | Apache-2.0 | macOS-only on-device shorts. | Skip (macOS), idea only. |
| artbyjazi/autoclip ✔ | MIT | Speaker-tracked vertical clips with Whisper + Ollama. | Idea; local-only path matches the free-tools rule. |
| tmoroney/auto-subs ✔, nicolaigaina/ai-video-captions ✔ | MIT | Local captions with Whisper + FFmpeg. | Studio captions. |
| Zulko/moviepy ✔, motion-canvas ✔, revideo ✔ | MIT | Video and animation as code. | Studio "make a motion graphic from words". Aligns with the motion-design manual. |
| remotion-dev/remotion ✔ | **custom licence** | React videos. | **Check the company licence** before using it in a paid edition. |
| nexu-io/html-video ✔ | Apache-2.0 | HTML/CSS to MP4 on your laptop. | Studio: the Groovy character as a video overlay. |
| lukaszliniewicz/Pandrator ✔, Huanshere/VideoLingo ✔ (Apache-2.0), SoniTranslate ✔ | MIT / Apache-2.0 | Local dubbing and translation. | Studio, later: dub a video into Spanish or Punjabi. |
| pluja/whishper ✔, ColinGPT9/clips-studio ✔, VoiceStudio ✔, Auto-Synced-Translated-Dubs ✔ | **AGPL-3.0** | Transcription, clips, dubbing. | **Skip for Studio code.** Fine as separate tools the user runs themselves. |
| pyvideotrans ✔ | **GPL-3.0** | Video translation. | Skip for bundling. |
| sasateguh/awesome-ai-video-for-creators ✔ | list | ~200 tools by category. | The catalogue to revisit when Studio starts. |

## 7. Small local models and engines (only if offline mode needs to improve)

| Repo | Licence | Fit |
|---|---|---|
| ggml-org/llama.cpp ○, ollama/ollama ○, EricLBuehler/mistral.rs ○, mozilla-ai/llamafile ○ | check | Ollama is already the first local choice. mistral.rs (Rust) would avoid a separate process but costs RAM; only if Ollama is shown to be the bottleneck. |
| k2-fsa/sherpa-onnx ○ | check | Already Groovy's speech runtime (STT, TTS, VAD, wake word). Check each new feature here first. |
| Dicklesworthstone/franken_ocr ○ | check | Pure-Rust CPU OCR. Only if reading text off the screen without Windows' built-in OCR becomes needed. |
| marcomq/tauri-plugin-python ✔ | not stated | Would let Groovy call Python helpers (Graphify, Whisper tools). Adds a Python dependency; prefer a separate helper process. |

---

## What would make Groovy "needed", and where each piece comes from

1. **It never cuts you off or misses you** (smart-turn, Moonshine streaming, Handy's hotkey modes).
2. **It can do the thing in your apps** (UI Automation first, Cua Driver as the helper, OpenAdapt-style "show me once").
3. **It knows you and keeps it straight** (round 3I notes + FTS5, then Graphiti-style `valid_until`, Supermemory-style "forget that", Letta-style tidy-up with approval).
4. **It finds anything fast and gets out of the way** (Flow Launcher: Game Mode, Dialog Jump, clipboard history, preview).
5. **Skills you can trust** (OpenClaw layout with Groovy's approvals; a vetted store, not a free-for-all).
6. **Studio: idea to posted video, free tools first** (ffmpeg-skill MCP, auto-editor, FunClip, openshorts, Moonshine/Whisper captions, FilmCraft via MCP).

## Shortlist to look at first (no commitment)

1. Smart Turn v3 (voice) 2. Windows UI Automation + Cua Driver (apps) 3. Flow Launcher ideas: Game Mode and Dialog Jump 4. Graphiti's `valid_until` idea (memory) 5. Moonshine streaming vs current speech default 6. ffmpeg-skill MCP + auto-editor (Studio) 7. OpenAdapt "learn by watching" 8. Handy's CLI-control and model-folder ideas 9. OpenClaw's skill override order (and its security lessons) 10. tauri-plugin-desktop-underlay (a Groovy that lives on the desktop)

## Not recommended

OmniParser until its licence is clear; Screenpipe as a bundled part; anything AGPL or GPL inside Studio; Graphify or Python tools inside Groovy's core; any cloud-only computer-use sandbox; running a 7B vision model locally for control (breaks the RAM budget).

## Open checks before any of this is used

- Read each ○ project's LICENSE file.
- Decide the repo licence (GPL-2.0 vs MIT) first; it changes what can be reused.
- Measure RAM and CPU on a Windows laptop; none of these figures are Groovy measurements.
- Catalogue star counts and "last commit" dates were as of 29 Oct 2026 on the page read; things move fast.
