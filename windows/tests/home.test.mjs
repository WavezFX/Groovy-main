// The island's normal state (FIXES_ROUND3 item 2, src/core/home.ts): what
// comes up next, which tiles show on which page, and when the player shows.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CELLS_E, CELLS_F, MEETING_WINDOW_MIN, PLAYER_HOLD_MS, PlayerHold, cellsUnderPlayer, nextMeeting, pagesOf, upNext,
} from "../src/core/home.ts";
import { tile } from "../src/core/tiles.ts";

const now = Date.UTC(2026, 9, 16, 9, 0);
const base = { now, hour: 9, meetings: [], mail: null, agent: null, reminder: null };
const meeting = (inMin, title = "Standup", url = null) => ({ title, start: now + inMin * 60_000, url, source: "Cal.com" });

test("a meeting within 30 minutes comes before everything else", () => {
  const next = upNext({
    ...base,
    meetings: [meeting(15, "Design review", "https://meet.example/x")],
    mail: { app: "Outlook", title: "Ana", body: "Hi" },
    agent: { name: "Claude Code", kind: "approval", text: "Bash" },
    reminder: { text: "Water", dueAt: now },
  });
  assert.equal(next.kind, "meeting");
  assert.equal(next.label, "UP NEXT · IN 15 MIN");
  assert.equal(next.title, "Design review");
  assert.equal(next.url, "https://meet.example/x");
  assert.match(next.detail, /^Cal\.com · \d\d:\d\d$/);
});

test("a meeting further away, or long started, does not count", () => {
  assert.equal(nextMeeting([meeting(MEETING_WINDOW_MIN + 1)], now), null);
  assert.equal(nextMeeting([meeting(-6)], now), null);
  assert.equal(nextMeeting([meeting(-2)], now)?.title, "Standup");
  assert.equal(upNext({ ...base, meetings: [meeting(0)] }).label, "UP NEXT · NOW");
  // The soonest of several.
  assert.equal(nextMeeting([meeting(20, "B"), meeting(5, "A")], now)?.title, "A");
});

test("then new mail, then an agent, then a reminder due, then a greeting", () => {
  const mail = { app: "Outlook", title: "Ana", body: "Lunch?" };
  const agent = { name: "Claude Code", kind: "finished", text: "Tests passed" };
  const reminder = { text: "Water the plants", dueAt: now + 10 * 60_000 };
  assert.equal(upNext({ ...base, mail, agent, reminder }).kind, "mail");
  assert.equal(upNext({ ...base, agent, reminder }).title, "Claude Code");
  assert.equal(upNext({ ...base, agent: { ...agent, kind: "approval" } }).label, "NEEDS YOU");
  assert.equal(upNext({ ...base, reminder }).kind, "reminder");
  // A reminder an hour away is not "up next".
  assert.equal(upNext({ ...base, reminder: { ...reminder, dueAt: now + 3_600_000 } }).kind, "greeting");
  assert.equal(upNext({ ...base, hour: 20 }).title, "Good evening");
  assert.equal(upNext({ ...base, hour: 9 }).title, "Good morning");
});

test("eight cells a page, one row of four under the player, the add tile last", () => {
  const cells = Array.from({ length: 7 }, (_, i) => tile("service", `integration_${i}`));
  const f = pagesOf(cells, CELLS_F, "+");
  assert.equal(f.length, 1);
  assert.equal(f[0].at(-1), "+");
  const more = pagesOf([...cells, tile("mail")], CELLS_F, "+");
  assert.deepEqual(more.map((p) => p.length), [8, 1]);
  assert.deepEqual(pagesOf(cells, CELLS_E, "+").map((p) => p.length), [4, 4]);
  assert.deepEqual(cellsUnderPlayer([tile("music"), tile("mail")]).map((c) => c.id), ["mail"]);
});

test("the player shows at once and folds 1.5 s after the music stops", () => {
  assert.equal(PLAYER_HOLD_MS, 1500);
  const hold = new PlayerHold();
  assert.equal(hold.update(false, 0), false);
  assert.equal(hold.update(true, 1000), true);
  assert.equal(hold.update(false, 2000), true, "a pause doesn't fold it at once");
  assert.equal(hold.wait(3000), PLAYER_HOLD_MS - 1000);
  assert.equal(hold.update(true, 4000), true, "a quick skip keeps it");
  assert.equal(hold.update(false, 5000), true);
  assert.equal(hold.update(false, 5000 + PLAYER_HOLD_MS - 1), true);
  assert.equal(hold.update(false, 5000 + PLAYER_HOLD_MS), false);
  assert.equal(hold.wait(20_000), null);
});
