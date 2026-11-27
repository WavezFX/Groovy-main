// Settings → Accounts → Discord (round 6, FIXES_ROUND6.md item 5): the
// owner's own Discord app (client ID, optional secret), Connect (Discord
// asks "Authorise Groovy?" once), status and Disconnect. Groovy ships no app
// and no secret: Discord lets an unapproved app's RPC work only for its owner
// and testers.

import { Bridge } from "../core/bridge";
import { parseDiscord } from "../core/discord";
import { t } from "../i18n/i18n";
import { clear, h } from "../views/dom";
import { errorText } from "../views/toast";
import { confirmClick } from "./parts";

const PORTAL = "https://discord.com/developers/applications";

export function discordSection(): HTMLElement {
  const message = h("div", {});
  const say = (text: string, ok: boolean) => {
    clear(message);
    if (text) message.append(h("div", { class: ok ? "notice ok" : "notice err", text }));
  };
  const status = h("div", { class: "hint" });
  const id = h("input", { type: "text", inputmode: "numeric", placeholder: t("Client ID"), "aria-label": t("Client ID"), autocomplete: "off", spellcheck: "false" }) as HTMLInputElement;
  const secret = h("input", { type: "password", placeholder: t("Client secret (only if Discord asks)"), "aria-label": t("Client secret (only if Discord asks)"), autocomplete: "off" }) as HTMLInputElement;
  const save = h("button", { text: t("Save") }) as HTMLButtonElement;
  const connect = h("button", { class: "primary", text: t("Connect") }) as HTMLButtonElement;
  const forget = h("button", { class: "danger", text: t("Disconnect") }) as HTMLButtonElement;

  async function draw() {
    const s = parseDiscord(await Bridge.discordState());
    const hasSecret = (await Bridge.discordHasSecret()) === true;
    secret.placeholder = hasSecret ? t("Client secret saved") : t("Client secret (only if Discord asks)");
    status.textContent = !s.setUp ? t("Not set up yet.") : s.authorised ? t("Connected: Groovy is approved in Discord.") : t("Saved. Press Connect and approve Groovy in Discord.");
    connect.disabled = !s.setUp;
    forget.hidden = !s.authorised && !hasSecret;
  }
  const store = async () => {
    const pass = secret.value;
    secret.value = "";
    await Bridge.discordConfigure(id.value, pass || null);
  };
  save.addEventListener("click", () => void store().then(() => say(t("Saved."), true), (e) => say(errorText(e), false)).then(draw));
  connect.addEventListener("click", () => {
    connect.disabled = true;
    say(t("Discord is asking you to approve Groovy: look at the Discord window."), true);
    void (id.value.trim() ? store() : Promise.resolve())
      .then(() => Bridge.discordAuthorise())
      .then(() => say(t("Connected. Put the Discord tile on the island and say “mute Discord”."), true), (e) => say(errorText(e), false))
      .then(draw);
  });
  confirmClick(forget, t("Sure?"), async () => {
    try {
      await Bridge.discordForget();
      say(t("Disconnected. Groovy's Discord sign-in and the saved secret are deleted."), true);
    } catch (e) {
      say(errorText(e), false);
    }
    await draw();
  });
  void draw();
  const step = (text: string) => h("li", { text });
  return h(
    "section",
    { id: "discord" },
    h("h2", { text: "Discord" }),
    message,
    status,
    h("ol", { class: "hint" },
      h("li", {},
        h("a", { href: "#", text: t("Open the Discord Developer Portal"), onclick: (e: Event) => {
          e.preventDefault();
          void Bridge.openUrl(PORTAL);
        } }),
        document.createTextNode(` ${t("and make a New Application (any name, for example Groovy).")}`),
      ),
      step(t("Under OAuth2, copy the Client ID, and add the redirect http://127.0.0.1.")),
      step(t("Paste it here, press Connect, and approve Groovy in Discord. Only if Discord refuses, paste the Client secret too.")),
    ),
    h("div", { class: "row" }, h("label", { text: t("Client ID") }), id, save),
    h("div", { class: "row" }, h("label", { text: "" }), secret, connect, forget),
    h("div", { class: "hint", text: t("Mute, deafen, the call and who's speaking come from the Discord app on this PC, only while the Discord tile is shown or you ask. Messages come from Discord's Windows notifications and stay in memory. Groovy opens the conversation and drafts your reply; you press Enter. It never uses your Discord account's token.") }),
  );
}
