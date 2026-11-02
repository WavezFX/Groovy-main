# Fixes round 3I: memory as linked markdown notes, smarter recall (2 Nov)

**Do this after 3H and 3D item 11.** One item per commit: "Fixes round 3I, item N: ...", tick it in `WORK_QUEUE.md` in the same commit, push, keep the build green. No user-facing feature is removed. Stay under 300 MB idle and 400 MB at peak; nothing loads until memory is used.

## Why
Memory is a SQLite table (`memory.rs`): four kinds, newest and pinned first into a 1500-character prompt budget, no search, no links. So recall is "most recent", not "most relevant". Plain markdown files with `[[links]]` fix that, can be browsed and edited in any editor (Obsidian opens the folder with no plugin), and are easy to back up.

## Item 1: Memories as markdown files
- A `memory` folder (default in the user's Documents, changeable in Settings -> Memory). One `.md` file per memory, slugged name, frontmatter `kind` (preference/fact/project/task), `pinned`, `created`, `updated`, then the text. Links are `[[other-note]]`.
- The files are the source of truth; the database is an index rebuilt from them if missing. First run imports every existing database memory into files (no loss, keep the 2000-character limit and kinds).
- Settings -> Memory keeps working (read, edit, pin, delete, clear, JSON export). Memory-off and clear-memory rules from `memory.rs` still hold. Never write secrets or keys into notes; reuse the log redaction.
- Writes are atomic (temp file then rename). Reject path tricks in slugs (`..`, separators).

## Item 2: Folder watcher (event-driven)
- Use Windows file-change notifications (`ReadDirectoryChangesW` or the `notify` crate), no timer polling (3F item 4). Start only when memory is on and the folder exists; stop when memory is turned off.
- Edits made outside Groovy (Notepad, Obsidian) update the index within a second. Debounce bursts. Conflict rule: the newer file wins; never overwrite a file changed since Groovy last read it.

## Item 3: Search index and links
- SQLite FTS5 over note text and title, plus a small `links(from, to)` table filled by parsing `[[...]]`. Build lazily on first recall; keep it on disk, not in RAM; release connections when idle (3F resource manager).
- Offer related notes and backlinks to the Settings -> Memory page ("Linked from" / "Links to").

## Item 4: Recall by relevance within the same budget
- Prompt memory = pinned notes first, then FTS5 matches for the user's request, then notes linked one hop from the top matches, then newest as the fallback, until the 1500-character budget is full. The budget does not grow.
- Memory text is data, not instructions (3G item 5): keep marking it as untrusted data in the prompt.
- Tests: a note about headphones is recalled for a music question and not for a shopping-list question; one-hop link added; pinned always present; memory off adds nothing; the budget is never exceeded.

## Item 5: Checks
- Idle private working set and CPU unchanged within noise (record before/after in `STATUS.md`); peak under 400 MB; no new thread running while memory is idle.
- Round-trip test: write in Groovy, edit externally, Groovy sees it; delete file, memory gone from index.
- Folder opens in Obsidian with working links and graph (manual check, note it in `STATUS.md`).

## Not in this round
- **Graphify inside Groovy:** not doing it (Python tool, needs a model for prose, little gain on a small memory set). Graphify on the repo for Claude Code is a setup step, not code.
- No live Obsidian connection or plugin.
