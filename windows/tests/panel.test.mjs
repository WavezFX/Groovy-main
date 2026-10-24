// Tiles open in the island (FIXES_ROUND3C item 4), and since round 3D item 8
// they grow in place on the tiles: Spotify to 4, Mail, This PC and Reminders
// to 6, with Back, and Mail can read a message and reply.
import assert from "node:assert/strict";
import { test } from "node:test";
import { installFakeDom } from "./fakedom.mjs";
import { internals, sent } from "./tauri.mjs";

installFakeDom();
const { face, Mail } = await import("../src/views/tiles.ts");
const { bytes, connectAccount, localInput, speed, SYSTEM_EVERY_MS } = await import("../src/views/panel.ts");
const { GROOVY_LIMIT, inboxRows, mailGrown, meterRows, systemGrown } = await import("../src/views/expand.ts");
const { State } = await import("../src/core/state.ts");
const { HOME_GROWN, PANEL_W, PANEL_H } = await import("../src/core/layout.ts");

const byText = (el, text) => [...el.walk()].find((e) => e.tagName === "BUTTON" && e.textContent === text);

test("Reminders, This PC, Mail and Spotify tiles grow in place on the tiles", () => {
  for (const [kind, grown] of [["reminders", "reminders"], ["system", "system"], ["mail", "mail"], ["music", "music"]]) {
    const seen = [];
    State.view = "home";
    State.homeExpanded = null;
    face({ kind, target: "", label: "" }, (v) => seen.push(v)).open();
    assert.deepEqual(seen, [], `${kind}: no other view, it grows where it is`);
    assert.equal(State.homeExpanded, grown);
  }
  // From another view, the island goes back to the tiles with it grown.
  const seen = [];
  State.view = "prompt";
  face({ kind: "mail", target: "", label: "" }, (v) => seen.push(v)).open();
  assert.deepEqual(seen, ["home"]);
  State.homeExpanded = null;
  State.view = "home";
  assert.ok(HOME_GROWN.w <= PANEL_W && HOME_GROWN.h <= PANEL_H, "it stays inside the island window");
});

test("This PC grown: meters with Groovy against 400 MB, Back, and it reads only while open", () => {
  const backs = [];
  const g = systemGrown(() => backs.push(1));
  const before = sent("system_status").length;
  g.shown(true);
  assert.equal(sent("system_status").length, before + 1, "reads at once");
  g.shown(false);
  g.el.querySelector(".grow-back").fire("click");
  assert.deepEqual(backs, [1]);
  assert.equal(SYSTEM_EVERY_MS, 2000);
  const rows = meterRows({ memoryTotal: 16 * 1024 ** 3, memoryAvailable: 6 * 1024 ** 3, cpuUsage: 12, groovyMemory: 212 * 1024 ** 2, disks: [{ mount: "C:", total: 100, available: 40 }] });
  assert.deepEqual(rows.map((r) => r.name), ["Memory", "CPU", "Disk", "Groovy"]);
  assert.equal(rows[3].value, "212 MB / 400 MB");
  assert.equal(GROOVY_LIMIT, 400 * 1024 * 1024);
  assert.ok(byText(g.el, "Controls"), "volume, brightness, Wi-Fi, Bluetooth and Do not disturb are one tap away");
});

test("network speed comes from two readings; sizes and picker values read well", () => {
  const a = { netReceived: 1000, netSent: 0 };
  const b = { netReceived: 1000 + 2 * 1024 * 1024, netSent: 4096 };
  assert.deepEqual(speed(a, b, 2000), { down: 1024 * 1024, up: 2048 });
  assert.equal(speed(null, b, 2000), null);
  assert.equal(bytes(3 * 1024 ** 3), "3.0 GB");
  assert.equal(bytes(5 * 1024 ** 2), "5 MB");
  assert.match(localInput(0 + new Date(2026, 9, 17, 9, 5).getTime() / 1000), /^2026-10-17T09:05$/);
});

const inbox = (id, email, unread, messages) => ({
  account: { id, kind: "imap", email, host: "imap.x.com", port: 993, username: email },
  unread, messages, error: null, checkedAt: 0, canModify: true,
});

test("connected mail (round 3C item 5): the tile counts the unread; grown, it lists every inbox, reads and replies", async () => {
  Mail.inboxes = [
    inbox(1, "me@icloud.com", 2, [
      { account: 1, id: "8", from: "Bob", subject: "Two", snippet: "", at: 200, unread: true, link: null },
      { account: 1, id: "7", from: "Anna", subject: "One", snippet: "", at: 100, unread: false, link: null },
    ]),
    inbox(2, "me@gmail.com", 1, [{ account: 2, id: "x", from: "GitHub", subject: "PR", snippet: "", at: 150, unread: true, link: "https://mail.google.com/" }]),
  ];
  const f = face({ kind: "mail", target: "", label: "" }, () => {});
  assert.equal(f.title, "Mail · 3 unread");
  assert.equal(f.sub, "Bob: Two", "the newest unread");
  assert.deepEqual(inboxRows(Mail.inboxes).map((r) => r.m.from), ["Bob", "GitHub", "Anna"], "one inbox, newest first");
  const sizes = [];
  const g = mailGrown(() => {}, () => sizes.push(1));
  const refreshes = sent("mail_refresh").length;
  g.shown(true);
  assert.match(g.el.textContent, /Inbox · 3 unread/);
  assert.match(g.el.textContent, /Bob.*GitHub.*Anna/);
  assert.equal(sent("mail_refresh").length, refreshes + 1, "growing it checks once");
  byText(g.el, "All 8").fire("click");
  assert.deepEqual(sizes, [1]);

  // Open Bob's message: its text is fetched now, and it is marked read.
  const real = internals.invoke;
  internals.invoke = async (cmd, args) => {
    const r = await real(cmd, args);
    return cmd === "mail_read" ? { from: "Bob", subject: "Two", text: "Are we still on?" } : r;
  };
  [...g.el.walk()].find((e) => e.classList.contains("grow-row") && e.textContent.startsWith("B")).fire("click");
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(sent("mail_read").at(-1), { account: 1, id: "8" });
  assert.match(g.el.textContent, /Are we still on\?/);
  assert.deepEqual(sent("mail_act").at(-1), { account: 1, id: "8", action: "read" });
  assert.ok(byText(g.el, "Archive"));
  assert.ok(![...g.el.walk()].some((e) => e.textContent.startsWith("Open in")), "an IMAP server with no web mail has nothing to open in");

  // Reply, polish, send.
  byText(g.el, "Reply").fire("click");
  const area = [...g.el.walk()].find((e) => e.tagName === "TEXTAREA");
  area.value = "sounds good";
  internals.invoke = async (cmd, args) => {
    const r = await real(cmd, args);
    return cmd === "mail_polish" ? "Sounds good, see you then." : r;
  };
  byText(g.el, "Groovy, polish it").fire("click");
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(area.value, "Sounds good, see you then.");
  internals.invoke = real;
  byText(g.el, "Send").fire("click");
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(sent("mail_reply").at(-1), { account: 1, id: "8", text: "Sounds good, see you then." });
  assert.match(g.el.textContent, /Inbox/, "sent: back to the inbox");
  g.shown(false);
  Mail.inboxes = [];
});

test("iCloud and IMAP connect in Settings → Mail (they need a password)", async () => {
  await connectAccount("icloud");
  assert.deepEqual(sent("open_settings_window").at(-1), { page: "mail" });
});
