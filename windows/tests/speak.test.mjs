// Answers read aloud: what is worth saying, and which voice says it.

import { test } from "node:test";
import assert from "node:assert/strict";
import { speakable, pickVoice } from "../src/voice/speak.ts";

test("markdown becomes plain speech, code and links stay in the chat", () => {
  assert.equal(speakable("**Sure!** Here's how:\n\n```js\nx()\n```\n- one\n- two"), "Sure! Here's how: (code in the chat) one two");
  assert.equal(speakable("See [the docs](https://x.dev) or https://y.dev/a"), "See the docs or the link in the chat");
});

test("a long answer is cut at a sentence", () => {
  const long = "One sentence here. ".repeat(80);
  const s = speakable(long, 100);
  assert.ok(s.length < 140);
  assert.ok(s.endsWith("The rest is in the chat."));
});

test("an offline British voice is preferred, a named one wins", () => {
  const v = (name, lang, localService) => ({ name, lang, localService });
  const list = [v("Online Aria", "en-US", false), v("David", "en-US", true), v("George", "en-GB", true)];
  assert.equal(pickVoice(list, "").name, "George");
  assert.equal(pickVoice(list, "Online Aria").name, "Online Aria");
  assert.equal(pickVoice([], ""), null);
});
