// Settings → Mail (round 3C item 5): connect Gmail and Outlook by signing in
// in the browser, iCloud Mail with an app-specific password, or any IMAP
// server; see and disconnect what is connected. Passwords go straight to
// Rust, which keeps them in Windows Credential Manager.

import { Bridge, type MailAccount } from "../core/bridge";
import { t } from "../i18n/i18n";
import { clear, h } from "../views/dom";
import { errorText } from "../views/toast";
import { confirmClick } from "./parts";

/** Apple's page for app-specific passwords. */
export const APPLE_PASSWORDS = "https://appleid.apple.com/";

const KIND_NAMES: Record<MailAccount["kind"], string> = { gmail: "Gmail", outlook: "Outlook", icloud: "iCloud Mail", imap: "IMAP" };

function input(type: string, placeholder: string, value = ""): HTMLInputElement {
  return h("input", { type, placeholder, value, autocomplete: "off", spellcheck: "false", style: "flex:1 1 auto;min-width:0" }) as HTMLInputElement;
}

export function mailSection(): HTMLElement {
  const accounts = h("div", { style: "display:flex;flex-direction:column;gap:6px" });
  const message = h("div", {});
  const say = (text: string, ok: boolean) => {
    clear(message);
    if (text) message.append(h("div", { class: ok ? "notice ok" : "notice err", text }));
  };

  async function draw() {
    clear(accounts);
    const list = (await Bridge.mailAccounts()) ?? [];
    if (!list.length) accounts.append(h("div", { class: "hint", text: t("No account connected yet.") }));
    for (const a of list) {
      const remove = h("button", { class: "danger", text: t("Disconnect") }) as HTMLButtonElement;
      confirmClick(remove, t("Sure?"), async () => {
        try {
          await Bridge.mailDisconnect(a.id);
          say(t("Disconnected. Its password or sign-in and what Groovy read from it are deleted."), true);
        } catch (e) {
          say(errorText(e), false);
        }
        await draw();
      });
      accounts.append(h("div", { class: "row" },
        h("span", { style: "flex:1 1 auto", text: `${a.email} · ${KIND_NAMES[a.kind]}${a.kind === "imap" ? ` (${a.host})` : ""}` }),
        remove,
      ));
    }
  }

  const done = async (what: Promise<MailAccount>) => {
    try {
      const a = await what;
      say(t("{email} is connected. The island checks it every 3 minutes.", { email: a.email }), true);
    } catch (e) {
      say(errorText(e), false);
    }
    await draw();
  };

  // Gmail and Outlook: the provider's own sign-in in the browser.
  const oauthRow = (provider: "gmail" | "outlook", name: string) => {
    const go = h("button", { class: "primary", text: t("Connect {name}", { name }) });
    const setup = h("div", { style: "display:none;flex-direction:column;gap:6px" });
    const id = input("text", t("Client ID"));
    const secret = input("password", t("Client secret"));
    const save = h("button", { text: t("Save") });
    setup.append(
      h("div", { class: "hint", text: provider === "gmail"
        ? t("This build has no Google sign-in yet. Paste the client ID and secret of a Google Cloud “Desktop app” with the Gmail API turned on.")
        : t("This build has no Microsoft sign-in yet. Paste the application (client) ID of an Azure app registration for personal and work accounts, with the redirect http://127.0.0.1.") }),
      h("div", { class: "row" }, id, provider === "gmail" ? secret : null, save),
    );
    save.addEventListener("click", async () => {
      try {
        await Bridge.mailOauthConfigure(provider, id.value, secret.value);
        secret.value = "";
        setup.style.display = "none";
        say(t("Saved. Now press Connect."), true);
      } catch (e) {
        say(errorText(e), false);
      }
    });
    go.addEventListener("click", async () => {
      const ready = await Bridge.mailOauthReady();
      if (!(provider === "gmail" ? ready?.google : ready?.microsoft)) {
        setup.style.display = "flex";
        return;
      }
      say(t("Finish signing in in your browser…"), true);
      await done(Bridge.mailConnectOauth(provider));
    });
    return h("div", {}, h("div", { class: "row" }, h("label", { text: name }), go), setup);
  };

  // iCloud: the Apple ID and an app-specific password.
  const icloudEmail = input("email", t("Apple ID email"));
  const icloudPass = input("password", t("App-specific password"));
  const icloudGo = h("button", { class: "primary", text: t("Connect {name}", { name: "iCloud" }) });
  icloudGo.addEventListener("click", () => {
    const pass = icloudPass.value;
    icloudPass.value = "";
    void done(Bridge.mailConnectImap("icloud", icloudEmail.value, "", 993, "", pass));
  });

  // Any other IMAP server.
  const imapEmail = input("email", t("Email address"));
  const imapHost = input("text", "imap.example.com");
  const imapPort = input("number", "993", "993");
  imapPort.style.flex = "0 0 80px";
  const imapUser = input("text", t("Username (if not the address)"));
  const imapPass = input("password", t("Password or app password"));
  const imapGo = h("button", { class: "primary", text: t("Connect {name}", { name: "IMAP" }) });
  imapGo.addEventListener("click", () => {
    const pass = imapPass.value;
    imapPass.value = "";
    void done(Bridge.mailConnectImap("imap", imapEmail.value, imapHost.value, Number(imapPort.value) || 993, imapUser.value, pass));
  });

  void draw();
  return h(
    "section",
    { id: "mail" },
    h("h2", { text: t("Mail accounts") }),
    message,
    accounts,
    h("h3", { text: t("Connect an account") }),
    oauthRow("gmail", "Gmail"),
    oauthRow("outlook", "Outlook / Hotmail"),
    h("div", { class: "row" }, h("label", { text: "iCloud Mail" }), icloudEmail, icloudPass, icloudGo),
    h("div", { class: "hint" },
      t("iCloud needs an app-specific password, not your Apple ID password: "),
      h("a", { href: "#", text: t("how to make one"), onclick: (e: Event) => {
        e.preventDefault();
        void Bridge.openUrl(APPLE_PASSWORDS);
      } }),
      t(" (Sign-In and Security → App-Specific Passwords)."),
    ),
    h("div", { class: "row" }, h("label", { text: t("Other (IMAP)") }), imapEmail, imapHost, imapPort),
    h("div", { class: "row" }, h("label", { text: "" }), imapUser, imapPass, imapGo),
    h("div", { class: "hint", text: t("Groovy reads only the sender, subject and a short preview of your newest mail, never whole messages, and only from the mail services themselves. Passwords and sign-ins are kept in Windows Credential Manager. Disconnecting deletes them and everything read.") }),
  );
}
