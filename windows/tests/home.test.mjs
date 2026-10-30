// The island's normal state (FIXES_ROUND3 item 2, src/core/home.ts): what
// comes up next, which tiles show on which page, and when the player shows.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CELLS_F, MEETING_WINDOW_MIN, PLAYER_HOLD_MS, PlayerHold, expandedLayout, expandsTo, nextMeeting, pagesOf, upNext,
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

test("a tile grows out of its spot (round 3D item 8): Spotify to 4, Mail and This PC to 6, Mail to all 8", () => {
  const others = ["a", "b", "c", "d", "e", "f", "g"];
  const music = expandedLayout("music", others, true, false);
  assert.deepEqual(music.panel, { col: "1 / span 2", row: "1 / span 2" });
  assert.equal(music.strip, null);
  assert.deepEqual(music.cells.map((c) => [c.tile, c.spot.col, c.spot.row]), [["a", "3", "1"], ["b", "4", "1"], ["c", "3", "2"], ["d", "4", "2"]], "the other four stay visible");
  const mail = expandedLayout("mail", others, true, false);
  assert.deepEqual(mail.panel.col, "1 / span 3");
  assert.deepEqual(mail.strip, { col: "4", row: "1 / span 2" }, "the music strip while music plays");
  assert.equal(mail.cells.length, 0);
  const quiet = expandedLayout("system", others, false, false);
  assert.deepEqual(quiet.cells.map((c) => [c.tile, c.spot.col, c.spot.row]), [["a", "4", "1"], ["b", "4", "2"]], "two tiles when nothing plays");
  const full = expandedLayout("mail", others, true, true);
  assert.deepEqual([full.panel.col, full.strip, full.cells.length], ["1 / span 4", null, 0], "All 8");
  assert.equal(expandedLayout("system", others, true, true).panel.col, "1 / span 3", "only Mail takes all 8");
  assert.deepEqual(["music", "mail", "system", "reminders", "service"].map((kind) => expandsTo(tile(kind))), ["music", "mail", "system", "reminders", null]);
});
