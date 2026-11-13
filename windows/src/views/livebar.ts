// The island while Groovy listens (FIXES_ROUND3 item 3, mockup Voice.dc.html).
// By default the island stays as it is and shows a small badge — a dot,
// "Dictating 0:04" (red "Recording 12:40" for a recorded call) and a wave
// button. The wave button, or Settings → Voice → While listening, show: Live
// strip, shrinks the island into a strip: Groovy, nine bars that move with
// your voice, the clock, "back to island" and Stop.

import { FlowBridge } from "../core/flow";
import { State } from "../core/state";
import { t } from "../i18n/i18n";
import { BANDS, LiveClock, liveClock, liveLevels, liveView, type LiveView } from "../island/live";
import { CallControl, MicLevel } from "../voice/call";
import { h } from "./dom";

const WAVE = "M4 12h3M9 8v8M13 5v14M17 9v6M20 12h1";
const EXPAND = "M4 9V4h5M20 15v5h-5M4 4l6 6M20 20l-6-6";

function stroke(path: string, size: number): SVGSVGElement {
  const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("width", String(size));
  s.setAttribute("height", String(size));
  s.setAttribute("aria-hidden", "true");
  const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
  p.setAttribute("d", path);
  s.append(p);
  return s;
}

export interface LiveBarHost {
  /** The wave button / "back to island": strip on or off. */
  setStrip(on: boolean): void;
}

export function stop(view: LiveView | null) {
  switch (view?.stop) {
    case "finish":
      void FlowBridge.liveAction("finish");
      break;
    case "stopNote":
      void FlowBridge.liveAction("stopNote");
      break;
    case "endCall":
      CallControl.toggle();
      break;
  }
}

export function buildLiveBar(host: LiveBarHost) {
  const dot = h("span", { class: "live-dot" });
  const text = h("span", { class: "live-text" });
  const wave = h("button", { class: "live-wave", title: t("Show as live strip"), "aria-label": t("Show as live strip") }, stroke(WAVE, 12));
  wave.addEventListener("click", (e) => {
    e.stopPropagation();
    host.setStrip(true);
  });
  const badge = h("div", { class: "live-badge" }, dot, text, wave);

  const bars = h("div", { class: "live-bars", "aria-hidden": "true" });
  for (let i = 0; i < BANDS; i++) bars.append(h("i", {}));
  const clock = h("span", { class: "live-clock" });
  const back = h("button", { class: "live-back", title: t("Back to the island"), "aria-label": t("Back to the island") }, stroke(EXPAND, 12));
  back.addEventListener("click", (e) => {
    e.stopPropagation();
    host.setStrip(false);
  });
  const stopBtn = h("button", { class: "live-stop", title: t("Stop"), "aria-label": t("Stop") }, h("i", {}));
  const notesBtn = h("button", { class: "live-notes", text: t("Stop & write notes") });
  const recLabel = h("span", { class: "live-rec" });
  let view: LiveView | null = null;
  for (const b of [stopBtn, notesBtn]) {
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      stop(view);
    });
  }
  const strip = h("div", { class: "live-strip" }, h("span", { class: "live-bot" }), recLabel, bars, clock, back, stopBtn, notesBtn);

  const el = h("div", { id: "live", hidden: true }, badge, strip);
  const clocks = new LiveClock();
  let started: number | null = null;
  const levels = new Array<number>(BANDS).fill(0);
  // Its own clock: about 30 frames a second while the bars move, once a second
  // for a clock alone (an hour-long recording), nothing when nothing listens.
  let timer: number | null = null;
  let frame: number | null = null;
  const loop = () => {
    timer = null;
    frame = null;
    if (!api.tick(Date.now())) return;
    if (!bars.hidden) timer = window.setTimeout(() => (frame = requestAnimationFrame(loop)), 33);
    else timer = window.setTimeout(loop, 1000);
  };
  const run = () => {
    if (timer !== null || frame !== null) return;
    loop();
  };

  const api = {
    el,
    /** What shows now, or null. Called on every State change. */
    sync(stripOn: boolean): LiveView | null {
      view = liveView(State.live, State.voiceCall);
      started = clocks.start(view, State.live, Date.now());
      el.hidden = !view;
      if (!view) return null;
      el.className = `${view.tone}${stripOn ? " strip" : " badge"}${view.kind === "recording" ? " recording" : ""}`;
      wave.hidden = view.kind === "message";
      stopBtn.hidden = !view.stop || view.kind === "recording";
      notesBtn.hidden = view.kind !== "recording";
      bars.hidden = view.kind === "recording" || view.kind === "message";
      recLabel.hidden = view.kind !== "recording";
      api.tick(Date.now());
      run();
      return view;
    },
    /** The clock and the bars; true while they move. */
    tick(now: number): boolean {
      if (!view) return false;
      const time = started === null ? "" : liveClock(now - started);
      text.textContent = t(view.label, { 0: time, 1: view.words ?? "" }).trim();
      text.title = view.words ?? "";
      clock.textContent = time;
      recLabel.textContent = t("Recording call · {0}", { 0: time });
      if (!bars.hidden) {
        for (let i = 0; i < BANDS; i++) {
          const goal = view.kind === "call"
            ? MicLevel.value * (0.6 + 0.4 * Math.sin(now / 140 + i * 1.1))
            : view.kind === "voice" || State.live.phase === "thinking"
              ? 0.18 + 0.18 * Math.sin(now / 200 - i * 0.7)
              : liveLevels[i];
          levels[i] += (goal - levels[i]) * 0.35;
          (bars.children[i] as HTMLElement).style.transform = `scaleY(${(0.18 + Math.min(1, Math.max(0, levels[i])) * 0.82).toFixed(3)})`;
        }
      }
      return view.timed || !bars.hidden;
    },
  };
  return api;
}
