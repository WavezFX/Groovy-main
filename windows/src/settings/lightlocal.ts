// "Light local" (FIXES_ROUND2 item 3): one recommended conversation model for
// this PC, picked by its memory (models.rs light_pick), shown with its size and
// measured speed, and set up in one click: the engine if it's missing, the
// model (both checked against their SHA-256), then Groovy uses it.

import { Bridge, type CatalogModel, type ModelsStatus } from "../core/bridge";
import type { ProviderId } from "../core/providers";
import type { AssistantPrefs, Settings } from "../core/state";
import { h } from "../views/dom";
import { showError } from "../views/toast";
import { t } from "../i18n/i18n";

const MB = 1024 ** 2;

export function lightModel(s: ModelsStatus): CatalogModel | undefined {
  return s.catalog.find((c) => c.id === s.lightPick);
}

/** Tokens a second → words a person hears: about three words for four tokens. */
export function wordsPerSecond(tokensPerSecond: number): number {
  return Math.max(1, Math.round(tokensPerSecond * 0.75));
}

/** The settings "Light local" leads to. Pure, tested. */
export function useLight(a: AssistantPrefs, file: string): { assistant: AssistantPrefs; chatProvider: ProviderId } {
  return { assistant: { ...a, aiSetup: "local", brain: "auto", localProvider: "llamacpp", localModel: file }, chatProvider: "llamacpp" };
}

export function lightLocalCard(s: ModelsStatus, settings: Settings, prefs: () => AssistantPrefs, save: () => void, done: () => void): HTMLElement | null {
  const m = lightModel(s);
  if (!m) return null;
  const have = s.local.some((x) => x.file === m.file);
  const inUse = have && prefs().aiSetup === "local" && prefs().localModel === m.file;
  const go = h("button", { class: "primary", text: inUse ? t("In use") : have ? t("Use it") : t("Download and use · {size}", { size: `${Math.round(m.size / MB)} MB` }) }) as HTMLButtonElement;
  go.disabled = inUse || s.downloading;
  go.addEventListener("click", () => {
    go.disabled = true;
    void (async () => {
      try {
        if (!s.runtimePath) {
          const rt = s.runtimes.find((r) => r.id === "cpu" && r.available);
          if (!rt) throw new Error(t("Install llama.cpp with your package manager so that llama-server is on the PATH."));
          await Bridge.modelsInstallRuntime(rt.id);
        }
        if (!have) await Bridge.modelsDownload(m.id);
        const next = useLight(prefs(), m.file);
        settings.assistant = next.assistant;
        settings.chatProvider = next.chatProvider;
        save();
      } catch (e) {
        showError(e);
      } finally {
        done();
      }
    })();
  });
  return h("div", { class: "card light-local" },
    h("div", { class: "row" },
      h("div", { class: "name" },
        h("b", { text: t("Light local: {model}", { model: m.label }) }),
        h("div", { class: "hint", text: t("{size} download, about {ram} MB of memory while loaded, about {words} words a second.", {
          size: `${Math.round(m.size / MB)} MB`,
          ram: m.ramMb,
          words: wordsPerSecond(m.tokensPerSecond),
        }) }),
        h("div", { class: "hint", text: `${m.goodFor} · ${m.license}` }),
      ),
      go,
    ),
  );
}
