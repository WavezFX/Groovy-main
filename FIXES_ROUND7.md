# Fixes round 7: Instagram messages by voice (27 Nov 2026)

Read `docs/GROOVY_SPEC.md` first (it wins), then `docs/plans/MESSAGING.md` (approved by the owner, 27 Nov 2026). The aim: "Hey Groovy, message Sam on Instagram" works through the Instagram app or instagram.com the owner is already signed into, with no IDs, keys or tokens, the way the Spotify tile works through the Spotify app.

**Within Instagram's terms:** no private API, no session token or cookie, no scraping, no stored password. Groovy only reads Windows' own notifications, opens Instagram's public web addresses (`instagram.com/direct/…`, `ig.me/m/<username>`) or the app, puts the draft on the clipboard, and (only with the owner's permission) types the name and the draft like a keyboard would. **Groovy never presses Send**: the owner does.

**Own module, no shared layer:** the Rust code is in `windows/src-tauri/src/instagram/`, the page's in `windows/src/instagram/`. Round 6 (Discord) is built at the same time on another branch; nothing here is shared with it. **After both merge, they can share code**: the notification-to-sender parsing, the "message X on Y" parser and the open-then-draft flow are the same shape in both, and can move into one messaging helper then.

**Rules (WORK_QUEUE.md):** RAM under 150 MB idle and 300 MB peak; lazy (nothing runs unless Instagram is on and used); no feature removed. New Tauri commands: `node scripts/command-access.mjs`. New text: all ten languages. New setting: `tests/settings-keys.test.mjs`. Windows-only code behind `cfg(windows)`; the Linux build keeps compiling.

## Item 1: New DMs on an Instagram tile
**Files:** `src-tauri/src/instagram/notes.rs` (new), `island_tiles.rs` (the mail listener), `src/instagram/store.ts` (new), `src/views/tiles.ts`, `src/core/tiles.ts`, `src/main.ts`.
1. The notification listener used for mail (`island_tiles.rs`, `UserNotificationListener`) also runs while Instagram is on and the island has an Instagram tile. No second listener and no second job on the scheduler.
2. A notification is Instagram's when its app is Instagram (the Store app or an installed Edge/Chrome web app) or its text names `instagram.com` (a browser's site notification). From its texts: the sender (the title, or "name: text" in the body when the title is just "Instagram") and a preview of at most 120 characters. Likes, follows, comments and lives are not DMs and are skipped.
3. Each new DM goes to the island as `instagram-message` (sender, preview, which app) and **not** as mail. With Instagram off, the mail watch behaves exactly as before.
4. The island keeps the five newest DMs, in memory only. The tile shows the newest sender and preview in Groovy's style; clicking it opens that conversation (item 2's opener).
**Test:** Rust table test of fake notifications (app DM, Edge web-app DM, Chrome site DM "name: text", "sent you a message", a like, a follow, a mail from Outlook, an empty one); the watch runs only with Instagram on and a tile; page test of the store (newest first, five kept) and the tile face.
**Done when:** the tests pass, and with Instagram off nothing new is read or kept.

## Item 2: "Message <name> on Instagram" and "reply", with no AI
**Files:** `src-tauri/src/instagram/command.rs` (new), `instagram/open.rs` (new), `okgroovy/basic.rs`, `chat.rs`.
1. Plain parser (round 4D item 7's command path: before any brain, from voice, chat and calls): "message Sam on Instagram", "DM Sam on insta saying I'm on my way", "send a message to Sam on Instagram that I'm late", "Instagram Sam: running late", "open my Instagram messages", and "reply …" / "reply to Sam …" for the newest DM (only while Instagram is on and a DM came in the last 15 minutes, so "reply" means nothing else otherwise). Fillers ("please", "can you", "Groovy") don't matter.
2. Opening: a sender seen in a notification (an Instagram username) opens straight to the conversation at `https://ig.me/m/<username>`. Any other name opens the new-message search (`https://www.instagram.com/direct/new/`), and, only if "Type for me" is on, types the name there after the page has had time to open. "Open my Instagram messages" opens `https://www.instagram.com/direct/inbox/`. The chosen app: the Instagram app (when Windows has shown one of its notifications, by its AppUserModelID), Edge, Chrome, or the default browser.
3. The draft: the message text goes on the clipboard and Groovy says "Paste it with Ctrl+V and press Send". With "Type for me" on, Groovy types it instead, only when the window in front is Instagram's. Never Enter, never Send, one message at a time.
4. The AI only writes the message when asked ("…and write it for me", "ask Sam on Instagram if… in a nice way"): the brain gets the request and the reply becomes the draft. Everything else runs with no brain at all.
5. With Instagram off: "Instagram is off. Turn it on in Settings → General → Instagram."
**Test:** Rust table test of at least 30 phrasings (who, the text, reply, inbox, AI wanted, not Instagram at all: "message Sam" alone, "what's on Instagram"); the link builder (usernames only, encoded, nothing else); reply only with a recent DM.
**Done when:** the tests pass, and with no brain set up the voice path drafts the message.

## Item 3: Settings, translations, privacy
**Files:** `settings.rs`, `src/core/state.ts`, `src/settings/instagram.ts` (new), `src/settings/main.ts`, `src/settings/tilesgrid.ts`, `src/i18n/extra.json`, `tests/settings-keys.test.mjs`.
1. Settings → General → Instagram: on/off (off by default), "Open in" (Automatic, the Instagram app, Edge, Chrome, the default browser), "Type for me" (off by default), and the privacy note: "Groovy reads only Instagram's notifications on this PC, keeps the last five in memory, and never signs in, stores or sends anything to Instagram for you. You press Send."
2. The Instagram tile can be added from the island's tiles. Turning Instagram off clears the kept DMs.
3. Every new string in all ten languages.
**Test:** `settings-keys.test.mjs` knows `instagram`; the i18n test passes; Rust settings load an old file without the key (defaults: off).

## Item 4: Lazy and light; checks
1. Nothing runs unless Instagram is on: no listener work and no parsing in Rust (the mail watch stays idle without a Mail tile or Instagram on with its tile); the page's part is a 40-line store with no timers, so it is not split out.
2. No new process, no new window, no WebView. RAM: no change at idle (measured in the build's smoke test, written in `STATUS.md`).
3. `groovy-test all` green; one Windows build (`[build]`).
**How the owner tries it:** Settings → General → Instagram: turn it on; add the Instagram tile; in Edge (or the Instagram app) allow Instagram's notifications; send yourself a DM from another account; it shows on the tile. Then "Hey Groovy, message <username> on Instagram saying hello" and press Ctrl+V and Send.
