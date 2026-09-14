import { app } from "./state.svelte";
import { open } from "@tauri-apps/plugin-dialog";
import { dailyNote, createNote, errorMessage, forgetMemory, importLogseq as importLogseqCmd, setConfig, setModelDir, templates } from "./api";
import { freeName } from "./tree";
import { fileKind } from "./files";

export interface Command { id: string; name: string; hotkey: string; run: () => void | Promise<void> }

// What Obsidian writes into a new base.
export const NEW_BASE = "views:\n  - type: table\n    name: Table\n";

export async function newNote(dir = "") {
  const n = freeName(app.files.map((f) => f.path), dir, "Untitled", ".md");
  await createNote(n);
  await app.refresh();
  await app.openNote(n);
}

export async function newBase(dir = "") {
  const n = freeName(app.files.map((f) => f.path), dir, "Untitled", ".base");
  await createNote(n, NEW_BASE);
  await app.refresh();
  await app.openNote(n);
}

/** Where an insertion can go: the active tab, when it is a note in an editing mode. */
export function templateTarget(): { pane: number; path: string } | null {
  const t = app.activeTab;
  if (!t || fileKind(t.path) !== "note" || t.mode === "reading") return null;
  return { pane: app.activePane, path: t.path };
}

// Obsidian's picker over the templates folder; the folder is read afresh each time.
export async function insertTemplate() {
  if (!templateTarget()) return app.say("Open a note in an editing mode to insert a template.");
  app.templates = await templates();
  if (!app.templates.length) return app.say(`No templates in ${app.config?.templates.folder ?? "the templates folder"}.`);
  app.palette = "templates";
}

/** Two folders, then the report opens. Journals land in the vault's daily folder, where Ctrl+D looks. */
export async function importLogseq() {
  const source = await open({ directory: true, title: "Choose the Logseq graph folder" });
  if (typeof source !== "string") return;
  const dest = await open({ directory: true, defaultPath: app.root ?? undefined, title: "Choose the folder in this vault to import into" });
  if (typeof dest !== "string") return;
  try {
    const s = await importLogseqCmd(source, dest);
    await app.refresh();
    await app.openNote(s.report);
    const rest = s.unmapped ? `${s.unmapped} things to look at in the report.` : "Everything was mapped.";
    app.say(`Imported ${s.pages} pages and ${s.journals} journals. ${rest}`);
  } catch (e) {
    // An import stops on the first write it cannot do, so what landed before
    // it and the report that names it are already in the vault.
    await app.refresh();
    const report = reportPath(dest);
    if (app.files.some((f) => f.path === report)) await app.openNote(report);
    app.say(errorMessage(e));
  }
}

/** Where a failed import left its report, vault-relative: there is no summary to read it from. */
function reportPath(dest: string): string {
  const root = app.root;
  if (!root || !dest.startsWith(root)) return "";
  const rel = dest.slice(root.length).replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  return rel ? `${rel}/import-report.md` : "import-report.md";
}

export const defaults: Command[] = [
  { id: "palette", name: "Open command palette", hotkey: "Ctrl+P", run: () => (app.palette = "commands") },
  { id: "switcher", name: "Quick switcher", hotkey: "Ctrl+O", run: () => (app.palette = "files") },
  { id: "search", name: "Search in all files", hotkey: "Ctrl+K", run: () => (app.palette = "search") },
  { id: "new-note", name: "New note", hotkey: "Ctrl+N", run: () => newNote() },
  { id: "new-base", name: "Create new base", hotkey: "", run: () => newBase() },
  { id: "graph", name: "Open graph view", hotkey: "Ctrl+G", run: () => app.openNote("graph:global") },
  { id: "local-graph", name: "Open local graph", hotkey: "", run: () => app.openNote("graph:local") },
  { id: "daily", name: "Open today's daily note", hotkey: "Ctrl+D", run: async () => { const p = await dailyNote(); await app.refresh(); await app.openNote(p); } },
  { id: "insert-template", name: "Templates: Insert template", hotkey: "", run: () => insertTemplate() },
  { id: "import-logseq", name: "Import: Logseq graph", hotkey: "", run: () => importLogseq() },
  { id: "close-tab", name: "Close current tab", hotkey: "Ctrl+W", run: () => { const p = app.pane; if (p.active >= 0) app.closeTab(p.id, p.active); } },
  { id: "toggle-mode", name: "Toggle live preview / source", hotkey: "Ctrl+E", run: () => { const t = app.activeTab; if (t) app.setMode(app.pane.id, t.path, t.mode === "source" ? "live" : "source"); } },
  { id: "toggle-reading", name: "Toggle reading view", hotkey: "Ctrl+Shift+E", run: () => { const t = app.activeTab; if (t) app.setMode(app.pane.id, t.path, t.mode === "reading" ? "live" : "reading"); } },
  { id: "split-right", name: "Split right", hotkey: "", run: () => app.split("row") },
  { id: "split-down", name: "Split down", hotkey: "", run: () => app.split("column") },
  { id: "toggle-left", name: "Toggle left sidebar", hotkey: "Ctrl+Shift+L", run: () => (app.showLeft = !app.showLeft) },
  { id: "toggle-right", name: "Toggle right sidebar", hotkey: "Ctrl+Shift+R", run: () => (app.showRight = !app.showRight) },
  { id: "save", name: "Save", hotkey: "Ctrl+S", run: () => { const d = app.activeDoc; if (d) return app.save(d); } },
  { id: "settings", name: "Open settings", hotkey: "Ctrl+,", run: () => (app.settings = true) },
  {
    id: "memory-toggle",
    name: "Memory: turn on or off",
    hotkey: "",
    run: async () => {
      const cfg = app.config;
      if (!cfg) return;
      cfg.memory.enabled = !cfg.memory.enabled;
      await setConfig($state.snapshot(cfg));
      app.say(cfg.memory.enabled ? "Memory is on." : "Memory is off; what it learned is kept.");
    },
  },
  {
    id: "memory-forget",
    name: "Memory: forget everything learned",
    hotkey: "",
    run: async () => {
      await forgetMemory();
      app.say("Memory forgotten.");
    },
  },
  {
    id: "model-dir",
    name: "Embedding: choose the model folder",
    hotkey: "",
    run: async () => {
      const dir = await open({ directory: true });
      if (typeof dir !== "string") return;
      await setModelDir(dir);
      app.say("Loading the model from that folder.");
    },
  },
];

export function allCommands(): Command[] {
  const over = app.config?.hotkeys ?? {};
  return defaults.map((c) => ({ ...c, hotkey: over[c.id] ?? c.hotkey }));
}

export function chord(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return parts.join("+");
}
