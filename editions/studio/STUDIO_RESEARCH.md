# Studio beta: repo research (30 Oct 2026)

Read from the pages and READMEs listed at the end. Licences were taken from those pages; **check each LICENSE file in the repo before relying on it.** Nothing here is legal advice.

## 1. Driving ArtCraft's apps (the "tell Groovy to edit this" flow)
| App | What it is | Can another program drive it? |
|---|---|---|
| ArtCraft Studio | Rust desktop IDE for AI image/video; 62 models (Veo, Kling, Seedance, FLUX…); README says dual MIT/Apache-2.0 | README mentions no CLI/API/MCP; hosted generation uses credits or the user's own keys |
| **FilmCraft** (Premiere alternative, alpha 0.8) | Pure-Rust editor | **Yes: `filmcraft-cli mcp` (stdio MCP server), 650+ commands (`timeline.place`, `effects.setParam`, import, transcript, audio mix, export), `command_list`/`command_run`/`command_batch`, `render_frame` to check the result, `edit.undo`. Headless mode (no window) or "bridge" mode to a running app started with `--control 9876`.** Limits: alpha, ~56 % real-work readiness, no VST3/OpenFX, no AV1, CPU-bound renders. Headless mode has no folder confinement. |
| PhotoCraft (Photoshop alternative, early alpha) | dual MIT/Apache-2.0 | article says typed commands over a documented control protocol (verify) |
| EffectCraft (After Effects alternative) | in development | no automation yet: use Groovy's own motion kit or FilmCraft |
| LightCraft, VectorCraft, PdfCraft, DesignCraft | in development / early | not stated |

Consequence: **FilmCraft is the first real integration target** (Groovy speaks MCP to `filmcraft-cli`). Groovy must confine the folders itself, because headless FilmCraft can read and write anywhere.

## 2. Teaching Groovy to edit (car edits, transitions, text, motion)
- **browser-use/video-use** (MIT): an agent edits from a **transcript with word timestamps plus a filmstrip/waveform image**, writes an edit list, renders with ffmpeg, checks its own cuts and fixes them (up to 3 times), keeps notes in `project.md`. Needs an **ElevenLabs Scribe key (paid)**: replace with Groovy's local speech-to-text (sherpa-onnx), to keep it free. Good pattern to copy (idea only; MIT code may be used with its notice).
- **OpenCut** (MIT): browser/desktop editor, Rust→WASM core; **no agent API yet** (MCP planned 2027+). Watch, don't depend on it.
- **Groovy style library:** Groovy researches (WebSearch) how a style is cut (car edits: beat-synced cuts, speed ramps, shake, flashes, colour grade, kinetic text), distils it into **editable style recipes** (JSON: cut rhythm vs. the music's beat, transitions, text styles, grade), and saves them in the vault (see 6). It learns recipes, never downloads copyrighted clips.

## 3. Posting
- **Postiz** (AGPL-3.0): 14+ platforms (Instagram, YouTube, TikTok, X, LinkedIn, Pinterest, Threads, Bluesky, Discord…), public API, Node SDK, n8n node, MCP server, CLI. But: self-hosting needs **Postgres, Redis, Temporal**, and **you create and get approved your own developer app for each platform**. **Too heavy to bundle in a desktop app, and AGPL has strict source rules.** Use only as an optional "connect your own Postiz" resource.
- **Default path:** Groovy talks to each platform's own API directly, starting with **YouTube Data API** (free quota, private uploads until Google's audit). TikTok and Instagram posting need app review, and Instagram posting needs a Business/Creator account.

## 4. Website building and "reverse engineering"
- **JCodesMore/ai-website-cloner-template**: pipeline (recon, extract design tokens/assets, component specs, parallel builders, visual-diff QA) into Next.js 16/React 19/Tailwind 4, driven by a coding agent. Licence not shown on the page: check.
- **firecrawl/open-lovable** and **abi/screenshot-to-code**: screenshot or URL to code. The screenshot-to-code fork read needs an OpenAI key (and a ScreenshotOne key to clone a URL): not free.
- **Rules for Groovy:** rebuilding **your own sites, sites you're migrating, or a layout you're studying with your own copy, images and brand** is fine. Copying someone else's site and publishing it (their code, text, images, logo) is copyright/trademark infringement, and a look-alike of a bank or shop is a scam. Groovy builds from the extracted *structure and design tokens*, with new content, and refuses to publish a copy of a third party's brand.

## 5. Graphify
safishamsi/graphify (Apache-2.0 / MIT): turns code, docs, PDFs, images and media into a **local knowledge graph** (code parsed locally with tree-sitter; docs and media use the assistant's model). Outputs `graph.html`, `GRAPH_REPORT.md`, `graph.json`; query with `graphify query/path/explain`, or an optional MCP server; `--wiki` makes a markdown wiki. Two uses:
1. **For Claude Code on the Groovy repo itself:** run `/graphify .` so it queries the graph instead of reading whole files (fewer tokens).
2. **For Studio:** a project/brand graph (scripts, assets, past videos) the Researcher and Manager use.

## 6. Obsidian
A vault is just a folder of markdown files. **Groovy Studio's memory can be a vault**: research notes, scripts, style recipes, the content calendar, asset lists, plus `[[links]]` the owner can browse in Obsidian. No plugin or bundling is needed (read/write plain `.md`); MCP servers for Obsidian exist if wanted. Graphify's `--wiki` output can land in the vault.

## 7. The studio "crew"
Roles run through the existing `Brain` interface, one model per job, with the Planner choosing the resource and cost: **Researcher** (web, trends, style recipes) → **Writer** (script, captions, titles) → **Designer** (thumbnails, motion kit, images) → **Editor** (FilmCraft MCP, ffmpeg, video-use pattern) → **Poster** (YouTube API, others) → **Manager** (reviews every output against the brief, shows the plan and cost, and **asks the owner before anything is published**).

## 8. Selling and licence
- Free Groovy plus a paid Studio (about £10) sold through a creator marketplace that acts as the **merchant of record** (handles UK VAT and payments). Examples: Gumroad, Lemon Squeezy, Payhip, itch.io, Ko-fi. Owner to name the one he meant.
- **Licence must be decided before selling.** `LICENSE` says GPL-2.0; `Cargo.toml` and phone-link say MIT; the repo also carries `LICENSE-COUCOU-MIT`. A GPL'd paid binary can be passed on by buyers. Common options: keep the free edition open and make Studio a **separate, proprietary add-on** with its own licence; get a solicitor's view first. Dependencies' licences (and AGPL tools like Postiz) matter too.

## Sources
- https://osrepos.com/repo/storytold-artcraft.md · https://raw.githubusercontent.com/storytold/artcraft/HEAD/README.md
- https://aiidelist.com/blog/artcraft-photocraft-filmcraft-free-adobe-alternatives.md · https://aiidelist.com/blog/filmcraft-mcp.md
- https://github.com/browser-use/video-use · https://github.com/opencutvideo/Opencut
- https://github.com/gitroomhq/postiz-app
- https://hublens.pages.dev/en/JCodesMore-ai-website-cloner-template/ · https://www.github.com/omalab/screenshot-to-code
- https://github.com/safishamsi/graphify
