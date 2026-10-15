// Settings → Models: Groovy's own offline engine (models.rs). Install the
// llama.cpp runtime once, download a GGUF model (both pinned to a SHA-256 and
// checked), pick one, and chat with no internet. Every model shows its size,
// quantisation and the memory it needs with the current performance profile,
// against what this computer has free right now.

import { Bridge, onEvent, type CatalogModel, type LocalModelInfo, type ModelsStatus } from "../core/bridge";
import { DEFAULT_ASSISTANT, type AssistantPrefs, type Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { confirmClick, statusDot } from "./parts";

const PROFILES: [AssistantPrefs["performanceProfile"], string, string][] = [
  ["lightweight", N_("Lightweight"), N_("2K context, half the cores, stops after 5 idle minutes, reduced animation. For 8 GB laptops.")],
  ["balanced", N_("Balanced"), N_("4K context, most cores, stops after 15 idle minutes.")],
  ["performance", N_("High performance"), N_("8K context, every core, stays loaded. Needs 16 GB or more.")],
];

export function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  if (n >= 1024 ** 2) return `${Math.round(n / 1024 ** 2)} MB`;
  return `${Math.round(n / 1024)} KB`;
}

/** How a model fits in the memory free now, with the profile's share of it. */
export function fit(needed: number, available: number, share: number): "ok" | "tight" | "no" {
  if (needed <= available * share * 0.8) return "ok";
  if (needed <= available * share) return "tight";
  return "no";
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
    const verdict = m.info ? fit(m.ramNeeded, s.memory.available, s.profile.ramShare) : "no";
    const tag = h("span", {
      class: verdict === "ok" ? "tag" : verdict === "tight" ? "tag warn" : "tag err",
      text: m.info
        ? t("needs ~{ram}", { ram: formatBytes(m.ramNeeded) })
        : t("unreadable"),
      title: m.error ?? "",
    });
    const meta = m.info
      ? `${formatBytes(m.size)} · ${m.info.quantization} · ${m.info.architecture}${m.info.contextLength ? ` · ${Math.round(m.info.contextLength / 1024)}K` : ""}`
      : m.error ?? "";
    const use = h("button", { class: selected ? "primary" : "", text: selected ? t("In use") : t("Use") }) as HTMLButtonElement;
    use.disabled = selected || !m.info;
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
    const verdict = fit(c.size * 1.25 + 350 * 1024 ** 2, s.memory.available, s.profile.ramShare);
    const b = h("button", { text: t("Download · {size}", { size: formatBytes(c.size) }) }) as HTMLButtonElement;
    b.disabled = s.downloading;
    const download = () => void run(b, () => Bridge.modelsDownload(c.id));
    // Too big for the memory free now: one more click to say so knowingly.
    if (verdict === "no") confirmClick(b, t("Click again to download anyway"), download);
    else b.addEventListener("click", download);
    return h(
      "div",
      { class: "pill-row full" },
      h(
        "div",
        { class: "name" },
        h("div", {}, h("span", { text: c.label }), c.id === "qwen2.5-1.5b" ? h("span", { class: "tag", text: t("Recommended") }) : null),
        h("div", { class: "hint", text: `${c.goodFor} · ${c.license}` }),
      ),
      verdict === "no" ? h("span", { class: "tag err", text: t("too big for now") }) : null,
      b,
    );
  }

  function draw(s: ModelsStatus) {
    clear(body);
    const p = prefs();
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
      const stop = h("button", { text: t("Unload {file}", { file: s.running }) }) as HTMLButtonElement;
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
        text: t("Small models are much weaker than online ones and slow on a laptop CPU: expect a few words per second from a 3B model on 8 GB. Every file is checked against its SHA-256 and deleted if it doesn't match."),
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
