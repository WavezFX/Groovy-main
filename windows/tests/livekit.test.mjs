// Round 4B item 5: LiveKit as an option. A mocked livekit-client: a wake joins
// the room, "okay bye" and the hang-up leave it, Test connection joins and leaves,
// and only LiveKit Cloud hosts are accepted.

import { test } from "node:test";
import assert from "node:assert/strict";
import { startLiveKit, testLiveKit, livekitHostAllowed } from "../src/voice/livekit.ts";

function mockLiveKit() {
  const log = [];
  const handlers = new Map();
  class Room {
    constructor() {
      this.localParticipant = { identity: "me", setMicrophoneEnabled: async (on) => log.push(`mic ${on}`) };
    }
    on(event, fn) { handlers.set(event, fn); return this; }
    async connect(url) { log.push(`connect ${url}`); }
    async disconnect() { log.push("disconnect"); }
  }
  const lk = {
    Room,
    RoomEvent: { ActiveSpeakersChanged: "a", TrackSubscribed: "t", TranscriptionReceived: "tr", Disconnected: "d" },
    Track: { Kind: { Audio: "audio" } },
  };
  return { lk, log, handlers };
}

const access = async () => ({ url: "wss://x.livekit.cloud", token: "tok" });

test("a wake joins the room and 'okay bye' leaves it first", async () => {
  const { lk, log, handlers } = mockLiveKit();
  const ends = [];
  const session = await startLiveKit({ onLive: () => log.push("live"), onEnd: (e) => ends.push(e ?? "ok") }, async () => lk, access);
  assert.deepEqual(log, ["connect wss://x.livekit.cloud", "mic true", "live"]);
  handlers.get("tr")([{ text: "play some music", final: true }], { identity: "me" });
  assert.ok(!log.includes("disconnect"), "an ordinary sentence keeps the room");
  handlers.get("tr")([{ text: "okay bye", final: true }], { identity: "me" });
  assert.equal(log.at(-1), "disconnect");
  assert.deepEqual(ends, ["ok"]);
  session.hangUp();
  assert.equal(log.filter((l) => l === "disconnect").length, 1, "left once");
});

test("the hang-up button leaves the room", async () => {
  const { lk, log } = mockLiveKit();
  const session = await startLiveKit({ onLive() {}, onEnd() {} }, async () => lk, access);
  session.hangUp();
  assert.equal(log.at(-1), "disconnect");
});

test("Test connection joins for a moment and leaves", async () => {
  const { lk, log } = mockLiveKit();
  assert.equal(await testLiveKit(1, async () => lk, access), null);
  assert.deepEqual(log, ["connect wss://x.livekit.cloud", "disconnect"]);
  const refused = await testLiveKit(1, async () => lk, async () => { throw new Error("Add your LiveKit API key"); });
  assert.match(refused, /API key/);
});

test("only LiveKit Cloud hosts are reached (the CSP is fixed at build time)", () => {
  assert.ok(livekitHostAllowed("wss://my.livekit.cloud"));
  assert.ok(!livekitHostAllowed("wss://livekit.example.com"));
  assert.ok(!livekitHostAllowed("ws://my.livekit.cloud"));
  assert.ok(!livekitHostAllowed("not a url"));
});
