// Entry point: boot the bridge, wire the island, start the greeting.

import "./style.css";
import { Bridge, IS_TAURI, onEvent, type MailInbox, type RunState } from "./core/bridge";
import { dropSprites } from "./mochi/sprites";
import { Sound } from "./core/sound";
import { installErrorReporting } from "./views/toast";
import { Mail, wantsMail, type MailNote } from "./views/tiles";
import { currentTiles, isMailNotification } from "./core/tiles";
import { State, type Settings } from "./core/state";
import { Island } from "./island/island";
import { migrateSettingsOutfit } from "./mochi/appearance";
import { mergeGrowth, migrateGrownLook, startGrowthClock } from "./mochi/growth";
import { registerHookHandlers } from "./island/hooks";
import { registerPhoneLink } from "./island/phone-link";
import { registerIntegrationHandlers, refreshConfigured } from "./island/integrations";
import { registerShortcutHandlers } from "./island/shortcuts";
import { registerVoiceHandlers } from "./island/okgroovy";
import { registerLiveHandlers } from "./island/live";
import { applySpotify, registerSpotifyHandlers } from "./island/spotify";
import { SPOTIFY_ID, Spotify } from "./core/spotify";
import { Recap } from "./recap/recap";
import { loadLanguage, onLanguageChange, resolveLanguage, setLanguage, systemLanguages } from "./i18n/i18n";
import { glassAlpha, glassStrength, type GlassLook } from "./core/glass";
import { frameStats, frameTour, frameVerdict, type TourStep } from "./core/frametest";

/** Shows the language Settings asks for ("" = the system's, when Groovy has it). */
function applyLanguage() {
  setLanguage(resolveLanguage(State.settings.language, systemLanguages()));
}

async function main() {
  installErrorReporting("notice");
  const root = document.getElementById("root");
  if (!root) return;

  void Sound.preload();

  // Before the island exists: its canvases size their bitmaps from the pixel
  // ratio once, so the zoom correction has to land first. If zoom was adjusted,
  // wait until the ratio updates (or up to 500ms) so nothing is sized against
  // the stale value.
  const dprBefore = window.devicePixelRatio || 1;
  const zoomChanged = await Bridge.fitZoom(dprBefore);
  if (zoomChanged) {
    for (let i = 0; i < 20 && window.devicePixelRatio === dprBefore; i++) {
      await new Promise((r) => setTimeout(r, 25));
    }
  }

  const island = new Island(root);

  const boot = await Bridge.boot();
  if (boot) {
    State.settings = { ...State.settings, ...boot.settings };
    // Round 3: the old single outfit becomes a Head/Face/Neck layer, once.
    if (migrateSettingsOutfit(State.settings)) void Bridge.saveSettings(State.settings);
    // Grove, Blaze and Tide are now Night, Ember and Frost fully grown, once.
    if (migrateGrownLook(State.settings)) void Bridge.saveSettings(State.settings);
  }
  // Groovy's look grows with the time he runs here (round 3 item 6).
  startGrowthClock(
    () => State.settings.growth,
    (g, grew) => {
      State.settings.growth = g;
      if (grew) State.notify();
    },
    (g) => void Bridge.saveGrowth(g),
  );
  // A language change redraws the island in place: the texts given as tl(…)
  // relabel themselves (views/dom.ts) and the views redraw the rest on this
  // sync. Nothing is rebuilt, so tasks, steps and the chat stay as they are.
  onLanguageChange(() => State.notify());
  // Round 3F item 6: the language's file is in before the first draw.
  await loadLanguage(resolveLanguage(State.settings.language, systemLanguages()));
  applyLanguage();
  // Rust shows the tray and its errors in the same language as the webview.
  void Bridge.setSystemLanguages(systemLanguages());
  island.applySettings();
  State.loadIntegrationTasks();
  if (boot && !boot.cursorPoll) island.followPageCursor();
  await island.desktop.init();

  await onEvent<{ x: number; y: number }>("cursor", ({ x, y }) => island.onCursor(x, y));
  await onEvent<boolean>("pointer-inside", (inside) => island.setPointerInside(inside));

  /** Pause has to reach Rust too, or the pollers keep calling out. */
  const setPaused = (on: boolean) => {
    if (State.paused === on) return;
    State.paused = on;
    void Bridge.setPaused(on);
  };

  await onEvent<string>("tray", (what) => {
    switch (what) {
      case "settings":
        setPaused(false);
        island.alert("settings");
        break;
      case "open":
        setPaused(false);
        island.alert(State.defaultView());
        break;
      case "recap":
        setPaused(false);
        void Recap.open(island);
        break;
      case "wardrobe":
        setPaused(false);
        island.alert("wardrobe");
        break;
      case "pause":
        setPaused(!State.paused);
        if (State.paused) island.fsm.forceHidden();
        else island.reveal();
        break;
    }
  });

  const checkZoom = async () => {
    const dpr = window.devicePixelRatio || 1;
    await Bridge.fitZoom(dpr);
  };

  await onEvent<null>("screen-changed", async () => {
    await checkZoom();
    void Bridge.reposition();
  });

  const mediaQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
  mediaQuery.addEventListener?.("change", () => void checkZoom());

  window.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey) e.preventDefault();
    },
    { passive: false }
  );
  window.addEventListener("keydown", (e) => {
    if (e.ctrlKey && (e.key === "+" || e.key === "-" || e.key === "=" || e.key === "0")) {
      e.preventDefault();
    }
  });

  // Glass (Windows' Acrylic behind the island) or solid: Rust decides from the
  // setting, Windows' transparency switch and battery saver (glass.rs).
  // "blur": Windows blurs behind it; "css": the page's own glass, no blur
  // (round 3B item 1: never a grey square around it).
  // How see-through it is: the Glass strength setting, raised a little over a
  // very bright backdrop (round 3B item 1b).
  let glassLook: GlassLook = "solid";
  let glassBoost = 0;
  const paintGlass = () => {
    const a = glassAlpha(glassLook, glassStrength(State.settings.glassStrength), glassBoost);
    document.body.style.setProperty("--glass-a", String(a));
  };
  const setGlass = (look: string) => {
    glassLook = look === "blur" || look === "css" ? look : "solid";
    document.body.classList.toggle("glass", glassLook !== "solid");
    document.body.classList.toggle("glass-css", glassLook === "css");
    paintGlass();
  };
  setGlass((await Bridge.islandGlass()) ?? "solid");
  await onEvent<string>("island-glass", setGlass);
  await onEvent<number>("island-glass-boost", (b) => {
    glassBoost = b;
    paintGlass();
  });

  // Settings → Reload sounds: read the sounds folder again, and let them hear it.
  await onEvent<null>("sounds-changed", () => {
    void Sound.reload().then(() => Sound.play("pop"));
  });

  // The settings window writes preferences; apply them here without a restart.
  await onEvent<Settings>("settings-changed", (s) => {
    const previousMain = State.mainPillId;
    const growth = mergeGrowth(State.settings.growth, s.growth);
    State.settings = { ...State.settings, ...s, growth };
    applyLanguage();
    island.applySettings();
    paintGlass();
    State.loadIntegrationTasks();
    // A new main tool comes to the front, as on macOS.
    if (State.mainPillId !== previousMain) State.setFocus(State.mainPillId);
    void refreshConfigured();
  });

  registerHookHandlers(island);
  registerPhoneLink(island);
  registerIntegrationHandlers(island);
  registerShortcutHandlers(island, () => setPaused(false));
  registerVoiceHandlers();
  // Dictation, calls being recorded, « OK Groovy » and Groovy's messages (flow/live.rs).
  void registerLiveHandlers(island);
  // "My island": a new mail from Windows' notifications pops up when the island has a Mail tile.
  await onEvent<MailNote>("mail-notification", (n) => {
    if (!wantsMail() || !isMailNotification(n.app, currentTiles(State.settings).mailApps)) return;
    Mail.push(n);
    island.alert("notify");
  });
  // Connected mail accounts (mail/): the tile's unread count and the Mail panel.
  // Standby, Conversation or Offline (round 3D item 10): shown on the island's rim.
  const showState = (s: RunState) => {
    document.body.dataset.run = s;
  };
  await onEvent<RunState>("runtime-state", showState);
  void Bridge.runtimeState().then((r) => r && showState(r.state));
  // Back in Standby: what the page holds only for a conversation goes.
  await onEvent<null>("runtime-release", () => window.dispatchEvent(new Event("groovy-release")));
  await onEvent<MailInbox[]>("mail-inboxes", (inboxes) => {
    Mail.inboxes = inboxes;
    State.notify();
  });
  void Bridge.mailInboxes().then((inboxes) => {
    if (inboxes) {
      Mail.inboxes = inboxes;
      State.notify();
    }
  });
  registerSpotifyHandlers(island);
  // Rust may have read Spotify before this page listened: ask once.
  if (State.settings.activeIntegrations.includes(SPOTIFY_ID)) {
    void Bridge.spotifyRefresh().then((s) => s && applySpotify(island, s));
  }

  // Monday recap: app start (greeting over), an agent starting work, waking up.
  const checkRecap = () => void Recap.check(island);
  island.onGreetingDone = checkRecap;
  island.onWake = checkRecap;
  await onEvent<null>("recap-check", checkRecap);

  // Near Groovy's 400 MB ceiling (resources/ceiling.rs): the cached pictures go.
  void onEvent<number>("memory-pressure", () => dropSprites());
  island.launch();
  if (boot?.smokeView === "frames") void frameTest(island);
  else if (boot?.smokeView === "peak") peakHold(island);
  else if (boot?.smokeView) holdForSmokeTest(island, boot.smokeView);

  // In a plain browser there is no wake strip behind the cursor: make the whole
  // page wake the island so the visuals can be checked with `npm run dev`.
  if (!IS_TAURI) {
    document.addEventListener("click", () => Sound.resume(), { once: true });
    // For checking the views in a browser (`npm run dev`): open any of them.
    (window as unknown as { groovy: unknown }).groovy = { island, State, Spotify };
  }
}

/**
 * CI's smoke test (scripts/corner-test.ps1) photographs the island's corners:
 * hold it open on the tiles ("home"), or with a pretend track and the Spotify
 * tile grown to 4 ("player", round 3D item 8: the island at its grown size).
 */
/**
 * CI's frame-time test (round 3B item 8b): F → E → Spotify and Mail grown and
 * folded (round 3D item 8) → the detail card → quick settings → the wardrobe
 * → F, each held 1.2 s, every frame timed; then the
 * island folds away so CI can read the idle CPU, and the frames go to Rust.
 */
async function frameTest(island: Island) {
  const track = { id: "spotify:track:frames", title: "Frame test", artist: "Groovy", album: "", duration: 200, artUrl: null };
  const music = (playing: boolean) => {
    Spotify.state = playing
      ? { ...Spotify.state, running: true, installed: true, playing: true, track, position: 10, positionAt: Date.now() }
      : { ...Spotify.state, playing: false };
    State.notify();
  };
  await new Promise((r) => setTimeout(r, 4000));
  island.setPinned(true);
  const steps: TourStep[] = [
    { name: "F", run: () => island.alert("home") },
    { name: "E", run: () => music(true) },
    // Round 3D item 8: Spotify grows to 4 tiles, then Mail to 6 beside the music strip.
    { name: "grown Spotify", run: () => { State.homeExpanded = "music"; State.notify(); } },
    { name: "grown Mail", run: () => { State.homeExpanded = "mail"; State.notify(); } },
    { name: "folded", run: () => { State.homeExpanded = null; State.notify(); } },
    { name: "detail card", run: () => island.setView("overview") },
    { name: "quick settings", run: () => island.setView("settings") },
    { name: "wardrobe", run: () => island.setView("wardrobe") },
    { name: "back to F", run: () => { music(false); island.setView("home"); } },
  ];
  const perStep = await frameTour(steps, 1200);
  const all = Object.values(perStep).flat();
  const stats = frameStats(all);
  const report = {
    stats,
    verdict: frameVerdict(stats),
    steps: Object.fromEntries(Object.entries(perStep).map(([k, v]) => [k, frameStats(v)])),
    deltas: perStep,
    refresh: Math.round(1000 / (stats.p95 || 16.7)),
  };
  island.setPinned(false);
  island.collapse();
  await Bridge.frameReport(report);
}

/**
 * CI's peak test (round 3D item 7, scripts/peak-test.ps1): the island held
 * open and turned every 3 s through its grown tiles (Spotify, Mail with the
 * music strip, This PC, Reminders) and the wardrobe, while Rust runs the
 * rest of the scenario.
 */
function peakHold(island: Island) {
  const track = { id: "spotify:track:peak", title: "Peak test", artist: "Groovy", album: "", duration: 200, artUrl: null };
  // Round 3D item 8: the grown tiles are the island's heaviest views.
  const grow = (k: "music" | "mail" | "system" | "reminders") => () => {
    island.alert("home");
    State.homeExpanded = k;
  };
  const views = [grow("music"), grow("mail"), grow("system"), grow("reminders"), () => island.setView("wardrobe")];
  let i = 0;
  window.setTimeout(() => {
    island.setPinned(true);
    window.setInterval(() => {
      Spotify.state = { ...Spotify.state, running: true, installed: true, playing: true, track, position: 10, positionAt: Date.now() };
      views[i++ % views.length]();
      State.notify();
    }, 3000);
  }, 4000);
}

function holdForSmokeTest(island: Island, view: "home" | "player") {
  const track = { id: "spotify:track:smoke", title: "Smoke test", artist: "Groovy", album: "", duration: 200, artUrl: null };
  window.setTimeout(() => {
    island.setPinned(true);
    // Held there: a notice (the AI choice, on a fresh install) doesn't move it.
    window.setInterval(() => {
      if (view === "player" && !(Spotify.state.playing && Spotify.state.track?.id === track.id)) {
        Spotify.state = { ...Spotify.state, running: true, installed: true, playing: true, track, position: 10, positionAt: Date.now() };
        State.notify();
      }
      if (State.mode !== "expanded" || State.view !== "home") island.alert("home");
      if (view === "player" && State.homeExpanded !== "music") {
        State.homeExpanded = "music";
        State.notify();
      }
    }, 500);
  }, 4000);
}

void main();
