// What the Studio does with an AI answer (studio.rs SYSTEM tells the model
// this format): files proposed in full under "### path" headings, and shell
// commands in ```shell blocks. Pure, so it is tested without a window.
//
// Nothing here runs or writes anything: the Studio shows proposals as diffs
// with Apply buttons, and commands with Run buttons.

export interface FileProposal {
  path: string;
  content: string;
}

export interface Proposals {
  files: FileProposal[];
  commands: string[];
}

const SHELL_LANGS = new Set(["shell", "sh", "bash", "powershell", "ps", "ps1", "cmd", "bat", "console", "terminal"]);

/** A proposed path is only taken when it is relative and stays inside the project. */
export function safePath(raw: string): string | null {
  const p = raw.trim().replace(/^`+|`+$/g, "").replace(/\\/g, "/").replace(/^\.\//, "");
  if (!p || p.length > 260) return null;
  if (p.startsWith("/") || /^[a-z]:/i.test(p) || p.includes("\0")) return null;
  const parts = p.split("/");
  if (parts.some((s) => s === ".." || s === "")) return null;
  if (!/^[\w.\- /@+()[\]]+$/.test(p)) return null;
  // A file name: with an extension or a folder, or one of the usual bare names
  // ("### Explanation" is a heading, not a file).
  const base = parts[parts.length - 1];
  const bare = ["Makefile", "Dockerfile", "LICENSE", "Procfile", "Gemfile", "Rakefile", "Justfile", "CODEOWNERS"];
  if (!base.includes(".") && !bare.includes(base)) return null;
  return p;
}

/** Files and commands found in an answer, in order. A later block for the same path wins. */
export function parseProposals(answer: string): Proposals {
  const files = new Map<string, string>();
  const commands: string[] = [];
  const lines = answer.split(/\r?\n/);
  let pendingPath: string | null = null;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const heading = /^#{2,4}\s+(?:File:\s*)?`?([^`]+?)`?\s*$/.exec(line);
    if (heading) {
      pendingPath = safePath(heading[1]);
      continue;
    }
    const fence = /^(`{3,}|~{3,})\s*([\w+#.-]*)\s*$/.exec(line);
    if (!fence) {
      // A heading only names the file when its code block comes right after it.
      if (line.trim() !== "") pendingPath = null;
      continue;
    }
    const marker = fence[1];
    const lang = fence[2].toLowerCase();
    const body: string[] = [];
    let j = i + 1;
    for (; j < lines.length; j++) {
      if (lines[j].startsWith(marker) && lines[j].trim() === marker.trim().slice(0, marker.length)) break;
      if (new RegExp(`^${marker[0]}{${marker.length},}\\s*$`).test(lines[j])) break;
      body.push(lines[j]);
    }
    i = j;
    const text = body.join("\n");
    if (SHELL_LANGS.has(lang) && !pendingPath) {
      for (const c of text.split("\n").map((l) => l.replace(/^\s*[$>]\s+/, "").trim())) {
        if (c && !c.startsWith("#") && c.length <= 1000) commands.push(c);
      }
    } else if (pendingPath) {
      files.set(pendingPath, text.endsWith("\n") || text === "" ? text : `${text}\n`);
    }
    pendingPath = null;
  }
  return { files: [...files].map(([path, content]) => ({ path, content })), commands };
}

/** "error[E0425]: … --> src/main.rs:4:5", "main.py:12: error", "app.ts(3,7): error TS2304" → places to jump to. */
export interface Problem {
  path: string;
  line: number;
  message: string;
}

export function parseProblems(output: string, limit = 100): Problem[] {
  const out: Problem[] = [];
  const seen = new Set<string>();
  const lines = output.split(/\r?\n/);
  const add = (path: string, line: number, message: string) => {
    const p = safePath(path);
    if (!p || !(line > 0)) return;
    const key = `${p}:${line}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push({ path: p, line, message: message.trim().slice(0, 300) });
  };
  for (let i = 0; i < lines.length && out.length < limit; i++) {
    const l = lines[i];
    let m = /^\s*-->\s+([^:\s][^:]*):(\d+):\d+/.exec(l); // rustc
    if (m) {
      add(m[1], Number(m[2]), lines[i - 1] ?? "");
      continue;
    }
    m = /^\s*File "([^"]+)", line (\d+)/.exec(l); // Python traceback
    if (m) {
      add(m[1], Number(m[2]), lines[i + 2] ?? lines[i + 1] ?? "");
      continue;
    }
    m = /^([^\s:(][^:(]*)\((\d+),\d+\):\s*(error.*)$/i.exec(l); // tsc
    if (m) {
      add(m[1], Number(m[2]), m[3]);
      continue;
    }
    m = /^([^\s:][^:]*\.[a-z0-9]+):(\d+)(?::\d+)?:\s*(.*(?:error|warning|Error|failed).*)$/.exec(l); // gcc, eslint, pytest
    if (m) add(m[1], Number(m[2]), m[3]);
  }
  return out;
}

/** The CodeMirror language to load for a file name. */
export function languageOf(path: string): "javascript" | "typescript" | "jsx" | "tsx" | "python" | "rust" | "html" | "css" | "json" | "markdown" | null {
  const ext = path.toLowerCase().split(".").pop() ?? "";
  switch (ext) {
    case "js": case "mjs": case "cjs": return "javascript";
    case "ts": case "mts": case "cts": return "typescript";
    case "jsx": return "jsx";
    case "tsx": return "tsx";
    case "py": case "pyw": return "python";
    case "rs": return "rust";
    case "html": case "htm": case "vue": case "svelte": return "html";
    case "css": case "scss": case "less": return "css";
    case "json": case "jsonc": return "json";
    case "md": case "markdown": return "markdown";
    default: return null;
  }
}
