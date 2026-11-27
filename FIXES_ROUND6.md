# Fixes round 6: Discord by voice and on the island

**Approved by the owner on 27 Nov 2026** (`docs/plans/DISCORD.md`). `docs/GROOVY_SPEC.md` still wins. Commit each item separately, with "Round 6, item N: …" as the subject, and tick it in `WORK_QUEUE.md` in the same commit.

The owner wants Discord done by voice and from the island: mute, deafen, who's in the call, leave the call, and messages ("message this person"), through the installed Discord app and without typing any IDs, the way the Spotify tile works.

## What Discord allows (checked 27 Nov 2026)

- **Local RPC** is the Discord app's IPC pipe: `\\?\pipe\discord-ipc-0` to `-9` on Windows, and `$XDG_RUNTIME_DIR/discord-ipc-N` on Linux. Each frame is a 4-byte opcode and a 4-byte length (both little-endian), followed by JSON. The opcodes are 0 HANDSHAKE (`{"v":1,"client_id":…}`), 1 FRAME, 2 CLOSE, 3 PING and 4 PONG.
- **Approval.** Discord does not allow RPC for unapproved apps unless the user is on the app's tester list. An unapproved app can have up to 50 testers. The `rpc`, `rpc.voice.read` and `rpc.voice.write` scopes are marked "approved partners only". So Groovy **cannot ship one shared app**. Each owner registers their own free Discord application and is its owner, so the owner is also its tester. Groovy keeps the **client ID as a setting** and never ships a client ID or a secret.
- **Sign-in.** AUTHORIZE opens Discord's own approval window and returns a code. The code is exchanged at `https://discord.com/api/oauth2/token`, then sent with AUTHENTICATE. Discord's OAuth page allows a code exchange with no secret only with a PKCE verifier. RPC AUTHORIZE does not document PKCE, so Groovy tries PKCE first. If Discord refuses, Settings → Discord asks for the app's **client secret**, which goes into Credential Manager and is never shown again. Tokens are stored in Credential Manager and refreshed. The user approves once.
- **Voice settings:** GET/SET_VOICE_SETTINGS (`mute`, `deaf`). Only one app can change voice settings at a time, and it holds them until it disconnects. That is one more reason to close the pipe when it isn't needed.
- **Voice channel:** GET_SELECTED_VOICE_CHANNEL gives the channel and its members. SELECT_VOICE_CHANNEL with `channel_id: null` leaves the call. SPEAKING_START/STOP and VOICE_STATE_* are subscribed per channel, and VOICE_CHANNEL_SELECT follows a channel change.
- **Messages:** Discord has no API for a user's own DMs, and using a user token ("self-bot") is against its terms. **Groovy never does that.** Messages come from Discord's Windows notifications only. A reply is opened in Discord itself, and **the user presses Enter**.

## How it fails gracefully

Every failure is a short sentence on the tile and in the voice reply, never silence (spec §4):

- Discord isn't running → "Discord isn't open."
- No client ID → "Set up Discord in Settings → Discord."
- Not authorised yet → "Approve Groovy in Discord first (Settings → Discord → Connect)."
- The scope is refused (`4002`/`4006`/`5000`), or the app is not approved and the user isn't a tester → "Discord refused voice control for this app: add yourself as a tester in the Discord Developer Portal."
- Another app holds the voice settings → Discord's own message.

If RPC isn't set up, the **round 4 item 5 shortcuts stay** (Ctrl+Shift+M and Ctrl+Shift+D with Discord in front). No feature is removed.

## Item 1: The Discord connection, lazily, over the local RPC pipe
- `src-tauri/src/discord.rs`:
  - framing, handshake, AUTHORIZE (with PKCE), the token exchange and refresh, and AUTHENTICATE;
  - the commands (`GET/SET_VOICE_SETTINGS`, `GET_SELECTED_VOICE_CHANNEL`, `SELECT_VOICE_CHANNEL` null) and events (`VOICE_SETTINGS_UPDATE`, `VOICE_CHANNEL_SELECT`, `VOICE_STATE_*`, `SPEAKING_*`).
- **Lazy:**
  - One connection task, only while it's wanted: the tile is visible (the island renews a 20 s lease) or a command is running.
  - It closes about 20 s after the last use. Nothing runs, and no pipe is open, while Discord isn't used.
  - Nothing is added to startup.
- The client ID lives in the database, like the mail client IDs. The optional client secret and the tokens live in Credential Manager (internal keys, never reachable from a webview).
- Commands: `discord_state`, `discord_watch`, `discord_authorise`, `discord_configure`, `discord_voice`, `discord_leave`, `discord_reply`, regenerated with `node scripts/command-access.mjs`.
- **Tests:** a fake IPC pipe (`tokio::io::duplex`) plays Discord's side. It covers the frame round trip; the handshake, then AUTHENTICATE, then SET_VOICE_SETTINGS; an error frame becoming a readable message; events changing the state; the speaking set; and the PKCE challenge.

## Item 2: The Discord tile
- New tile kind `discord` (one of a kind), offered in Settings → My island.
  - **Compact:** the channel and "muted", "deafened" or "speaking: Sam".
  - **Grown (3 × 2):**
    - the channel, and who's in it with the speaking ring;
    - big **Mute** and **Deafen** buttons, which show their state;
    - **Leave call**;
    - the latest Discord messages, each with **Reply**.
- Groovy's own style: the existing `grow-panel` pieces, frosted glass and animated in and out, using the island's grid layout.
- While the tile is visible it renews the lease (item 1). When hidden it stops, and the connection closes on its own.
- **Tests (front-end):**
  - `parseTiles` keeps a `discord` tile;
  - `expandsTo` grows it;
  - the face text for muted, deafened, speaking, not running and not set up.

## Item 3: Voice and chat commands with no AI
- `pc.rs` (inside `tools::parse`, so it runs before any brain on voice, chat and calls, as 4D item 7 asks):
  - "mute Discord", "unmute Discord", "deafen", "undeafen", "deafen Discord", "who's in the call?", "who's talking?", "leave the call", "hang up Discord".
  - The older phrases ("mute me on Discord", "deafen me") stay.
- **Run:**
  - RPC first: it sets the state exactly, and "who's in the call?" answers "Sam, Alex and you are in #general.";
  - with no RPC, the round 4 shortcut toggle;
  - "leave" with no RPC says how to set it up.
- `okgroovy` tool names `discord_unmute`, `discord_undeafen`, `discord_who` and `discord_leave` are added beside the existing ones.
- **Note:** "mute Discord" used to mute Discord's sound (per-app volume). The owner's brief makes it the mic. "Mute Discord's sound" and "turn Discord down" still change its volume.
- **Tests:** a table of phrasings in `pc.rs`, plus "mute" alone still being the system volume (`basic.rs`).

## Item 4: Messages through Windows notifications
- The mail notification watch (`island_tiles.rs`) also runs while the island has a Discord tile. Discord's notifications go to the tile and not to Mail.
  - Rust keeps the last few, in memory only, so "reply" knows who.
- "Reply" (on the tile), or "reply saying …", "message Sam on Discord saying …", "message Sam on Discord", "message this person":
  1. Discord is brought to the front (`discord://` when it isn't open).
  2. Ctrl+K opens the quick switcher, Groovy types the name (or `#channel` for a server message), and presses Enter.
  3. The drafted text is put on the clipboard. If **Type for you** is allowed in Settings → Permissions, it is typed into the message box instead.
  4. **The user presses Enter.** Groovy never sends.
- No user token, no Discord API for messages, ever.
- **Tests:**
  - the switcher query from a notification title ("Sam", "Sam (#general, Server)");
  - parsing of the reply phrases;
  - Discord notes being kept apart from mail (front end).

## Item 5: Settings, translations, privacy, checks
- **Settings → Discord:**
  - client ID, optional client secret, Connect (approve in Discord), status, Disconnect (forget tokens);
  - three numbered steps to set up the Discord app.
- Every new string in all ten languages (`i18n/extra.json`).
- **Privacy note** (`docs/PRIVACY.md`, or the privacy section where it lives):
  - what is read: the voice state over the local pipe, and Discord's notifications as Windows shows them;
  - what is stored: the client ID; the tokens in Credential Manager;
  - what is sent: only the OAuth exchange to discord.com;
  - what is never done: no user token, no sending for you.
- **Checks:** `groovy-test all` green, and the Windows build green with `[build]`. RAM: no new idle cost (nothing runs until the tile is shown).

## How the owner sets it up
1. Go to https://discord.com/developers/applications → New Application (any name, e.g. "Groovy").
2. OAuth2 → copy the **Client ID**. Add the redirect `http://127.0.0.1` (Discord asks for one). Copy the **Client secret** too, in case PKCE is refused.
3. In Groovy: Settings → Discord, paste them, then **Connect**. Discord shows "Authorise Groovy?"; approve it.
4. Put the Discord tile on the island (Settings → My island). Join a voice channel, then say "Hey Groovy, mute Discord" or "who's in the call?".
