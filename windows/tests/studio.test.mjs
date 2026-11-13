// The Coding Studio's parsing of AI answers and build output (src/studio/ai.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import { languageOf, parseProblems, parseProposals, safePath } from "../src/studio/ai.ts";

test("files proposed under headings, commands in shell blocks", () => {
  const answer = [
    "Here is the calculator.",
    "",
    "### src/main.py",
    "```python",
    "print(1 + 1)",
    "```",
    "",
    "#### `README.md`",
    "```markdown",
    "# Calc",
    "```",
    "",
    "Run it with:",
    "```shell",
    "$ python src/main.py",
    "# a comment",
    "pip install -r requirements.txt",
    "```",
    "",
    "```python",
    "not a file: no heading",
    "```",
  ].join("\n");
  const p = parseProposals(answer);
  assert.deepEqual(p.files, [
    { path: "src/main.py", content: "print(1 + 1)\n" },
    { path: "README.md", content: "# Calc\n" },
  ]);
  assert.deepEqual(p.commands, ["python src/main.py", "pip install -r requirements.txt"]);
});

test("the last block for a path wins, and longer fences hold inner ones", () => {
  const p = parseProposals("### a.md\n````md\n```js\nx\n```\n````\n### a.md\n```\nv2\n```");
  assert.deepEqual(p.files, [{ path: "a.md", content: "v2\n" }]);
  const q = parseProposals("### doc.md\n````md\n```js\nx\n```\n````");
  assert.equal(q.files[0].content, "```js\nx\n```\n");
});

test("paths that leave the project are refused", () => {
  assert.equal(safePath("src/app.ts"), "src/app.ts");
  assert.equal(safePath("./src/app.ts"), "src/app.ts");
  assert.equal(safePath("src\\win.rs"), "src/win.rs");
  for (const bad of ["../x", "/etc/passwd", "C:\\Windows\\x", "a//b", "a/../b", "", "x\u0000y", "a;rm -rf.sh"]) {
    assert.equal(safePath(bad), null, bad);
  }
  assert.deepEqual(parseProposals("### ../../evil.sh\n```\nboom\n```").files, []);
  // A heading that is not a file name, or not followed straight by its block.
  assert.deepEqual(parseProposals("### Explanation\n```python\nx\n```").files, []);
  assert.deepEqual(parseProposals("### main.py\nSome words first.\n```python\nx\n```").files, []);
  assert.deepEqual(parseProposals("### Makefile\n```make\nall:\n```").files, [{ path: "Makefile", content: "all:\n" }]);
});

test("errors in build output point at files and lines", () => {
  const out = [
    "error[E0425]: cannot find value `x` in this scope",
    " --> src/main.rs:4:5",
    'Traceback (most recent call last):',
    '  File "app/main.py", line 12, in <module>',
    "    foo()",
    "NameError: name 'foo' is not defined",
    "src/index.ts(3,7): error TS2304: Cannot find name 'y'.",
    "lib/util.c:10:2: error: expected ';'",
    "src/main.rs:4:5: error: duplicate",
    "/etc/passwd:1: error: nope",
  ].join("\n");
  assert.deepEqual(parseProblems(out).map((p) => `${p.path}:${p.line}`), ["src/main.rs:4", "app/main.py:12", "src/index.ts:3", "lib/util.c:10"]);
  assert.match(parseProblems(out)[0].message, /cannot find value/);
  assert.match(parseProblems(out)[1].message, /NameError/);
});

test("languages by extension", () => {
  assert.equal(languageOf("a/b.rs"), "rust");
  assert.equal(languageOf("x.TSX"), "tsx");
  assert.equal(languageOf("Makefile"), null);
});
