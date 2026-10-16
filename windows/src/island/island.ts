// The island: DOM shell, sizing animation, Groovy placement, mouse handling.
// Mirrors IslandRootView.swift + IslandWindowController.swift.

import { parseGrowth, progressLine, shownStage } from "../mochi/growth";
import { flavorOf } from "../core/sounds-synth";
import { showError } from "../views/toast";
import { Tracked, Spring, clamp } from "../core/anim";
import { Bridge, IS_TAURI, onDragDrop, onEvent, type TaskItem } from "../core/bridge";
import {
  EXPANDED_CORNER, EXPANDED_W, LIVE_STRIP_H, LIVE_STRIP_W, NOTCH_W, PANEL_H, PANEL_W,
  ROUNDED_CORNER, botGlowColor, botGlowOpacity, botPosition, chatPromptHeight,
  islandSize,
  QUESTION_PICKER_H,
  type BotEmoteName, type IslandMode, type IslandViewName,
} from "../core/layout";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import { SPOTIFY_ID, islandDances } from "../core/spotify";
import { BotEngine } from "../mochi/engine";
import { SEED_ISLAND, SeedIntro, seedKind } from "../mochi/seed";
import { MOTION, duration, easeOut } from "../core/motion";
import { DebouncedSave, chooseLook, shownLook } from "../core/lookchoice";
import { createMiniBot, pruneMiniBots, syncMiniBotStates, tickMiniBots } from "../mochi/minibots";
import { parseSkin } from "../mochi/skins";
import { clearSlot, currentWardrobe, tryOn, wear } from "../mochi/appearance";
import { VoiceHost } from "./voice";
import { Notices, liveBusy } from "./live";
import { buildLiveBar } from "../views/livebar";
import { BreakControl, Breaks, BREAKS, ReminderControl, snoozeReminder } from "./breaks";
import { speak } from "../voice/speak";
import { t } from "../i18n/i18n";
import { UploadCanvas } from "../upload/canvas";
import { USC, UploadSeq } from "../upload/sequence";
import { closePlanCard } from "../views/usage";
import { buildHeader, buildViews, type ViewActions, type ViewHost } from "../views/views";
import { h } from "../views/dom";
import { IslandStateMachine } from "./fsm";
import { refreshHookPills } from "./integrations";
import { DesktopLink } from "./desktop";
import type { ViewCommand } from "./shortcuts";
import { DRAG_THRESHOLD } from "../mochi/desktop-logic";

const BOT_OVERHANG = 40;
const CLAUDE_DESKTOP_ID = "agent_claude-desktop";
/** Extra canvas on each side of Groovy, for the witch hat's brim and the Santa hat's tip. */
const BOT_SIDE = 24;
/** One frame in ten (10 fps) for his looping animation in the small island. */
const SLOW_FRAME_MS = 100;
/** A look try-on ends by itself after this, should its mouseleave be lost. */
const LOOK_PREVIEW_MS = 4000;
/** Same margin as the Rust hit test (src-tauri/src/island.rs). */
const HIT_MARGIN = 14;

/** The three views the drop sequence owns; leaving them stops the engine. */
const UPLOAD_VIEWS: ReadonlySet<IslandViewName> = new Set(["upload", "uploading", "choose"]);

/** Seconds between the drop and the moment the progress bar starts filling. */
const PRE_PROGRESS = USC.T_PROG_START - USC.T_DROP;

export class Island {
  readonly fsm = new IslandStateMachine();
  /** Groovy on the desktop: his life cycle and the drag out of the island. */
  readonly desktop: DesktopLink;

  private root: HTMLElement;
  private islandEl!: HTMLElement;
  private clipEl!: HTMLElement;
  private contentEl!: HTMLElement;
  private viewsEl!: HTMLElement;
  private botCanvas!: HTMLCanvasElement;
  private botGlow!: HTMLElement;
  private greetingCanvas!: HTMLCanvasElement;
  private miniGrid!: HTMLElement;
  private countdown!: HTMLElement;
  private wakeStrip!: HTMLElement;

  private header!: ViewHost;
  private views!: Map<IslandViewName, ViewHost>;
  private uploadCanvas!: UploadCanvas;

  private width = new Tracked(NOTCH_W);
  private height = new Tracked(0);
  private radius = new Tracked(ROUNDED_CORNER);
  private botCx = new Spring(46);
  private botCy = new Spring(16);
  private botSize = new Spring(10);

  private engine = new BotEngine();
  /** Held open by a brain setup or a voice call (see the update loop). */
  private holding = false;
  private voice = new VoiceHost(this.engine, { openChat: () => this.alert("prompt") });
  private breaks = new Breaks({
    show: () => {
      Sound.play("greet");
      this.engine.triggerEmote("happy", 1.5);
      this.alert("note");
      const kind = State.breakDue;
      if (kind && State.settings.voiceEnabled && State.settings.voiceSpeak) {
        void speak(`${t(BREAKS[kind].title)} ${t(BREAKS[kind].detail)}`, State.settings.voiceTts);
      }
    },
  });
  /** Look choices are saved once after 300 ms of quiet (round 3B item 4). */
  private lookSave = new DebouncedSave(() => void Bridge.saveSettings(State.settings));
  private lookPreviewTimer: number | null = null;

  /** The launch intro ("Seed"): made at launch, let go when it ends. */
  private intro: SeedIntro | null = null;
  private introUsed = false;
  private greetingShown = false;

  private running = false;
  private lastFrame = 0;
  private dirty = true;
  private canvasPx = 0;

  // Rust starts the window at full size so the launch greeting has room.
  private collapsed = false;
  private collapseTimer: number | null = null;
  private wasInIsland = false;
  /** Last shape handed to Rust for the click-through test. */
  private pushedRect = { x: -1, y: -1, w: -1, h: -1, r: -1 };

  // Bot hover → love (IslandWindowController.botHoverIn)
  private botHovering = false;
  private botHoverTimer: number | null = null;
  private lastLoveTime = 0;
  private botHoverStart = { x: 0, y: 0 };

  private confusedRecovery: number | null = null;
  private prevViewBeforeConfused: IslandViewName = "overview";
  private lastSyncedView: IslandViewName | null = null;

  /** The launch greeting ended, or the island came out of hidden — two of the
   *  moments the Monday recap may open (see src/recap/recap.ts). */
  onGreetingDone: (() => void) | null = null;
  onWake: (() => void) | null = null;

  /** Where a press on Groovy started: moving past DRAG_THRESHOLD drags him out. */
  private botPress: { x: number; y: number } | null = null;

  /** The next reveal from hidden makes no peek (music starting, as on macOS). */
  private silentReveal = false;

  /** Drop sequence bookkeeping: last tick played, and whether the ✓ has fired. */
  private uploadTens = 0;
  private uploadDone = false;

  constructor(root: HTMLElement) {
    this.root = root;
    this.desktop = new DesktopLink({
      reveal: () => this.reveal(),
      wardrobeFromDesktop: () => this.wardrobeFromDesktop(),
      dizzyFromDesktop: () => this.handleDizzy(),
    });
    this.build();
    this.wireFsm();
    this.wireInput();
    this.engine.onDizzy = () => this.handleDizzy();
    this.engine.onWake = () => this.ensureRunning();
    State.subscribe(() => {
      this.dirty = true;
      this.ensureRunning();
      this.voice.sync();
      this.syncLive();
    });
    this.voice.sync();
    BreakControl.done = () => {
      this.breaks.done();
      Sound.play("pop");
      this.setView(State.defaultView());
    };
    BreakControl.snooze = () => {
      this.breaks.snooze();
      this.setView(State.defaultView());
    };
    // Reminders come due in Rust (tasks.rs), once each.
    void onEvent<TaskItem>("reminder", (task) => this.showReminder(task));
    const endReminder = () => {
      State.reminderDue = null;
      State.notify();
      this.setView(State.defaultView());
    };
    ReminderControl.done = () => {
      const r = State.reminderDue;
      if (r) void Bridge.taskSetDone(r.id, true).catch(showError);
      Sound.play("pop");
      endReminder();
    };
    ReminderControl.snooze = () => {
      const r = State.reminderDue;
      if (r) void snoozeReminder(Bridge, r).catch(showError);
      endReminder();
    };
  }

  /** A reminder came due: Groovy pops out with it, and says it when voice is on. */
  private showReminder(task: TaskItem) {
    if (State.pendingApproval || State.voiceCall) {
      // Never on top of something that needs you: it waits a minute.
      setTimeout(() => this.showReminder(task), 60_000);
      return;
    }
    State.breakDue = null;
    State.reminderDue = { id: task.id, text: task.text };
    State.noteMessage = task.text;
    Sound.play("greet");
    this.engine.triggerEmote("happy", 1.5);
    this.alert("note");
    if (State.settings.voiceEnabled && State.settings.voiceSpeak) {
      void speak(`${t("Reminder")}: ${task.text}`, State.settings.voiceTts);
    }
    State.notify();
  }

  private noticeTimer: number | null = null;

  /** The badge and the live strip while Groovy listens (views/livebar.ts). */
  private liveBar = buildLiveBar({ setStrip: (on) => this.setLiveStrip(on) });
  /** The strip instead of the badge, for what is listening now. */
  private liveStrip = false;
  private liveShown: string | null = null;

  /** The strip shows (compact island) rather than the badge. */
  private stripActive(): boolean {
    return this.liveStrip && this.liveShown !== null && State.mode === "compact";
  }

  setLiveStrip(on: boolean) {
    this.liveStrip = on;
    Sound.play("blip");
    if (on) {
      if (State.mode === "expanded") this.collapse();
      else if (State.mode === "hidden") this.fsm.reveal();
    } else {
      this.alert(State.defaultView());
    }
    this.syncLive();
    this.animateGeometry();
  }

  /** Something started or stopped listening: badge, strip, and staying on screen. */
  private syncLive() {
    const view = this.liveBar.sync(this.liveStrip);
    const kind = view && view.kind !== "message" ? view.kind : null;
    if (kind !== this.liveShown) {
      const started = kind !== null && this.liveShown === null;
      this.liveShown = view ? view.kind : null;
      // Settings → Voice → While listening, show: the badge (default) or the strip.
      if (started) this.liveStrip = State.settings.listeningView === "strip";
      if (!view) this.liveStrip = false;
      if (started && this.fsm.state === "hidden") this.fsm.reveal();
      this.animateGeometry();
    } else if (view) {
      this.liveShown = view.kind;
    }
    document.body.classList.toggle("live-on", view !== null);
    this.liveBar.el.classList.toggle("in-compact", State.mode === "compact");
  }

  /**
   * A message from Groovy (flow/live.rs, what the bottom pill used to say):
   * the island opens on it, unless a card waits for an answer or a call is on,
   * then it waits its turn. It goes by itself after its time, not while the
   * pointer is on the island.
   */
  showNotice() {
    const n = Notices.current;
    if (!n) return;
    if (State.pendingApproval || State.voiceCall || State.reminderDue || (State.mode === "expanded" && State.view === "greeting")) {
      window.setTimeout(() => this.showNotice(), 3000);
      return;
    }
    this.alert("notify");
    State.notify();
    if (this.noticeTimer !== null) clearTimeout(this.noticeTimer);
    const expire = () => {
      this.noticeTimer = null;
      if (Notices.current?.id !== n.id) return;
      if (this.wasInIsland) {
        this.noticeTimer = window.setTimeout(expire, 1000);
        return;
      }
      Notices.dismiss(n.id);
      if (Notices.current) this.showNotice();
      else if (State.mode === "expanded" && State.view === "notify") this.collapse();
    };
    this.noticeTimer = window.setTimeout(expire, Math.max(3, n.seconds) * 1000);
  }

  /** The request has its answer: the card goes and the session carries on. */
  private closeApproval() {
    State.endApproval();
    this.fsm.pinned = false;
    this.setView(State.defaultView());
  }

  /**
   * Folds a card that is waiting for an answer down to the compact island,
   * without answering it (Mac #290). Nothing is decided: the request keeps
   * waiting, the island stays on screen, and opening it shows the card again.
   */
  foldApproval() {
    if (!State.pendingApproval || State.mode !== "expanded") return;
    State.isPinned = true;
    this.fsm.pinned = true;
    this.fsm.forcePetit();
  }

  // ── DOM ─────────────────────────────────────────────────────────────────────

  private build() {
    const actions: ViewActions = {
      setView: (v) => this.setView(v),
      cancelDrop: () => this.discardDrop(),
      collapse: () => this.collapse(),
      foldApproval: () => this.foldApproval(),
      setFocus: (id) => {
        State.setFocus(id);
        Sound.play("blip");
        // A pill with a waiting request opens on its card: going back to it
        // after looking at another pill brings the card up again.
        const req = State.pendingApproval;
        if (req?.pillId === id) this.setView(req.questions ? "question" : "approval");
      },
      openTerminal: () => {
        const task = State.focusTask;
        // Sessions from the Claude desktop app live there, not in a terminal.
        if (task?.id === CLAUDE_DESKTOP_ID) void Bridge.openClaudeDesktop();
        else void Bridge.openSession(task?.sessionId ?? null, task?.sessionCwd ?? null);
      },
      // The ↗ button — same targets as openAgentTarget() on macOS.
      openTarget: () => {
        const task = State.focusTask;
        if (!task) return;
        const urls: Record<string, string> = {
          integration_resend: "https://resend.com/emails",
          integration_vercel: "https://vercel.com/dashboard",
          integration_github: "https://github.com/pulls",
          integration_stripe: "https://dashboard.stripe.com/payments",
          integration_notion: "https://notion.so",
          integration_calcom: "https://app.cal.com/bookings",
        };
        if (task.id === CLAUDE_DESKTOP_ID) void Bridge.openClaudeDesktop();
        else if (task.id === "integration_claude" || task.sessionId) {
          void Bridge.openSession(task.sessionId ?? null, task.sessionCwd ?? null);
        } else if (task.id === "integration_n8n") void Bridge.openN8n();
        else if (task.id === SPOTIFY_ID) void Bridge.spotifyOpen();
        else if (urls[task.id]) void Bridge.openUrl(urls[task.id]);
      },
      openUrl: (url) => {
        if (url) void Bridge.openUrl(url);
      },
      decide: (d) => {
        const req = State.pendingApproval;
        void Bridge.log(`decide ${d} req=${req?.requestId ?? "none"}`);
        if (!req) return;
        Sound.play(d === "deny" ? "blip" : "approve");
        void Bridge.approvalDecision(req.requestId, d);
        this.closeApproval();
      },
      answer: (answers) => {
        const req = State.pendingApproval;
        if (!req) return;
        Sound.play("approve");
        void Bridge.approvalAnswer(req.requestId, answers);
        this.closeApproval();
      },
      answerInTerminal: () => {
        const req = State.pendingApproval;
        if (!req) return;
        Sound.play("blip");
        void Bridge.approvalDecline(req.requestId);
        this.closeApproval();
      },
      toggleSound: () => {
        State.settings.soundEnabled = !State.settings.soundEnabled;
        Sound.setEnabled(State.settings.soundEnabled);
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      setVolume: (v) => {
        State.settings.soundVolume = v;
        Sound.setVolume(v);
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      setAutoClose: (s) => {
        State.settings.autoCloseInterval = s;
        this.fsm.homeToPetitDelay = s;
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      openSettingsWindow: () => void Bridge.openSettingsWindow(),
      blip: () => Sound.play("blip"),
      dress: (change) => {
        // Head, face and neck each on their own (round 3): only that slot changes.
        const prefs = currentWardrobe(State.settings.wardrobe);
        const layers = change === "all" || change === "head" || change === "face" || change === "neck"
          ? clearSlot(prefs.appearance.layers, change)
          : wear(prefs.appearance.layers, change).layers;
        State.settings.wardrobe = { ...prefs, appearance: { ...prefs.appearance, layers } };
        State.wardrobePreview = null;
        void Bridge.saveSettings(State.settings);
        Sound.play("pop");
        this.engine.triggerEmote("proud");
        State.notify();
      },
      previewOutfit: (outfit) => {
        State.wardrobePreview = outfit;
        State.notify();
      },
      chooseLook: (look) => {
        // Chosen: the try-on ends, the choice shows at once and is saved once.
        this.clearLookPreview();
        if (parseSkin(State.settings.groovyLook) === look) return;
        chooseLook(State.settings, look);
        this.lookSave.schedule();
        Sound.setFlavor(flavorOf(look));
        Sound.play("pop");
        this.engine.triggerEmote("proud");
        State.notify();
      },
      previewLook: (look) => {
        State.lookPreview = look;
        // A lost mouseleave never pins a try-on: it ends by itself.
        if (this.lookPreviewTimer !== null) window.clearTimeout(this.lookPreviewTimer);
        this.lookPreviewTimer = look ? window.setTimeout(() => this.clearLookPreview(), LOOK_PREVIEW_MS) : null;
        State.notify();
      },
    };

    this.wakeStrip = h("div", { id: "wake-strip" });
    this.botGlow = h("div", { id: "bot-glow" });
    this.botCanvas = h("canvas", { id: "bot-canvas" });
    this.greetingCanvas = h("canvas", { id: "greeting-canvas" });
    this.miniGrid = h("div", { id: "mini-grid" });
    this.countdown = h("div", { id: "countdown" });

    this.header = buildHeader(actions);
    this.views = buildViews(actions, () => this.animateGeometry());
    this.viewsEl = h("div", { id: "views" });
    for (const v of this.views.values()) this.viewsEl.append(v.el);
    this.contentEl = h("div", { id: "content" }, this.header.el, this.viewsEl);

    // The drop sequence draws the card, the bar and its own Groovy. It sits under
    // the header, which stays visible on top of it exactly as on macOS.
    this.uploadCanvas = new UploadCanvas({
      ask: () => {
        State.promptContext = State.droppedFile
          ? { kind: "file", name: State.droppedFile.name, path: State.droppedFile.path }
          : null;
        this.setView("prompt");
      },
      cancel: () => this.discardDrop(),
    });

    this.clipEl = h(
      "div",
      { id: "island-clip" },
      this.greetingCanvas,
      this.uploadCanvas.el,
      this.contentEl,
    );
    this.islandEl = h(
      "div",
      { id: "island" },
      this.clipEl,
      this.botGlow,
      this.botCanvas,
      this.miniGrid,
      this.countdown,
      this.liveBar.el,
    );

    const dpr = Math.min(2, window.devicePixelRatio || 1);
    this.greetingCanvas.width = Math.round(EXPANDED_W * dpr);
    this.greetingCanvas.height = Math.round(150 * dpr);
    this.greetingCanvas.style.width = `${EXPANDED_W}px`;
    this.greetingCanvas.style.height = "150px";

    this.root.append(this.wakeStrip, this.islandEl);
    this.applyGeometry();
  }

  // ── FSM ─────────────────────────────────────────────────────────────────────

  private wireFsm() {
    this.fsm.homeToPetitDelay = State.settings.autoCloseInterval;
    // Typing in the island (the chat, a search) or dictating keeps it open.
    this.fsm.holdOpen = () => {
      const a = document.activeElement as HTMLElement | null;
      const typing = !!a && (a.tagName === "INPUT" || a.tagName === "TEXTAREA" || a.isContentEditable);
      return typing || State.dictating;
    };
    // While Groovy listens, the compact island (with its badge or strip) stays.
    this.fsm.stayVisible = () => liveBusy() || State.voiceCall;
    this.fsm.openOnHover = State.settings.openOnHover;
    this.fsm.keepVisible = State.settings.keepVisible !== false;
    if (this.fsm.keepVisible && this.fsm.state === "hidden" && this.greetingShown) this.fsm.reveal();
    this.fsm.onTransition = (from, to) => {
      // The greeting is over, however it ended: back to his desktop spot.
      if (from === "groovy" && to !== "groovy") this.desktop.launch();
      switch (to) {
        case "hidden":
          this.setMode("hidden");
          break;
        case "petit":
          if (from === "groovy") this.intro?.interrupt();
          else if (from === "hidden" && !this.silentReveal) Sound.play("peek");
          this.setMode("compact");
          if (from === "groovy") State.view = State.defaultView();
          if (!this.wasInIsland) this.fsm.mouseLeft();
          break;
        case "home":
          this.expand(State.defaultView());
          if (!this.wasInIsland) this.fsm.mouseLeft();
          // Hooks may have been installed in a terminal since: the idle cards
          // say so on the next open, without polling while the island is shut.
          void refreshHookPills();
          break;
        case "groovy":
          this.expand("greeting");
          // Once per launch: a second "groovy" goes straight on to the open island.
          if (!this.startIntro()) queueMicrotask(() => this.fsm.greetComplete());
          break;
      }
      State.notify();
      if (from === "hidden") this.onWake?.();
    };
  }

  launch() {
    this.fsm.launch();
  }

  /** Ends a look try-on (the wardrobe closed, a look was chosen, or it timed out). */
  private clearLookPreview(notify = true) {
    if (this.lookPreviewTimer !== null) window.clearTimeout(this.lookPreviewTimer);
    this.lookPreviewTimer = null;
    if (State.lookPreview === null) return;
    State.lookPreview = null;
    if (notify) State.notify();
  }

  /**
   * The Seed intro, once per launch: the first one ever, one with a stage
   * reached since the last launch, or a plain hello; a still fade-in when
   * Windows' animations are off.
   */
  private startIntro(): boolean {
    if (this.introUsed) return false;
    this.introUsed = true;
    const g = parseGrowth(State.settings.growth);
    const reduced = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    const kind = seedKind(g.seen, g.reached, reduced);
    const appearance = currentWardrobe(State.settings.wardrobe).appearance;
    const worn = tryOn(appearance.layers, null);
    const intro = new SeedIntro(kind, {
      skin: parseSkin(State.settings.groovyLook),
      stage: kind === "grew" ? Math.min(g.seen, shownStage(g)) : shownStage(g),
      next: shownStage(g),
      appearance,
      outfit: worn[0] ?? "none",
      layers: worn.slice(1),
      lite: State.settings.assistant.performanceProfile === "lightweight",
    }, kind === "grew"
      ? { title: t("I grew while you were away!"), sub: progressLine(g) }
      : { title: t("Hi, I'm Groovy."), sub: t("I'll grow the longer we work together.") });
    intro.onGrow = () => this.animateGeometry();
    intro.onComplete = () => {
      if (this.intro === intro) this.intro = null;
      // Nothing of it is kept: its canvas gives its pixels back too.
      this.greetingCanvas.width = 1;
      this.greetingCanvas.height = 1;
      this.fsm.greetComplete();
      this.onGreetingDone?.();
    };
    this.intro = intro;
    intro.start();
    // Greeted at this stage: the next launch knows what is new.
    if (g.seen !== g.reached) {
      State.settings.growth = { ...g, seen: g.reached };
      void Bridge.saveGrowth(State.settings.growth);
    }
    return true;
  }

  // ── Mode / view ─────────────────────────────────────────────────────────────

  private setMode(mode: IslandMode) {
    const prev = State.mode;
    if (mode === prev) return;
    State.mode = mode;
    if (mode === "expanded") Sound.play("open");
    if (prev === "expanded") {
      Sound.play("close");
      // A folded card is still waiting: it keeps the island pinned.
      if (!State.pendingApproval) State.isPinned = false;
      void Bridge.focusWindow(false);
    }
    if (mode !== "expanded") {
      closePlanCard();
      this.engine.resetMorph();
      // Nothing can be seen of the sequence once the island is shut, and leaving
      // it running would keep the frame loop awake — the island must cost
      // nothing while hidden.
      UploadSeq.deactivate();
    }
    this.updateWindowCollapsed();
    this.animateGeometry();
    State.notify();
  }

  /** True while the drop sequence owns the island body. */
  private get uploadActive(): boolean {
    return State.mode === "expanded" && UploadSeq.isActive && UPLOAD_VIEWS.has(State.view);
  }

  /** Navigating out of the drop flow ends the sequence, as on macOS. */
  private stopSequenceIfLeaving(view: IslandViewName) {
    if (UploadSeq.isActive && !UPLOAD_VIEWS.has(view)) UploadSeq.deactivate();
  }

  expand(view: IslandViewName) {
    this.stopSequenceIfLeaving(view);
    if (view !== "overview") closePlanCard();
    State.view = view;
    if (State.mode !== "expanded") this.setMode("expanded");
    else this.animateGeometry();
    State.lastActivity = performance.now();
    State.notify();
  }

  setView(view: IslandViewName) {
    this.stopSequenceIfLeaving(view);
    if (view !== "overview") closePlanCard();
    if (State.mode !== "expanded") {
      this.fsm.forceHome();
      State.view = view;
      this.animateGeometry();
      State.notify();
      return;
    }
    State.view = view;
    State.lastActivity = performance.now();
    this.animateGeometry();
    State.notify();
  }

  collapse() {
    // A waiting card is only ever folded, never dropped by a close.
    if (State.pendingApproval) {
      this.foldApproval();
      return;
    }
    State.isPinned = false;
    this.fsm.pinned = false;
    // Drive the state machine rather than the mode: setting the mode behind its
    // back left it thinking the island was still open, and a click on the compact
    // island then did nothing — the island could never be reopened.
    this.fsm.forcePetit();
  }

  /** Alert from the hook server: open on this view. Pinned alerts never auto-close. */
  alert(view: IslandViewName) {
    this.fsm.pinned = State.isPinned;
    this.fsm.forceHome();
    this.expand(view);
  }

  reveal() {
    this.fsm.reveal();
  }

  /** Music started playing: the compact island, without the peek sound (musicReveal). */
  revealSilently() {
    this.silentReveal = true;
    this.fsm.reveal();
    this.silentReveal = false;
  }

  /** Right-click on Groovy: wardrobe open ↔ back to the usual view. */
  toggleWardrobe() {
    if (State.paused || State.mode === "hidden") return;
    // The greeting and the drop sequence draw a Groovy of their own.
    if (State.mode === "expanded" && (State.view === "greeting" || this.uploadActive)) return;
    if (State.mode === "expanded" && State.view === "wardrobe") this.setView(State.defaultView());
    else this.setView("wardrobe");
  }

  /**
   * Right-click on the desktop Groovy (macOS openWardrobeFromDesktop): opens the
   * wardrobe from any state, or goes back if it is already open.
   */
  wardrobeFromDesktop() {
    this.wardrobeAnywhere();
  }

  /**
   * The wardrobe from any state — compact or hidden island included — or back
   * to the usual view if it is already open. The desktop Groovy's right-click
   * and the wardrobe shortcut (`open-wardrobe`) both land here.
   */
  wardrobeAnywhere() {
    if (State.mode === "expanded" && State.view === "wardrobe") {
      this.setView(State.defaultView());
      return;
    }
    if (State.paused) return;
    this.alert("wardrobe");
  }

  /** An alert stopped waiting for an answer: let the island auto-close again. */
  dropPin() {
    this.fsm.pinned = false;
    // The countdown the pin held back starts now, if the mouse is elsewhere.
    if (!this.wasInIsland) this.fsm.mouseLeft();
  }

  // ── Keyboard shortcuts (island/shortcuts.ts) ────────────────────────────────

  emote(name: BotEmoteName) {
    this.engine.triggerEmote(name);
    this.ensureRunning();
  }

  /** Ctrl+P: keep the open island from folding away, or let it fold again. */
  setPinned(on: boolean) {
    State.isPinned = on;
    this.fsm.pinned = on;
    if (on) {
      // The countdown bar reads the state machine's deadline, cleared with it.
      this.fsm.cancelTimers();
    } else if (!this.wasInIsland && this.fsm.state === "home") {
      this.fsm.mouseLeft();
    }
    State.notify();
  }

  /** The island takes the keyboard, so its own shortcuts work (Mac: makeKey).
   *  It gives it back when it closes, or when the chat is left. */
  takeKeyboard() {
    void Bridge.focusWindow(true);
  }

  /** The desktop shortcut needs a desktop Groovy: none on GNOME's Wayland. */
  canLeaveIsland(): boolean {
    return this.desktop.supported;
  }

  /** The desktop shortcut (macOS DesktopMochiController.flyOutOrHome). */
  flyOutOrHome() {
    // The greeting and the drop sequence draw a Groovy of their own: he stays
    // for them, as he does for a drag (canDragOut).
    const busy = State.mode === "expanded" && (State.view === "greeting" || this.uploadActive);
    if (busy && !State.mochiOnDesktop) return;
    this.desktop.flyOutOrHome();
  }

  /** Ctrl+O / Ctrl+E go to the view on screen. */
  viewCommand(command: ViewCommand) {
    this.views.get(State.view)?.command?.(command);
  }

  // ── File drop ───────────────────────────────────────────────────────────────

  private onDragDrop(e: { type: string; paths?: string[] }) {
    if (e.type !== "over") void Bridge.log(`drag ${e.type} ${e.paths?.length ?? 0} file(s)`);
    if (State.paused) return;
    switch (e.type) {
      case "enter":
      case "over": {
        if (State.fileDragOver) return;
        State.fileDragOver = true;
        this.engine.animateMorph(1);
        // enterZone must run before the island expands, so the sequence is
        // already active by the time the view becomes `upload`.
        UploadSeq.enterZone(State.mouseInIsland.x, State.mouseInIsland.y);
        this.alert("upload");
        break;
      }
      case "leave": {
        if (!State.fileDragOver) return;
        State.fileDragOver = false;
        this.engine.animateMorph(0);
        // The island deliberately stays open: the drag session is still alive.
        UploadSeq.exitZone();
        State.notify();
        break;
      }
      case "drop": {
        State.fileDragOver = false;
        const path = e.paths?.[0];
        if (!path) {
          this.engine.animateMorph(0);
          this.setView(State.defaultView());
          return;
        }
        this.swallow(path);
        break;
      }
    }
  }

  /**
   * Groovy eats the file. Nothing here waits on the file system: the copy into
   * the inbox runs in the background and swaps the path in when it lands, so a
   * slow disk can never stall the animation — same as FileDropHandler on macOS.
   */
  private swallow(path: string) {
    const name = path.split(/[\\/]/).pop() || "file";
    State.droppedFile = { name, path };
    State.promptContext = { kind: "file", name, path };
    State.chatHistory = [];
    void Bridge.chatReset();

    UploadSeq.performDrop(State.uploadDuration);
    this.uploadTens = 0;
    this.uploadDone = false;

    this.engine.gulp();
    Sound.play("approve");
    this.engine.triggerEmote("happy");
    this.engine.animateMorph(0);

    State.uploadProgress = 0;
    this.setView("uploading");
    this.ensureRunning();

    void Bridge.ingestFile(path)
      .then((file) => {
        // Cancelled (or replaced by another drop) while the copy was running.
        if (State.droppedFile?.path !== path) return;
        State.droppedFile = { name: file.name, path: file.path };
        State.promptContext = { kind: "file", name: file.name, path: file.path };
        State.notify();
      })
      .catch((err) => {
        if (State.droppedFile?.path !== path) return;
        UploadSeq.deactivate();
        State.noteMessage = String(err).replace(/^Error:\s*/, "");
        this.engine.animateMorph(0);
        this.setView("note");
        Sound.play("error");
        window.setTimeout(() => this.setView(State.defaultView()), 2400);
      });
  }

  /** "Cancel" on the dropped file: forget it, so the chat does not pick it up. */
  private discardDrop() {
    State.droppedFile = null;
    State.promptContext = null;
    this.setView(State.defaultView());
  }

  /**
   * Sounds and view changes hung off the canvas timeline: a `tick` every 10 %,
   * the ✓ chime when the bar completes, then `choose` once Groovy has grown back.
   */
  private stepSequence() {
    const since = UploadSeq.sinceDrop();
    if (since == null) return;
    const dur = State.uploadDuration;
    const p = Math.max(0, Math.min(1, (since - PRE_PROGRESS) / dur));

    const tens = Math.floor(p * 10);
    if (tens > this.uploadTens && tens < 10) {
      this.uploadTens = tens;
      Sound.play("tick");
    }

    if (!this.uploadDone && since >= PRE_PROGRESS + dur) {
      this.uploadDone = true;
      Sound.play("approve");
      this.engine.triggerEmote("happy");
    }
    // The extra second is the grow-back, after which the choose card is up.
    if (since >= PRE_PROGRESS + dur + 1 && State.view === "uploading") {
      this.setView("choose");
    }
  }

  // ── Geometry ────────────────────────────────────────────────────────────────

  private targetSize(): { w: number; h: number; r: number } {
    let { w, h } = islandSize(State.mode, State.view, State.chatHistory.length);
    // The intro's seed falls into a small island, which grows around Groovy.
    if (State.mode === "expanded" && State.view === "greeting" && this.intro && !this.intro.grown) {
      return { w: SEED_ISLAND.w, h: SEED_ISLAND.h, r: EXPANDED_CORNER };
    }
    if (State.mode === "expanded" && State.view === "question" && State.pendingApproval?.questions) {
      h = QUESTION_PICKER_H;
    }
    // A recorded call's strip holds "Stop & write notes": a little wider.
    if (this.stripActive()) return { w: this.liveShown === "recording" ? LIVE_STRIP_W + 64 : LIVE_STRIP_W, h: LIVE_STRIP_H, r: LIVE_STRIP_H / 2 };
    const r = State.mode === "expanded" ? EXPANDED_CORNER : ROUNDED_CORNER;
    return { w, h, r };
  }

  /** The window's size as last asked (island::fit). */
  private fitted = { w: 0, h: 0 };

  /** Sizes the window to `w`×`h`: the island and nothing around it (round 3 §8 B). */
  private fitWindow(w: number, h: number) {
    w = Math.ceil(w);
    h = Math.ceil(h);
    if (w === this.fitted.w && h === this.fitted.h) return;
    this.fitted = { w, h };
    void Bridge.fitIsland(w, h);
  }

  private animateGeometry() {
    const { w, h, r } = this.targetSize();
    // Grown first to whichever is larger, now or after; shrunk when it settles (frame).
    this.fitWindow(Math.max(w, this.width.value), Math.max(h, this.height.value));
    // One ease-out curve each way (round 3B item 2): no overshoot past the
    // window, which was just grown to the larger size, so nothing is cut.
    const ms = duration(MOTION.island);
    const now = performance.now();
    this.width.curveTowards(w, ms, now, easeOut);
    this.height.curveTowards(h, ms, now, easeOut);
    this.radius.curveTowards(r, ms, now, easeOut);
    this.ensureRunning();
  }

  private applyGeometry() {
    const w = this.width.value;
    const hh = this.height.value;
    const r = this.radius.value;
    this.islandEl.style.width = `${w}px`;
    this.islandEl.style.height = `${hh}px`;
    this.islandEl.style.borderRadius = `0 0 ${r}px ${r}px`;
    this.islandEl.style.transform = `translateX(-50%)`;
    // These follow the island as it resizes, so they belong here rather than in
    // the state-driven DOM sync.
    this.miniGrid.style.left = `${w - 40 - 14.5}px`;
    this.miniGrid.style.top = `${hh / 2 - 14.5}px`;
    this.greetingCanvas.style.left = `${(w - EXPANDED_W) / 2}px`;
    this.uploadCanvas.el.style.left = `${(w - EXPANDED_W) / 2}px`;

    const viewportW = window.innerWidth > 0 ? window.innerWidth : PANEL_W;
    const rect = { x: (viewportW - w) / 2, y: 0, w, h: hh, r };
    const p = this.pushedRect;
    if (Math.abs(p.x - rect.x) > 0.5 || Math.abs(p.w - rect.w) > 0.5 || Math.abs(p.h - rect.h) > 0.5 || Math.abs(p.r - rect.r) > 0.5) {
      this.pushedRect = rect;
      // The glass (Windows' Acrylic) is clipped to this shape too (glass.rs).
      void Bridge.setIslandRect(rect.x, rect.y, rect.w, rect.h, rect.r);
    }
  }

  /** Island rect in window coordinates (origin top-left of the 720×320 window). */
  private islandRect(): { x: number; y: number; w: number; h: number } {
    const w = this.width.value;
    const hh = this.height.value;
    const viewportW = window.innerWidth > 0 ? window.innerWidth : PANEL_W;
    return { x: (viewportW - w) / 2, y: 0, w, h: hh };
  }

  // ── Window collapse (hidden → tiny wake strip, zero polling) ────────────────

  private updateWindowCollapsed() {
    if (this.collapseTimer != null) {
      window.clearTimeout(this.collapseTimer);
      this.collapseTimer = null;
    }
    if (State.mode === "hidden") {
      // Let the island finish retracting, then drop the window to the wake strip:
      // from there the OS delivers no cursor events, so nothing polls at all.
      this.collapseTimer = window.setTimeout(() => {
        this.collapseTimer = null;
        if (State.mode !== "hidden") return;
        this.collapsed = true;
        void Bridge.setCollapsed(true);
      }, 420);
    } else if (this.collapsed) {
      // Grow the window back before the island animates open.
      this.collapsed = false;
      void Bridge.setCollapsed(false);
    }
  }

  // ── Input ───────────────────────────────────────────────────────────────────

  // ── Sliding the island along the top of the screen ─────────────────────────
  /** A press on the closed island: where it started, and where the island was. */
  private slide: { x0: number; from: number; travel: number; moved: boolean; frame: number } | null = null;

  private startSlide(screenX: number) {
    const slide = { x0: screenX, from: 0.5, travel: 1, moved: false, frame: 0 };
    this.slide = slide;
    void Bridge.islandTrack().then((t) => {
      if (t && this.slide === slide) [slide.from, slide.travel] = t;
    });
  }

  private slideFraction(screenX: number): number {
    const s = this.slide!;
    return Math.max(0, Math.min(1, s.from + (screenX - s.x0) / s.travel));
  }

  /** True while the press is a slide (so nothing else handles the move). */
  private moveSlide(screenX: number, buttons: number): boolean {
    const s = this.slide!;
    if (!(buttons & 1)) {
      this.slide = null;
      return false;
    }
    if (!s.moved && Math.abs(screenX - s.x0) <= DRAG_THRESHOLD) return true;
    s.moved = true;
    if (!s.frame) {
      s.frame = requestAnimationFrame(() => {
        s.frame = 0;
        if (this.slide === s) void Bridge.islandMove(this.slideFraction(screenX), false);
      });
    }
    return true;
  }

  private endSlide(screenX: number) {
    const s = this.slide!;
    this.slide = null;
    if (s.frame) cancelAnimationFrame(s.frame);
    if (s.moved) {
      void Bridge.islandMove(this.slideFraction(screenX), true);
      Sound.play("pop");
    } else if (State.mode !== "expanded") {
      this.fsm.click();
    }
  }

  private wireInput() {
    // The wake strip is the only thing the OS can hit while the island is hidden.
    this.wakeStrip.addEventListener("mouseenter", () => {
      Sound.resume();
      if (State.mode === "hidden") this.fsm.mouseEntered();
    });

    this.islandEl.addEventListener("mousedown", (e) => {
      Sound.resume();
      State.lastActivity = performance.now();
      // A click makes a hover-opened island an ordinary open one.
      this.fsm.userInteracted();
      // A press on Groovy may become a drag out to the desktop.
      if (e.button === 0 && this.isBotHit(e.clientX, e.clientY)) {
        this.botPress = { x: e.clientX, y: e.clientY };
      }
      // Right-click: the menu (contextmenu below), not a press.
      if (e.button === 2) return;
      // A click skips the intro.
      if (State.mode === "expanded" && State.view === "greeting") {
        this.intro?.skip();
        return;
      }
      // Pressed on an empty part of the island (not Groovy, not a button or
      // field): it may be a slide along the top of the screen. A plain click on
      // the closed island still opens it, on release.
      if (e.button === 0 && !this.isBotHit(e.clientX, e.clientY) && !isControl(e.target)) {
        this.startSlide(e.screenX);
        return;
      }
      if (State.mode !== "expanded") {
        this.fsm.click();
        return;
      }
      if (this.isBotHit(e.clientX, e.clientY)) {
        this.cancelBotHover();
        this.engine.slap();
      }
    });

    // Right-click anywhere on the island but a text field: Groovy's menu
    // (Open Groovy, Settings, Wardrobe, Coding Studio). Text fields keep the
    // webview's own menu, for copy and paste.
    this.islandEl.addEventListener("contextmenu", (e) => {
      const target = e.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable]")) return;
      e.preventDefault();
      this.cancelBotHover();
      void Bridge.contextMenu();
    });

    // Dragging Groovy out of the island puts him on the desktop.
    window.addEventListener("mousemove", (e) => {
      if (this.slide && this.moveSlide(e.screenX, e.buttons)) return;
      if (this.desktop.carrying) {
        this.desktop.carry(e.clientX, e.clientY);
        return;
      }
      const press = this.botPress;
      if (!press) return;
      if (!(e.buttons & 1)) {
        this.botPress = null;
        return;
      }
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) <= DRAG_THRESHOLD) return;
      this.botPress = null;
      if (!this.canDragOut()) return;
      this.cancelBotHover();
      this.desktop.pickUp(e.clientX, e.clientY);
    });
    window.addEventListener("mouseup", (e) => {
      if (this.slide) this.endSlide(e.screenX);
      this.botPress = null;
      if (this.desktop.carrying) this.desktop.carryEnd(e.clientX, e.clientY);
    });

    // Only keys typed into the island itself land here, never Escape typed in
    // a terminal — so it may fold a waiting card away, as Escape in the notch
    // does on macOS.
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && State.mode === "expanded") {
        if (State.pendingApproval) this.foldApproval();
        else if (!State.isPinned) this.collapse();
      }
      State.lastActivity = performance.now();
    });

    void onDragDrop((e) => this.onDragDrop(e));

    // Outside Tauri (plain browser) drive the cursor from DOM events so the
    // island can be inspected with `npm run dev`.
    if (!IS_TAURI) this.followPageCursor();
  }

  /**
   * Takes the cursor from the page's own mouse events instead of Rust's poll.
   * Used where the OS has no global cursor position (Wayland): the events only
   * fire while the pointer is over the island, so leaving the window is
   * reported as a cursor far away, which is what the poll would have said.
   */
  followPageCursor() {
    window.addEventListener("mousemove", (e) => this.onCursor(e.clientX, e.clientY));
    window.addEventListener("mouseout", (e) => {
      if (e.relatedTarget == null) this.onCursor(-10_000, -10_000);
    });
  }

  /**
   * Pointer on/off the island as the compositor sees it (Linux only). Null
   * until the first report, so a platform that never sends it is not gated.
   */
  private pointerInside: boolean | null = null;

  setPointerInside(inside: boolean) {
    this.pointerInside = inside;
  }

  /** Cursor in window-logical coordinates. */
  onCursor(x: number, y: number) {
    // WebKitGTK can deliver a mousemove after the pointer has left the layer
    // surface; trusting it re-enters the island and the auto-close never runs.
    if (this.pointerInside === false) {
      x = -10_000;
      y = -10_000;
    }
    State.mouse = { x, y };
    const rect = this.islandRect();
    State.mouseInIsland = { x: x - rect.x, y: y - rect.y };

    // Windows sends no cursor position with an OLE drag, so the drop sequence is
    // fed from the Win32 cursor poll instead — it runs throughout the drag.
    if (UploadSeq.isActive && !UploadSeq.dropped) {
      UploadSeq.updateCursor(State.mouseInIsland.x, State.mouseInIsland.y);
    }

    const inIsland =
      x >= rect.x - HIT_MARGIN && x <= rect.x + rect.w + HIT_MARGIN &&
      y >= rect.y - HIT_MARGIN && y <= rect.y + rect.h + HIT_MARGIN;

    // Recorded before the state machine hears of it: a transition it makes
    // right away (open on hover) reads where the pointer is, and must not see
    // the pointer as still outside.
    const wasIn = this.wasInIsland;
    this.wasInIsland = inIsland;
    if (inIsland && !wasIn) {
      if (this.fsm.state === "groovy") this.intro?.hover();
      this.fsm.mouseEntered();
    }
    if (!inIsland && wasIn) {
      this.fsm.mouseLeft();
    }

    // Bot hover → love
    const overBot = State.mode === "expanded" && State.stateOverride == null && this.isBotHit(x, y);
    if (overBot && !this.botHovering) this.botHoverIn(x, y);
    if (!overBot && this.botHovering) this.cancelBotHover();
    this.botHovering = overBot;
    if (this.botHovering) {
      const d = Math.hypot(x - this.botHoverStart.x, y - this.botHoverStart.y);
      if (d > 40) {
        this.botHoverStart = { x, y };
        this.scheduleLove();
      }
    }

    this.ensureRunning();
  }

  /** The greeting and the drop sequence draw a Groovy of their own: not that one. */
  private canDragOut(): boolean {
    if (State.mode === "hidden" || !this.desktop.canPickUp()) return false;
    return !(State.mode === "expanded" && (State.view === "greeting" || this.uploadActive));
  }

  private isBotHit(x: number, y: number): boolean {
    // Out on the desktop, the island's Groovy is invisible: nothing to hit.
    if (State.mochiOnDesktop) return false;
    const rect = this.islandRect();
    const cx = rect.x + this.botCx.value;
    const cy = rect.y + this.botCy.value;
    const radius = this.botSize.value / 2;
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius * radius;
  }

  private botHoverIn(x: number, y: number) {
    if (performance.now() / 1000 - this.lastLoveTime < 6) return;
    this.botHoverStart = { x, y };
    this.engine.blink();
    this.engine.tgEs = 1.08;
    Sound.play("hover");
    this.scheduleLove();
  }

  private scheduleLove() {
    if (this.botHoverTimer != null) window.clearTimeout(this.botHoverTimer);
    this.botHoverTimer = window.setTimeout(() => {
      this.botHoverTimer = null;
      if (!this.botHovering || State.stateOverride != null) return;
      if (performance.now() / 1000 - this.lastLoveTime < 6) return;
      this.lastLoveTime = performance.now() / 1000;
      this.engine.triggerEmote("love");
      Sound.play("love");
    }, 1900);
  }

  private cancelBotHover() {
    if (this.botHoverTimer != null) window.clearTimeout(this.botHoverTimer);
    this.botHoverTimer = null;
    this.engine.tgEs = 1;
  }

  /** Three slaps → dizzy + confused view for 3.3 s, then back. */
  handleDizzy() {
    this.prevViewBeforeConfused = State.view;
    State.stateOverride = "dizzy";
    this.engine.setState("dizzy");
    Sound.play("dizzy");
    this.alert("confused");
    if (this.confusedRecovery != null) window.clearTimeout(this.confusedRecovery);
    this.confusedRecovery = window.setTimeout(() => {
      this.confusedRecovery = null;
      State.stateOverride = null;
      this.engine.setState(State.effectiveState);
      if (State.view === "confused") {
        const fallback = State.defaultView();
        this.setView(this.prevViewBeforeConfused === "confused" ? fallback : this.prevViewBeforeConfused);
      }
      this.engine.triggerEmote("happy");
    }, 3300);
  }

  // ── Frame loop ──────────────────────────────────────────────────────────────

  ensureRunning() {
    if (this.running) return;
    this.running = true;
    this.lastFrame = performance.now();
    requestAnimationFrame(this.frame);
  }

  private frame = (nowMs: number) => {
    const dt = Math.min(0.05, (nowMs - this.lastFrame) / 1000);
    this.lastFrame = nowMs;

    this.width.step(dt, nowMs);
    this.height.step(dt, nowMs);
    this.radius.step(dt, nowMs);
    this.applyGeometry();

    if (this.dirty) {
      this.dirty = false;
      this.syncDom();
    }

    this.updateBotTargets();
    this.botCx.step(dt);
    this.botCy.step(dt);
    this.botSize.step(dt);

    const greetingActive = State.mode === "expanded" && State.view === "greeting";
    if (greetingActive) {
      const gctx = this.greetingCanvas.getContext("2d");
      if (gctx) {
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        gctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        this.intro?.draw(gctx);
      }
    } else {
      // Kept running even while the drop canvas is up, so the island's own Groovy
      // is already in the right place the moment the canvas fades out.
      this.drawBot(dt);
    }

    const uploadActive = this.uploadActive;
    if (uploadActive) this.uploadCanvas.draw(UploadSeq.frame(), nowMs / 1000);
    this.uploadCanvas.el.classList.toggle("on", uploadActive);
    this.viewsEl.classList.toggle("hidden-by-upload", uploadActive);

    tickMiniBots(dt);
    // A ticker scroll that loses its frames freezes mid-way, rows overlapping.
    const viewAnimating = this.views.get(State.view)?.tick?.(nowMs) === true;
    if (UploadSeq.isActive) this.stepSequence();
    this.updateCountdown(nowMs);

    // Nothing is drawn while the island is hidden, so nothing may keep the loop
    // alive either. This used to read `... || this.engine.busy || State.mode !==
    // "hidden"`, and engine.busy is permanently true for any state with a
    // looping animation — breathing, ratelimit sweat, sleeping z's, the search
    // sweep — so a hidden island went on burning frames in exactly the states it
    // spends most of its life in. Geometry still has to finish retracting.
    const settling =
      this.width.animating || this.height.animating || this.radius.animating;
    const moving = settling ||
      !this.botCx.settled || !this.botCy.settled || !this.botSize.settled ||
      greetingActive || this.engine.moving || UploadSeq.isActive || viewAnimating;
    // Folded into the small island (round 3 §9: idle CPU ~0): his looping
    // animation runs at a tenth of the frames while he works, and not at all
    // while he just rests; open, it runs at full rate.
    const pace = State.mode === "hidden" ? (settling ? "full" : "stop")
      : moving ? "full"
      : !this.engine.ambient ? "stop"
      : State.mode === "expanded" ? "full"
      : State.effectiveState !== "idle" ? "slow" : "stop";

    if (!settling) this.fitWindow(this.width.value, this.height.value);
    if (pace === "full") {
      requestAnimationFrame(this.frame);
    } else if (pace === "slow") {
      window.setTimeout(() => requestAnimationFrame(this.frame), SLOW_FRAME_MS);
    } else {
      this.running = false;
      Sound.idle();
    }
  };

  private updateBotTargets() {
    const p = this.stripActive()
      ? { cx: 24, cy: LIVE_STRIP_H / 2, diameter: 22, opacity: this.liveShown === "recording" ? 0 : 1 }
      : botPosition(State.mode, State.view, this.height.value, State.uploadProgress);
    this.botCx.target = p.cx;
    this.botCy.target = p.cy;
    this.botSize.target = p.diameter / 0.6;

    const greetingActive = State.mode === "expanded" && State.view === "greeting";
    // The drop canvas draws its own Groovy; two of them would overlap. Out on the
    // desktop, he isn't here at all.
    const away = State.mochiOnDesktop;
    const visible = p.opacity > 0 && !greetingActive && !this.uploadActive && !away;
    this.botCanvas.style.opacity = visible ? "1" : "0";

    if (visible && State.mode === "expanded" && State.view !== "uploading") {
      const d = p.diameter;
      const color = botGlowColor(State.effectiveState);
      this.botGlow.style.display = "block";
      this.botGlow.style.width = `${d * 2.2}px`;
      this.botGlow.style.height = `${d * 2.2}px`;
      this.botGlow.style.left = `${this.botCx.value - d * 1.1}px`;
      this.botGlow.style.top = `${this.botCy.value - d * 1.1}px`;
      this.botGlow.style.background = `radial-gradient(circle, ${color} 0%, transparent 62%)`;
      this.botGlow.style.opacity = String(botGlowOpacity(State.effectiveState));
    } else {
      this.botGlow.style.display = "none";
    }
  }

  private drawBot(dt: number) {
    const size = this.botSize.value;
    const w = Math.max(1, Math.round(size));
    const hCss = w + BOT_OVERHANG;
    const wCss = w + BOT_SIDE * 2;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    if (this.canvasPx !== w) {
      this.canvasPx = w;
      this.botCanvas.width = Math.round(wCss * dpr);
      this.botCanvas.height = Math.round(hCss * dpr);
      this.botCanvas.style.width = `${wCss}px`;
      this.botCanvas.style.height = `${hCss}px`;
    }
    this.botCanvas.style.left = `${this.botCx.value - wCss / 2}px`;
    this.botCanvas.style.top = `${this.botCy.value - BOT_OVERHANG / 2 - hCss / 2}px`;

    const ctx = this.botCanvas.getContext("2d");
    if (!ctx) return;

    // Groovy always wears his own look's colours (round 3B item 3): never a
    // tile's, a plan's or the theme's. Only the user's colours change them.
    this.engine.bodyColor = null;
    this.engine.particleOverhang = BOT_OVERHANG;
    this.engine.lookX = this.lookX();
    this.engine.lookY = this.lookY();
    if (this.engine.morph > 0.3) {
      this.engine.slotHTarget = State.fileDragOver ? 0.2 : 0;
    } else {
      this.engine.slotHTarget = 0;
      if (this.engine.morph < 0.05) {
        this.engine.slotH = 0;
        this.engine.slotHVel = 0;
      }
    }
    // Only the main Groovy is dressed — the one of the main tool's pill (Settings →
    // Active pills): a focused integration pill shows its own colours, unless
    // the wardrobe is open (BotCanvasView.showOutfit, macOS).
    // In the wardrobe the hovered outfit swaps in at once, without the drop-in.
    const inWardrobe = State.mode === "expanded" && State.view === "wardrobe";
    const mainFocused = State.focusId == null || State.focusId === State.mainPillId;
    const showOutfit = mainFocused || State.mode !== "expanded" || inWardrobe;
    // Head, face and neck, with the item under the pointer tried on in its slot.
    const appearance = State.appearancePreview ?? currentWardrobe(State.settings.wardrobe).appearance;
    const worn = tryOn(appearance.layers, State.wardrobePreview);
    this.engine.setOutfit(showOutfit ? (worn[0] ?? "none") : "none", !inWardrobe);
    this.engine.layers = showOutfit ? worn.slice(1) : [];
    this.engine.appearance = appearance;
    this.engine.setSkin(shownLook(State.lookPreview, State.settings.groovyLook), !inWardrobe);
    this.engine.setStage(shownStage(parseGrowth(State.settings.growth)), !inWardrobe);
    // Fewer particles and calmer motion when asked to, and in the small island.
    this.engine.lite = State.settings.assistant.performanceProfile === "lightweight" || State.mode !== "expanded";
    // Dances while music plays: always in the compact island, expanded only on
    // the music pill's card (BotCanvasView, macOS). Asked every frame.
    this.engine.setDancing(islandDances({
      music: State.spotifyPlaying,
      state: State.effectiveState,
      mode: State.mode,
      view: State.view,
      focusId: State.focusTask?.id,
    }));

    this.engine.update(dt);
    ctx.setTransform(dpr, 0, 0, dpr, BOT_SIDE * dpr, 0);
    ctx.clearRect(-BOT_SIDE, 0, wCss, hCss);
    ctx.save();
    this.engine.applyDance(ctx, w, hCss);
    this.engine.draw(ctx, w, hCss);
    ctx.restore();
  }

  /** BotCanvasView.lookX / lookY — tanh of the distance to the bot. */
  private lookX(): number {
    const rect = this.islandRect();
    const botScreenX = rect.x + this.botCx.value;
    return Math.tanh((State.mouse.x - botScreenX) / 260);
  }

  private lookY(): number {
    return -Math.tanh((State.mouse.y - this.botCy.value) / 200);
  }

  private updateCountdown(nowMs: number) {
    // The state machine's own deadline, so the bar follows an auto-close delay
    // edited while the countdown runs.
    const dueAt = this.fsm.homeCollapseDueAt;
    if (State.mode !== "expanded" || State.isPinned || dueAt == null) {
      this.countdown.style.width = "0px";
      return;
    }
    const autoClose = this.fsm.homeToPetitDelay;
    // Under 3 s the bar would only flash.
    if (autoClose < 3) {
      this.countdown.style.width = "0px";
      return;
    }
    const windowS = Math.min(10, autoClose * 0.6);
    const remaining = (dueAt - nowMs) / 1000;
    this.countdown.style.width =
      remaining < windowS ? `${Math.max(0, clamp(remaining / windowS, 0, 1) * 160)}px` : "0px";
  }

  // ── DOM sync ────────────────────────────────────────────────────────────────

  private syncDom() {
    const expanded = State.mode === "expanded";
    const greetingActive = expanded && State.view === "greeting";

    const live = expanded && !greetingActive;
    this.contentEl.style.opacity = live ? "1" : "0";
    // While the drop sequence owns the body its buttons are painted on the canvas
    // underneath, so only the header may keep taking clicks up here.
    this.contentEl.style.pointerEvents = live && !this.uploadActive ? "auto" : "none";
    this.header.el.style.pointerEvents = live ? "auto" : "none";
    this.greetingCanvas.style.display = greetingActive ? "block" : "none";

    // Leaving the greeting, however it ends, lets its sound fade out.
    if (this.greetingShown && !greetingActive) this.intro?.leave();
    this.greetingShown = greetingActive;
    // A wardrobe try-on never outlives the wardrobe.
    if (State.wardrobePreview && !(expanded && State.view === "wardrobe")) State.wardrobePreview = null;
    if (State.lookPreview && !(expanded && State.view === "wardrobe")) this.clearLookPreview(false);
    // Closing the island stops listening for a command (and hangs up a call).
    if (!expanded && State.voiceListening) this.voice.dismiss();
    // Setting up the brain or on a call: the island stays open until it's over.
    const hold = !!State.brainSetup || State.voiceCall;
    if (hold !== this.holding) {
      this.holding = hold;
      this.fsm.pinned = hold || State.isPinned;
    }
    // A reminder closed without an answer comes back a little later.
    if (State.breakDue && !(expanded && State.view === "note")) this.breaks.snooze();
    if (State.reminderDue && !(expanded && State.view === "note")) {
      const r = State.reminderDue;
      State.reminderDue = null;
      void snoozeReminder(Bridge, r).catch(showError);
    }

    this.header.sync();
    for (const [name, view] of this.views) {
      const on = name === State.view;
      view.el.classList.toggle("on", on);
      if (on) view.sync();
    }

    // The chat is the only view with a text field, so it is the only time the
    // island is allowed to take keyboard focus.
    if (this.lastSyncedView !== State.view) {
      const wasChat = this.lastSyncedView === "prompt";
      this.lastSyncedView = State.view;
      if (State.view === "prompt") {
        void Bridge.focusWindow(true);
        window.setTimeout(() => this.views.get("prompt")?.focus?.(), 120);
      } else if (wasChat) {
        void Bridge.focusWindow(false);
      }
    }

    // Compact mini grid
    // The badge takes the grid's place on the compact island.
    const showGrid = State.mode === "compact" && this.liveShown === null;
    this.miniGrid.style.opacity = showGrid ? "1" : "0";
    if (showGrid) {
      const others = State.otherTasks.slice(0, 4);
      const key = others.map((t) => t.id).join("|");
      if (this.miniGrid.dataset.key !== key) {
        this.miniGrid.dataset.key = key;
        this.miniGrid.replaceChildren();
        for (const t of others) {
          this.miniGrid.append(createMiniBot(t, 13));
        }
        pruneMiniBots();
      }
    }

    syncMiniBotStates(State.tasks);
    this.engine.setState(State.effectiveState);
  }

  /** Applies settings coming from Rust at boot. */
  applySettings() {
    Sound.setEnabled(State.settings.soundEnabled);
    Sound.setVolume(State.settings.soundVolume);
    Sound.setFlavor(flavorOf(parseSkin(State.settings.groovyLook)));
    this.fsm.homeToPetitDelay = State.settings.autoCloseInterval;
    this.fsm.openOnHover = State.settings.openOnHover;
    this.fsm.keepVisible = State.settings.keepVisible !== false;
    if (this.fsm.keepVisible && this.fsm.state === "hidden" && this.greetingShown) this.fsm.reveal();
    State.notify();
  }

  get panelSize() {
    return { w: PANEL_W, h: PANEL_H };
  }

  get chatHeight() {
    return chatPromptHeight(State.chatHistory.length);
  }
}

/** Something in the island that takes its own clicks: never the start of a slide. */
function isControl(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    !!target.closest("button, input, select, textarea, a, label, [contenteditable], .chat-log, .wardrobe-grid, .pill")
  );
}
