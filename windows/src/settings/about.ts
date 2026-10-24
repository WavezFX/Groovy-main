// App → About & updates (round 3D item 1): the version, where updates come
// from, and the licences of what Groovy is built with.

import { Bridge } from "../core/bridge";
import { N_, t } from "../i18n/i18n";
import { h } from "../views/dom";

const RELEASES = "https://github.com/WavezFX/Groovy-main/releases";

/** What Groovy is built with, and under which licence. */
export const CREDITS: [string, string][] = [
  ["Tauri", "MIT / Apache-2.0"],
  ["sherpa-onnx", "Apache-2.0"],
  ["NVIDIA Parakeet", "CC-BY-4.0"],
  ["Kokoro", "Apache-2.0"],
  ["llama.cpp", "MIT"],
  ["SQLite (rusqlite)", N_("Public domain / MIT")],
  ["Symphonia, lofty, rodio", "MPL-2.0 / MIT / Apache-2.0"],
  ["rustls, ring", "Apache-2.0 / ISC"],
];

export function aboutSection(version: string): HTMLElement {
  return h(
    "section",
    {},
    h("h2", { text: t("Version") }),
    h("div", { class: "row" },
      h("label", { text: `Groovy ${version}` }),
      h("button", { text: t("See what's new and download updates"), onclick: () => void Bridge.openUrl(RELEASES) }),
    ),
    h("h2", { text: t("Licences") }),
    h("div", { class: "row" }, h("label", { text: "Groovy" }), h("span", { class: "hint", text: "GPL-2.0" })),
    ...CREDITS.map(([name, licence]) => h("div", { class: "row" }, h("label", { text: name }), h("span", { class: "hint", text: t(licence) }))),
    h("div", { class: "hint", text: t("Some of Groovy's code is used under the MIT licence; its notice ships with Groovy. Speech, voice and local AI models are downloaded only when you ask, each under its own licence, shown where you download it.") }),
  );
}
