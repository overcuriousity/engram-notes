// Screenshots the UI served by `pnpm dev` in headless Chromium, with Tauri's
// IPC answered from a fixture, for machines where a desktop screenshot fails.
// Usage: node scripts/shot.mjs <fixture.json> <out.png> [width] [height]
// Env: CHROME (binary), SETTLE (ms to wait), EVAL (expression run before the shot).
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [, , fixturePath, out, width = "1280", height = "820"] = process.argv;
const CHROME = process.env.CHROME ?? `${process.env.HOME}/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`;
const PORT = 9333;
const URL_ = "http://localhost:5173/";
const fixture = readFileSync(fixturePath, "utf8");
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const mock = `
(() => {
  const FX = ${fixture};
  let cb = 0;
  const md = (p) => p.toLowerCase().endsWith(".md");
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (fn) => { const id = ++cb; window["_" + id] = fn; return id; },
    unregisterCallback: () => {},
    convertFileSrc: (p) => "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="160" height="120"><rect width="160" height="120" fill="#7aa2f7"/><text x="10" y="65" font-size="14">' + p.split("/").pop() + '</text></svg>'),
    invoke: async (cmd, args = {}) => {
      window.__calls = (window.__calls || []).concat([[cmd, args]]);
      const file = (p) => FX.files.find((f) => f.path === p);
      switch (cmd) {
        case "startup_vault": return FX.root;
        case "recent_vaults": return [];
        case "open_vault": return { root: FX.root, config: FX.config, stats: { added: 0, updated: 0, removed: 0, unchanged: 0 }, index_recreated: !!FX.index_recreated, watch_error: FX.watch_error ?? null };
        case "list_files": return FX.files.map((f) => ({ path: f.path, mtime_ms: 0, size: f.size ?? (f.text ?? "").length, is_markdown: md(f.path) }));
        case "titles": return FX.files.filter((f) => md(f.path)).map((f) => [f.path, f.path.split("/").pop().replace(/\\.md$/i, "")]);
        case "get_workspace": return FX.workspace ?? {};
        case "get_config": return FX.config;
        case "read_note": { const f = file(args.path); if (!f) throw { code: "not_found", message: "not found: " + args.path }; return { path: f.path, text: f.text, mtime_ms: 0 }; }
        case "tags": return FX.tags ?? [];
        case "backlinks": return FX.backlinks?.[args.path] ?? [];
        case "outgoing": return FX.outgoing?.[args.path] ?? [];
        case "properties": return FX.properties?.[args.path] ?? {};
        case "attachment_path": return FX.root + "/" + args.path;
        case "anchor_line": return FX.anchors?.[args.path + "#" + args.fragment] ?? null;
        case "resolve_link": { const f = FX.files.find((x) => md(x.path) && x.path.split("/").pop().replace(/\\.md$/i, "").toLowerCase() === String(args.target).toLowerCase()); return f ? f.path : null; }
        case "list_folders": return FX.folders ?? [...new Set(FX.files.flatMap((f) => f.path.split("/").slice(0, -1).map((_, i, a) => a.slice(0, i + 1).join("/"))))];
        case "graph": return FX.graph ?? { nodes: [], edges: [] };
        case "get_graph_config": return FX.graphConfig ?? {};
        case "run_base": { const t = FX.bases?.[args.path]; if (!t) throw { code: "base", message: "no fixture for " + args.path }; return { ...t, view: args.view }; }
        case "plan_rename": return { from: args.from, to: args.to, affected: FX.affected ?? [] };
        case "search": return FX.search ?? { hits: [], associated: [] };
        case "related": return FX.related ?? { associated: [], similar: [], suggested: [] };
        case "semantic_edges": return FX.semanticEdges ?? [];
        case "embed_status": return FX.embed ?? { model: null, state: "off", pending: 0, error: null };
        case "record_event": case "typing": case "set_config": case "forget_memory": return null;
        case "plugin:event|listen": return ++cb;
        default: return null;
      }
    },
  };
})();`;

const summary = `JSON.stringify({
  panes: document.querySelectorAll(".pane").length,
  dividers: document.querySelectorAll(".divider").length,
  tabs: [...document.querySelectorAll(".pane")].map((p) => [...p.querySelectorAll(".tabs button")].map((b) => b.textContent.replace("×", "").trim())),
  modes: [...document.querySelectorAll(".pane")].map((p) => p.querySelector(".modes button.active")?.textContent.trim() ?? null),
  focusedTab: [...document.querySelectorAll(".pane.focused .tabs button.active")].map((b) => b.textContent.replace("×", "").trim()),
  views: [...document.querySelectorAll(".pane")].map((p) => p.querySelector(".cm-editor") ? "editor" : p.querySelector(".reading") ? "reading" : p.querySelector(".fileview") ? "file" : "empty"),
  toast: document.querySelector(".toast")?.textContent ?? null,
  extra: typeof window.__extra === "function" ? window.__extra() : null,
})`;

const profile = mkdtempSync(join(tmpdir(), "cdp-"));
const chrome = spawn(CHROME, [
  "--headless=new", `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`,
  "--no-first-run", "--no-default-browser-check", "--disable-gpu", `--window-size=${width},${height}`, "about:blank",
], { stdio: "ignore" });

let target;
for (let i = 0; i < 100 && !target; i++) {
  try {
    const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
    target = list.find((t) => t.type === "page");
  } catch { /* not up yet */ }
  if (!target) await wait(100);
}
if (!target) { console.error("chromium did not start"); chrome.kill(); process.exit(1); }

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let next = 0;
const pending = new Map();
const listeners = [];
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); return; }
  for (const l of listeners) l(msg);
});
const send = (method, params = {}) => new Promise((resolve) => { const id = ++next; pending.set(id, resolve); ws.send(JSON.stringify({ id, method, params })); });
listeners.push((m) => {
  if (m.method === "Runtime.exceptionThrown") console.log("EXCEPTION", m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
  if (m.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(m.params.type)) console.log("CONSOLE", m.params.type, m.params.args.map((a) => a.value ?? a.description).join(" "));
});

await send("Page.enable");
await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride", { width: Number(width), height: Number(height), deviceScaleFactor: 1, mobile: false });
// Headless pages are never focused otherwise, and editors react to focus.
await send("Emulation.setFocusEmulationEnabled", { enabled: true });
await send("Page.addScriptToEvaluateOnNewDocument", { source: mock });
const loaded = new Promise((r) => listeners.push((m) => m.method === "Page.loadEventFired" && r()));
await send("Page.navigate", { url: URL_ });
await loaded;
await wait(Number(process.env.SETTLE ?? 2500));
if (process.env.EVAL) console.log("EVAL", JSON.stringify((await send("Runtime.evaluate", { expression: process.env.EVAL, awaitPromise: true, returnByValue: true })).result?.result?.value));
if (process.env.EVAL) await wait(800);
const res = await send("Runtime.evaluate", { expression: summary, returnByValue: true });
console.log("SUMMARY", res.result?.result?.value ?? JSON.stringify(res.result));
const shot = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(out, Buffer.from(shot.result.data, "base64"));
console.log("SAVED", out);
ws.close();
chrome.kill();
process.exit(0);
