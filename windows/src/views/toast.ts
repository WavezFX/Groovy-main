// When something the user clicked fails, say so. The backend's error text is
// shown in a toast in windows that have room for one (Control Centre, Studio),
// and in the floating pill for the island and Groovy on the desktop, whose
// windows are only as big as what they draw. Background polling stays quiet:
// only user actions report (bridge.ts and flow.ts decide which), plus any
// promise nobody caught, which would otherwise be a silent dead button.

import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n/i18n";

export const ERROR_EVENT = "groovy-error";

/** The text of an error from invoke(): a string, an Error or anything else. */
export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  if (err && typeof err === "object" && "message" in err && typeof (err as { message: unknown }).message === "string") {
    return (err as { message: string }).message;
  }
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

/** Reports a failed user action; shown by whichever reporter the window installed. */
export function showError(err: unknown) {
  const text = errorText(err).trim() || t("Something went wrong.");
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent<string>(ERROR_EVENT, { detail: text }));
}

let installed = false;

/**
 * Shows reported errors. `mode`: "page" draws toasts in this window; "notice"
 * hands them to Rust, which shows them as a notice in the island (notify_error).
 */
export function installErrorReporting(mode: "page" | "notice") {
  if (installed || typeof window === "undefined") return;
  installed = true;
  let box: HTMLElement | null = null;
  const show = (text: string) => {
    if (mode === "notice") {
      void invoke("notify_error", { text }).catch(() => {});
      return;
    }
    if (!box) {
      box = document.createElement("div");
      box.className = "toasts";
      box.setAttribute("role", "status");
      box.setAttribute("aria-live", "polite");
      document.body.append(box);
    }
    // The same error twice in a row is shown once.
    if (box.lastElementChild?.querySelector(".toast-text")?.textContent === text) return;
    const toast = document.createElement("div");
    toast.className = "toast";
    const span = document.createElement("span");
    span.className = "toast-text";
    span.textContent = text;
    const close = document.createElement("button");
    close.className = "toast-close";
    close.textContent = "×";
    close.title = t("Close");
    close.addEventListener("click", () => toast.remove());
    toast.append(span, close);
    box.append(toast);
    while (box.childElementCount > 3) box.firstElementChild?.remove();
    setTimeout(() => toast.remove(), 8000);
  };
  window.addEventListener(ERROR_EVENT, (e) => show((e as CustomEvent<string>).detail));
  window.addEventListener("unhandledrejection", (e) => {
    console.error("[groovy] unhandled", e.reason);
    show(errorText(e.reason));
  });
}
