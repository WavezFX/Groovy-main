# Discord and Instagram on Groovy: how it would work (not started; owner to approve)

Builds on `docs/plans/DISCORD.md`. The aim: tell Groovy "message Sam I'll be 10 minutes late" and he does it, without the owner doing it by hand.

## The safe principle for both apps
- **Never** use a user token, a self-bot or a scraper (against both services' rules; the account can be banned).
- Groovy works **through the app the owner already has open and is signed into**, never with stored passwords, and **always shows what he's about to send and waits for a yes** (a "Send" button on the island, or "OK Groovy, yes, send it"). One message at a time, never loops, never bulk.
- Nothing needs the owner's own IDs or keys in the repo. If an app needs registering, it's a free app on the owner's own account, and its id/secret live in Windows Credential Manager only.

## Discord (the app on the PC)
1. **Voice controls** (clean, official): the Discord tile and voice commands for mute, deafen, who's talking, leave call, over Discord's **local RPC**. Groovy asks you to authorise it once in Discord. Registering a free Discord app (client id) is needed for this; no secret is committed.
2. **Reading messages:** Groovy reads Discord's **Windows notifications** (sender, server or DM, text) and shows them on the tile. No account access.
3. **"Message this person":** Groovy opens Discord, uses its **quick switcher (Ctrl+K)** to find the person or channel by name, **pastes the drafted text, and presses Enter only after you confirm**. This is keyboard automation of the real Discord window, like a person typing: not using the account's API, but still a grey area under Discord's terms, so it's **opt-in, one message at a time, with confirmation**, and off by default.
4. Optional: a Groovy **bot** for servers the owner runs.

## Instagram (messages only)
- **Honest limits:** Instagram has **no official way for a personal account to read or send DMs from another app.** Meta's messaging API only works for **Business/Creator accounts** linked to a Facebook Page and an approved Meta app, so it's not the "just like Spotify" route. As far as I know Meta also retired its Windows desktop app, so on a PC Instagram DMs live in **instagram.com in the browser** (DuckDuckGo).
- **What Groovy can do without breaking the rules:**
  1. **Show new DMs** from the browser's Windows notifications (turn on Instagram notifications in the browser): sender and text on an island tile.
  2. **Open the right conversation** in the browser from the tile.
  3. **Draft the reply** (Groovy writes it from what you say) and, **after you confirm**, type and send it **in your own signed-in browser tab** using UI automation. Same grey area as Discord's: opt-in, one at a time, confirmation required, and Meta's terms forbid automation, so there is some risk to the account. The owner decides if that's acceptable.
- **If the owner has a Business/Creator account:** the official messaging API is possible later (needs a Meta developer app and review), and would allow proper reading and replying.

## How a voice command would run
"OK Groovy, message Sam on Discord I'll be 10 minutes late" → Groovy drafts "I'll be 10 minutes late" → island shows "To Sam: I'll be 10 minutes late [Send] [Edit] [Cancel]" → on Send, Groovy opens the app, finds Sam, pastes, sends, and says "Sent". The same flow for Instagram in the browser.

## Memory and CPU
Lazy (round 3D item 9): nothing connects or listens until a Discord/Instagram tile is on the island or a command is used; notification listening reuses the listener Groovy already has for mail.

## Rounds
"Round 6: Discord" (voice controls, notifications, the confirm-and-send flow), then "Round 7: Instagram" (notification tile, open-conversation, confirm-and-send). Both come after the current queue and need the owner's approval first.
