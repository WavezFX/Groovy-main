# Fixes round 3C: the owner tested 0.6.0 (16 Oct, evening)

**Do these next**, after the item you're on now. Commit each one separately with "Fixes round 3C, item N: …" as the subject, tick it in `WORK_QUEUE.md` in the same commit, push, and keep the build green.

## Item 1: Real frosted glass: translucent and blurred

Right now the island is **clear, not frosted**. The owner can read the browser tabs and "github.com" sharply through it, because the corner fix (`f9db93b`) fell back to the page's glass with **no blur**. He wants a **translucent surface with a background blur**, like Windows 11's own flyouts or iOS glass.

- **Do the blur the Windows 11 way, clipped to the island's shape:**
  - put a DirectComposition / `Windows.UI.Composition` visual tree under the WebView: a `HostBackdropBrush` (blur behind the window), a soft tint and a little noise;
  - **clip it to a rounded rectangle** (`CompositionRoundedRectangleGeometry`) that matches the island's CSS shape and radius at the monitor's DPI;
  - draw the WebView on top with a transparent background. Use the `windows` crate (`Windows.UI.Composition`, `ICompositorDesktopInterop`, a `DesktopWindowTarget`), on a window without a redirection bitmap if needed.
  - This way the blur can **never** spill into the corners, and the window region is only needed for hit-testing.
- **When it animates,** the clip geometry follows the island's shape, using the same motion module as round 3B item 8 (animate the geometry with composition animations, not per-frame Rust calls).
- **The look (Balanced):** a strong blur (about 30 px or more), tint `rgba(20,22,28,0.45)`, a 1 px light rim `rgba(255,255,255,0.18)`, a soft top highlight and slight saturation. **Text behind it must not be readable**, only colours and shapes.
  - Glass strength settings: Clear is lighter, Frosted heavier.
  - Solid is opaque.
- **Fallback:** if host backdrop isn't available (Windows 10, transparency effects off, battery saver), use the accent blur *if it stays inside the shape*. Otherwise use an opaque-enough tint (alpha about 0.80) so it stays readable. Never use a clear surface with no blur.
- **Readability:** all text and icons on the island must pass contrast (4.5:1 for text) over a busy white browser page and over a dark wallpaper.
- **Test:** extend `corner-test.ps1`.
  - Draw a **high-contrast black-and-white text pattern** on the desktop, then show the island over it.
  - The pixels inside the island must be **blurred**: low local variance compared with the same area without the island. The corners must be untouched.
  - Upload the screenshots.

## Item 2: Hover opens the island; leaving closes it

Today he has to **click** Groovy to open the island, but leaving it closes it.

- **Hovering** over the closed island or Groovy opens it after about **150 ms** (a short intent delay, so passing the mouse across it doesn't trigger it).
- **Leaving** it closes it after the close-timing setting (round 4 item 2: When I leave / 1 / 2 / 3 / 5 s / Click away). Moving back in cancels the close.
- **Clicking** still works.
- The open and close use the same morph animation (round 3B item 8).
- **Full-screen apps and games:** never hover-open while a full-screen app has focus (keep the existing quiet rules).
- **Setting:** "Open on hover" (on by default) / "Open on click".

## Item 3: Bigger icons and controls

The **Spotify buttons** (previous, play/pause, next) are tiny, and so are other icons.

- Every clickable control on the island needs at least a **32 × 32 px hit area** (at 100% scaling) with a **20 px icon**. The play/pause button should be the biggest: about 40 px with a 24 px icon.
- **Tile icons:** 20–24 px. Labels should be at least 12 px, never cut off mid-word (use two lines or a shorter label).
- Add a hover state and a pressed state (scale 0.95) on every control. Tooltips should be simple and quick to appear.
- Scale everything correctly at 125%, 150% and 200% display scaling.

## Item 4: Tiles open in the island ("soft settings"); Settings is the "hard settings"

Today, clicking **Mail**, **Reminders**, **This PC** and similar tiles just opens the full Settings window. Instead, **clicking a tile expands it inside the island** into a bigger panel that covers the tile grid. Do the task there, then it folds back into the tile. Settings is only for deep setup ("All settings" link at the bottom of each panel).

The same pattern applies to every tile: it opens into its own panel with a back arrow, Esc or a click-away to close, and it's animated.

- **Reminders:**
  - a list of upcoming reminders (tick to complete);
  - an "Add reminder" field with natural-language time ("tomorrow 9am", "in 20 minutes"), using a quick date/time picker when the text can't be understood;
  - **Save** → it folds back, and the tile shows the next reminder.
  - Editing and deleting happen inline.
  - Notifications use the existing reminder system.
- **This PC:** a small live panel showing CPU, memory, disk, battery and network, plus quick toggles that are safe behind round 4 item 5's permissions (volume, brightness, Wi-Fi/Bluetooth on or off if allowed, do not disturb). There's a "Top apps by memory" list with an **End** button that asks first. Update it every 2 s **only while the panel is open**.
- **Mail:** an inbox preview (sender, subject, time) for each connected account, with unread counts. Click an email to open it in the web or app mail client. Add "Mark read" and "Archive" where the account allows it. A **Connect account** button opens the connect flow in item 5. With no accounts connected, the panel shows the connect buttons, not Settings.
- **Spotify:** the full player: art, scrub bar, volume and the big controls from item 3.
- **Claude Code / agents** and the other integrations: their status, plus their most useful one-click actions.
- **Quick settings** (round 4 item 2) is the same kind of panel, opened from the gear.

## Item 5: Connect real mail accounts

The owner wants to connect **Gmail** and the mail he uses on his **iPhone** (iCloud Mail, and possibly Outlook).

- **Gmail:**
  - Google OAuth in the default browser (DuckDuckGo), with the Gmail API, read-only plus modify scopes for "mark read" and "archive". Tokens go in Windows Credential Manager.
  - Note for the owner in `STATUS.md`: the Google Cloud project needs the Gmail API turned on, and it stays in testing mode (up to 100 test users) until Google verifies it.
- **Outlook / Hotmail:** Microsoft OAuth with Microsoft Graph Mail.
- **iCloud Mail** (the iPhone's mail):
  - IMAP (`imap.mail.me.com:993`) with the user's Apple ID email and an **app-specific password**. Show a short "How to make an app-specific password" link (appleid.apple.com → Sign-In and Security → App-Specific Passwords).
  - The password goes in Credential Manager.
- **Any other provider:** IMAP with a server, username and password (or app password).
- **Fetching:**
  - Check for new mail every 2–5 minutes (IMAP IDLE where supported).
  - Show a small toast for new mail from important senders (optional).
  - Show the unread count on the Mail tile.
  - Groovy only reads headers and snippets unless the user opens an email in the panel.
- **Privacy:** nothing leaves the PC except the calls to the mail providers. Each account can be disconnected, which deletes its tokens and cached mail.
- **"OK Groovy, any new emails?"** reads out the count and the top senders, using the same data.

## Item 6: Settings must animate open and closed

Settings **still just appears**. Round 3B item 8 asked for it to grow out of the island, but the owner doesn't see it.

- **Opening** (from the gear, a tile's "All settings", or the tray): the Settings window starts **hidden and transparent**, is created off-screen or with zero opacity, and is shown only after its first paint. Then it animates from the island's position and size to its final place, scaling and fading in over about 320 ms. The content slides in after it.
- **Closing** reverses it, back into the island, and then destroys the window (round 3B item 10).
- **No white flash:** set the WebView2 default background to transparent or the theme colour, and don't show the window before the first paint.
- **Respect reduce-motion:** use a fade instead.
- **Test:** CI opens Settings and records the window's opacity and bounds over time. It fails if the window is fully visible in the first 50 ms, or if there's no change in size or opacity during the opening.

## Item 7: Checks

- All tests green.
- Idle memory under 300 MB and CPU close to 0 (round 3B item 10's realistic test).
- The frame-time test passes with the blur on (blur must not cause lag).
- The corner and blur test passes.
