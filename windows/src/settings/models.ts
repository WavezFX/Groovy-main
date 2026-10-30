// Settings → Models: Groovy's own offline engine (models.rs). Install the
// llama.cpp runtime once, download a GGUF model (both pinned to a SHA-256 and
// checked), pick one, and chat with no internet. Every model shows its size,
// quantisation and the memory it needs, judged against this PC's total memory
// (not what happens to be free this second, FIXES_ROUND2 item 3).

import { Bridge, onEvent, type CatalogModel, type LocalModelInfo, type ModelsStatus } from "../core/bridge";
import { lightLocalCard } from "./lightlocal";
import { DEFAULT_ASSISTANT, type AssistantPrefs, type Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { confirmClick, statusDot } from "./parts";

const PROFILES: [AssistantPrefs["performanceProfile"], string, string][] = [
  ["lightweight", N_("Lightweight"), N_("2K context, half the cores, stops after 5 idle minutes, reduced animation. For 8 GB laptops.")],
  ["balanced", N_("Balanced"), N_("4K context, most cores, stops after 10 idle minutes.")],
  ["performance", N_("High performance"), N_("8K context, every core, stays loaded. Needs 16 GB or more.")],
];

/** Round 3D item 7: the built-in model runs inside Groovy's own memory. */
export const OVER_LIMIT_MODEL = N_("Uses extra memory: over Groovy's 400 MB limit. Ollama + Qwen (Brains → On this PC) does the thinking in its own app and keeps Groovy under it.");

export function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  if (n >= 1024 ** 2) return `${Math.round(n / 1024 ** 2)} MB`;
  return `${Math.round(n / 1024)} KB`;
}

/** Share of the PC's memory a model may need and still suit it (models.rs FIT_SHARE). */
export const FIT_SHARE = 0.4;

/** How a model suits this PC: by its total memory; "short" when other apps hold it right now. */
export function fit(needed: number, total: number, available: number): "fits" | "short" | "too-big" {
  if (needed > total * FIT_SHARE) return "too-big";
  if (needed > available) return "short";
  return "fits";
}

/** What a catalogue model takes once loaded: measured, or estimated from its size (models.rs catalog_ram). */
export function catalogRam(c: CatalogModel): number {
  return c.ramMb > 0 ? c.ramMb * 1024 ** 2 : c.size * 1.25 + 350 * 1024 ** 2;
}

function fitTag(verdict: ReturnType<typeof fit>, needed: number): HTMLElement {
  return h("span", {
    class: verdict === "too-big" ? "tag err" : "tag",
    text: verdict === "too-big" ? t("Too big for this PC") : t("Fits this PC"),
    title: t("needs ~{ram}", { ram: formatBytes(needed) }),
  });
}

export function modelsSection(settings: Settings, save: () => void): HTMLElement {
  const body = h("div", { class: "stack" });
  const el = h("section", { id: "models" }, h("h2", {}, h("span", { text: t("Models") })), body);
  const progress = h("div", { class: "hint" });
  let status: ModelsStatus | null = null;

  const prefs = (): AssistantPrefs => ({ ...DEFAULT_ASSISTANT, ...(settings.assistant ?? {}) });
  const set = <K extends keyof AssistantPrefs>(key: K, value: AssistantPrefs[K]) => {
    settings.assistant = { ...prefs(), [key]: value };
    save();
  };

  const notice = (text: string, kind: "ok" | "err" | "warn" = "err") => {
    const n = h("div", { class: `notice ${kind}`, text });
    body.prepend(n);
    setTimeout(() => n.remove(), 9000);
  };

  async function run(button: HTMLButtonElement, work: () => Promise<unknown>) {
    button.disabled = true;
    try {
      await work();
    } catch (err) {
      notice(String(err).replace(/^Error:\s*/, ""));
    } finally {
      button.disabled = false;
      await refresh();
    }
  }

  function runtimeBlock(s: ModelsStatus): HTMLElement {
    const box = h("div", { class: "stack" });
    const installed = s.runtimeInstalled;
    box.append(
      h(
        "div",
        { class: "row" },
        statusDot(Boolean(s.runtimePath)),
        h("span", {
          text: s.runtimePath
            ? t("Engine installed: llama.cpp ({kind})", { kind: installed === "vulkan" ? "Vulkan" : installed === "path" ? "PATH" : t("CPU") })
            : t("The offline engine (llama.cpp) is not installed yet."),
        }),
      ),
    );
    const row = h("div", { class: "row" });
    for (const r of s.runtimes) {
      if (!r.available) continue;
      const b = h("button", { text: t("Install {name} · {size}", { name: r.id === "vulkan" ? "Vulkan" : t("CPU"), size: formatBytes(r.size) }), title: r.label }) as HTMLButtonElement;
      b.addEventListener("click", () => void run(b, () => Bridge.modelsInstallRuntime(r.id)));
      row.append(b);
    }
    if (row.childElementCount) box.append(row);
    else if (!s.runtimePath) box.append(h("div", { class: "hint", text: t("Install llama.cpp with your package manager so that llama-server is on the PATH.") }));
    box.append(h("div", { class: "hint", text: t("Pinned release b11000 from github.com/ggml-org/llama.cpp, checked against its SHA-256 before use. Vulkan uses the GPU, integrated Radeon and Intel included; CPU works everywhere.") }));
    if (installed === "vulkan") {
      const layers = h("input", { type: "number", min: 0, max: 999, value: prefs().gpuLayers, style: "width:80px" }) as HTMLInputElement;
      layers.addEventListener("change", () => set("gpuLayers", Math.max(0, Math.min(999, Math.floor(Number(layers.value) || 0)))));
      box.append(h("div", { class: "row" }, h("label", { text: t("Layers on the GPU") }), layers, h("span", { class: "hint", text: t("0 = CPU only; 99 = all") })));
    }
    return box;
  }

  function localRow(m: LocalModelInfo, s: ModelsStatus): HTMLElement {
    const selected = (prefs().localModel || s.local.find((x) => x.info)?.file) === m.file;
    const running = s.running === m.file;
    const tag = m.info
      ? fitTag(fit(m.ramNeeded, s.memory.total, s.memory.available), m.ramNeeded)
      : h("span", { class: "tag err", text: t("unreadable"), title: m.error ?? "" });
    const meta = m.info
      ? `${formatBytes(m.size)} · ${m.info.quantization} · ${m.info.architecture}${m.info.contextLength ? ` · ${Math.round(m.info.contextLength / 1024)}K` : ""}`
      : m.error ?? "";
    const use = h("button", { class: selected ? "primary" : "", text: selected ? t("In use") : t("Use") }) as HTMLButtonElement;
    use.disabled = selected || !m.info;
    if (!m.info) use.title = m.error ?? t("This file can't be read as a GGUF model.");
    use.addEventListener("click", () => {
      set("localModel", m.file);
      void refresh();
    });
    const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
    confirmClick(del, t("Click again to delete"), () => void run(del, () => Bridge.modelsDelete(m.file)));
    return h(
      "div",
      { class: "pill-row full" },
      statusDot(running),
      h("div", { class: "name" }, h("div", { text: m.info?.name || m.file }), h("div", { class: "hint", text: meta })),
      tag,
      use,
      del,
    );
  }

  function catalogRow(c: CatalogModel, s: ModelsStatus): HTMLElement | null {
    if (s.local.some((m) => m.file === c.file)) return null;
    const needed = catalogRam(c);
    const verdict = fit(needed, s.memory.total, s.memory.available);
    const b = h("button", { text: t("Download · {size}", { size: formatBytes(c.size) }) }) as HTMLButtonElement;
    b.disabled = s.downloading;
    const download = () => void run(b, () => Bridge.modelsDownload(c.id));
    // Too big for this PC: one more click to say so knowingly.
    if (verdict === "too-big") confirmClick(b, t("Click again to download anyway"), download);
    else b.addEventListener("click", download);
    return h(
      "div",
      { class: "pill-row full" },
      h(
        "div",
        { class: "name" },
        h("div", {}, h("span", { text: c.label }), c.id === s.lightPick ? h("span", { class: "tag", text: t("Recommended") }) : null),
        h("div", { class: "hint", text: `${c.goodFor} · ${c.license}` }),
      ),
      fitTag(verdict, needed),
      b,
    );
  }

  /** The last start found too little memory free: who holds it, and Load anyway. */
  function shortMemoryBlock(s: ModelsStatus, needed: number): HTMLElement {
    const box = h("div", { class: "notice warn" },
      h("div", { text: t("Only {free} of memory is free right now and the model needs about {needed}.", { free: formatBytes(s.memory.available), needed: formatBytes(needed) }) }),
    );
    const list = h("ul", { class: "hint" });
    const free = h("button", { text: t("Free up memory") }) as HTMLButtonElement;
    free.addEventListener("click", () => void Bridge.memoryHogs().then((hogs) => {
      list.replaceChildren(...(hogs ?? []).map((x) => h("li", { text: `${x.name} · ${formatBytes(x.bytes)}` })));
      list.prepend(h("li", { text: t("Close the apps you don't need, then load the model again:") }));
    }));
    const anyway = h("button", { text: t("Load anyway") }) as HTMLButtonElement;
    anyway.addEventListener("click", () => void run(anyway, () => Bridge.modelsLoadAnyway()));
    box.append(h("div", { class: "row" }, free, anyway), list);
    return box;
  }

  function draw(s: ModelsStatus) {
    clear(body);
    const p = prefs();
    if (!s.localAllowed) {
      const use = h("button", { text: t("Use local AI on this PC") }) as HTMLButtonElement;
      body.append(h("div", { class: "notice warn", text: t(OVER_LIMIT_MODEL) }));
      use.addEventListener("click", () => {
        set("aiSetup", "local");
        void refresh();
      });
      body.append(
        h("div", { class: "notice warn", text: s.memory.total <= 9 * 1024 ** 3
          ? t("Local AI is off. On this PC ({total}) a local model slows everything down: a cloud AI is the better choice.", { total: formatBytes(s.memory.total) })
          : t("Local AI is off: Groovy never loads a model on this computer until you turn it on.") }),
        h("div", { class: "row" }, use),
      );
    }
    if (s.shortMemory) body.append(shortMemoryBlock(s, s.shortMemory));
    const light = lightLocalCard(s, settings, prefs, save, () => void refresh());
    if (light) body.append(h("div", { class: "subhead", text: t("Light local") }), light);
    const prof = PROFILES.find((x) => x[0] === p.performanceProfile) ?? PROFILES[1];
    const profile = h("select", {}) as HTMLSelectElement;
    for (const [v, label] of PROFILES) profile.append(h("option", { value: v, text: t(label) }));
    profile.value = prof[0];
    profile.addEventListener("change", () => {
      set("performanceProfile", profile.value as AssistantPrefs["performanceProfile"]);
      void refresh();
    });
    body.append(
      h(
        "div",
        { class: "row" },
        h("label", { text: t("Performance profile") }),
        profile,
        s.recommendedProfile !== prof[0]
          ? h("span", { class: "tag warn", text: t("Suggested here: {name}", { name: t(PROFILES.find((x) => x[0] === s.recommendedProfile)![1]) }) })
          : null,
      ),
      h("div", { class: "hint", text: t(prof[2]) }),
      h("div", {
        class: "hint",
        text: t("This PC: {total} memory, {free} free now, {cpus} CPU threads.", {
          total: formatBytes(s.memory.total),
          free: formatBytes(s.memory.available),
          cpus: s.cpus,
        }),
      }),
      h("div", { class: "subhead", text: t("Engine") }),
      runtimeBlock(s),
      h("div", { class: "subhead", text: t("Installed models") }),
    );
    if (s.local.length === 0) body.append(h("div", { class: "hint", text: t("None yet. Download one below, or put a .gguf file in the models folder.") }));
    for (const m of s.local) body.append(localRow(m, s));

    const controls = h("div", { class: "row" });
    if (s.running) {
      const stop = h("button", { text: s.runningMemory
        ? t("Local model loaded, using {gb} GB: Unload", { gb: (s.runningMemory / 1024 ** 3).toFixed(1) })
        : t("Unload {file}", { file: s.running }) }) as HTMLButtonElement;
      stop.addEventListener("click", () => void run(stop, () => Bridge.modelsStop()));
      controls.append(stop);
    } else if (s.local.some((m) => m.info) && s.runtimePath) {
      const start = h("button", { class: "primary", text: t("Load now") }) as HTMLButtonElement;
      start.addEventListener("click", () => void run(start, () => Bridge.modelsStart()));
      controls.append(start);
    }
    const folder = h("button", { text: t("Open the models folder") });
    folder.addEventListener("click", () => void Bridge.modelsRevealFolder());
    controls.append(folder);
    body.append(controls);

    body.append(h("div", { class: "subhead", text: t("Download a model") }));
    for (const c of s.catalog) {
      const row = catalogRow(c, s);
      if (row) body.append(row);
    }
    if (s.downloading) {
      const cancel = h("button", { text: t("Cancel the download") });
      cancel.addEventListener("click", () => void Bridge.modelsCancelDownload());
      body.append(h("div", { class: "row" }, progress, cancel));
    }
    body.append(
      h("div", {
        class: "hint",
        text: t("Small models are much weaker than online ones. Memory and speed were measured with llama.cpp on a 4-thread CPU; a laptop may be a little slower. Every file is checked against its SHA-256 and deleted if it doesn't match."),
      }),
    );
  }

  async function refresh() {
    status = await Bridge.modelsStatus();
    if (status) draw(status);
    else {
      clear(body);
      body.append(h("div", { class: "hint", text: t("Open Groovy's settings from the app to manage models.") }));
    }
  }

  void onEvent<{ what: string; done: number; total: number }>("model-download", (p) => {
    progress.textContent = t("Downloading {what}: {done} of {total}", {
      what: p.what,
      done: formatBytes(p.done),
      total: formatBytes(p.total),
    });
    if (status && !status.downloading) {
      status.downloading = true;
      draw(status);
    }
  });
  void refresh();
  return el;
}
