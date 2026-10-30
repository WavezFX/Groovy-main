# Discord on Groovy: plan (not started; owner to approve)

**Goal:** do everyday Discord things from the island: see and reply to messages, mute and deafen yourself, see who's talking, and join or leave voice, without breaking Discord's rules or risking the owner's account.

## What Discord allows

- **Discord's local RPC** (talking to the Discord app running on the PC over its local IPC pipe):
  - Rich Presence, which needs no permission.
  - With an **authorised app** (the user approves Groovy once in Discord), it can read and **set voice settings**: **mute, deafen**, input and output volume, and push-to-talk. It can also see the current voice channel and **who's speaking**, and select or leave a voice channel.
  - Some of these RPC scopes have historically been restricted by Discord to approved apps. Check the current rules with a test app before building, and apply for access if needed.
- **Reading and replying to your DMs and servers as yourself:** Discord offers **no official API** for user accounts. Automating a user account ("self-bot", using the user token) is **against Discord's Terms of Service and can get the account banned**. **Groovy must never do this.**
- **Bots:** a Discord bot can read and reply in **servers where it's added**, but it posts as the bot, not as you, and can't see your DMs.

## Proposed Groovy features (all within the rules)

1. **Discord tile on the island:**
   - shows your voice status: muted, deafened, current channel and who's speaking;
   - big **Mute** and **Deafen** buttons;
   - **Leave call**;
   - all through local RPC once you authorise Groovy.
2. **Voice commands:** "OK Groovy, mute Discord", "deafen", "who's in the call?"
3. **Messages:**
   - Groovy reads **Discord's Windows notifications** (the same notification listener Groovy already uses for mail) and shows them on the tile: sender, server or DM, and the text Discord put in the notification.
   - **Reply** opens Discord straight to that conversation (a `discord://` deep link) with focus in the message box. Groovy can **draft** the reply text (copied to the clipboard, or typed with permission through PC control in round 4 item 5), and the user presses Enter.
   - No user-token automation.
4. **Optional server bot:** for servers the owner runs, a Groovy bot can post announcements or read a channel. It's set up separately with the owner's bot token, stored in Credential Manager.
5. **Memory:** the Discord connection is lazy (round 3D item 9). It only connects while the tile is visible or a Discord command is used, and closes after that.

## Next step

The owner approves this approach. Then add it as its own round (for example "Round 6: Discord") with tests, after the current queue.
