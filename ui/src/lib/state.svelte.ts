import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import * as L from "./layout";
import { fileKind } from "./files";
import type { Dir, Mode, Node, Pane, TabRef } from "./layout";

export interface Doc {
  path: string;
  text: string;
  savedText: string;
  mtime_ms: number;
  conflict: boolean;
}

class AppStateStore {
  root = $state<string | null>(null);
  config = $state<api.AppConfig | null>(null);
  files = $state<api.FileEntry[]>([]);
  // One buffer per open path, shared by every tab showing it.
  docs = $state<Record<string, Doc>>({});
  layout = $state<Node>({ kind: "pane", id: 1, tabs: [], active: -1 });
  activePane = $state(1);
  toast = $state<string | null>(null);
  titles = $state<[string, string][]>([]);
  showLeft = $state(true);
  showRight = $state(true);
  palette = $state<"none" | "files" | "commands">("none");
  leftPane = $state<"files" | "search">("files");
  watching = $state(true);
  private nextId = 2;

  get pane(): Pane {
    return L.findPane(this.layout, this.activePane) ?? L.panes(this.layout)[0];
  }

  get paneCount(): number {
    return L.panes(this.layout).length;
  }

  get activeTab(): TabRef | null {
    const p = this.pane;
    return p.active >= 0 ? p.tabs[p.active] : null;
  }

  get activeDoc(): Doc | null {
    const t = this.activeTab;
    return t ? (this.docs[t.path] ?? null) : null;
  }

  async open(root: string) {
    const info = await api.openVault(root);
    this.root = info.root;
    this.config = info.config;
    if (info.index_recreated) this.say("The index was damaged and has been rebuilt.");
    this.watching = info.watch_error === null;
    if (info.watch_error) this.say(`File watching is off: ${info.watch_error}. Changes are read when the window gains focus.`);
    await this.refresh();
    await this.restore(await api.getWorkspace());
    await api.onIndexChanged(() => this.refresh());
    await api.onFileChanged((c) => this.externalChange(c));
    await api.onWatchFailed((msg) => {
      if (this.watching) this.say(`File watching stopped: ${msg}. Changes are read when the window gains focus.`);
      this.watching = false;
    });
    await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && !this.watching) this.rescan().catch((e) => this.say(api.errorMessage(e)));
    });
  }

  // The foundation's `{tabs, active}` still opens, as one pane.
  private async restore(ws: Record<string, unknown>) {
    const mode = this.config?.editor.default_mode ?? "live";
    let layout: Node = L.isNode(ws.layout)
      ? ws.layout
      : {
          kind: "pane",
          id: 1,
          tabs: ((ws.tabs as string[] | undefined) ?? []).map((path) => ({ path, mode })),
          active: Number(ws.active ?? 0),
        };
    for (const path of new Set(L.panes(layout).flatMap((p) => p.tabs.map((t) => t.path)))) {
      try {
        if (fileKind(path) === "note") await this.load(path);
      } catch {
        layout = L.withoutPath(layout, path); // gone since last session
      }
    }
    for (const p of L.panes(layout)) p.active = Math.min(Math.max(p.active, 0), p.tabs.length - 1);
    this.layout = layout;
    this.nextId = L.maxId(layout) + 1;
    const wanted = Number(ws.activePane);
    this.activePane = L.findPane(layout, wanted) ? wanted : L.panes(layout)[0].id;
  }

  async refresh() {
    this.files = await api.listFiles();
    this.titles = await api.titles();
  }

  private async load(path: string) {
    if (this.docs[path]) return;
    const n = await api.readNote(path);
    this.docs[path] = { path, text: n.text, savedText: n.text, mtime_ms: n.mtime_ms, conflict: false };
  }

  /** Opens `path` in the active pane, or activates its tab there. */
  async openNote(path: string) {
    const p = this.pane;
    const i = p.tabs.findIndex((t) => t.path === path);
    if (i >= 0) return this.activate(p.id, i);
    if (fileKind(path) === "note") await this.load(path);
    p.tabs.push({ path, mode: this.config?.editor.default_mode ?? "live" });
    p.active = p.tabs.length - 1;
    this.persist();
  }

  activate(paneId: number, index: number) {
    const p = L.findPane(this.layout, paneId);
    if (!p) return;
    p.active = index;
    this.activePane = paneId;
    this.persist();
  }

  focusPane(paneId: number) {
    if (this.activePane === paneId) return;
    this.activePane = paneId;
    this.persist();
  }

  setMode(paneId: number, path: string, mode: Mode) {
    const t = L.findPane(this.layout, paneId)?.tabs.find((x) => x.path === path);
    if (!t) return;
    t.mode = mode;
    this.persist();
  }

  closeTab(paneId: number, index: number) {
    const path = L.findPane(this.layout, paneId)?.tabs[index]?.path;
    this.layout = L.closeTab(this.layout, paneId, index);
    this.afterLayoutChange();
    if (path) void this.release(path);
  }

  /** Obsidian's split right and split down: the current note opens again beside itself. */
  split(dir: Dir) {
    const t = this.activeTab;
    const fresh: Pane = { kind: "pane", id: this.nextId++, tabs: t ? [{ ...t }] : [], active: t ? 0 : -1 };
    this.layout = L.split(this.layout, this.pane.id, dir, fresh, this.nextId++);
    this.activePane = fresh.id;
    this.persist();
  }

  resize(splitId: number, sizes: number[]) {
    const s = L.findSplit(this.layout, splitId);
    if (s) s.sizes = sizes;
  }

  renamed(from: string, to: string) {
    const d = this.docs[from];
    if (d) {
      delete this.docs[from];
      d.path = to;
      this.docs[to] = d;
    }
    this.layout = L.renamePath(this.layout, from, to);
    this.persist();
  }

  forget(path: string) {
    this.layout = L.withoutPath(this.layout, path);
    delete this.docs[path];
    this.afterLayoutChange();
  }

  private afterLayoutChange() {
    if (!L.findPane(this.layout, this.activePane)) this.activePane = L.panes(this.layout)[0].id;
    this.persist();
  }

  private shown(path: string): boolean {
    return L.panes(this.layout).some((p) => p.tabs.some((t) => t.path === path));
  }

  // A buffer no tab shows any more is saved, then dropped.
  private async release(path: string) {
    const d = this.docs[path];
    if (!d || this.shown(path)) return;
    await this.save(d);
    if (!this.shown(path)) delete this.docs[path];
  }

  async save(doc: Doc) {
    if (doc.text === doc.savedText) return;
    try {
      doc.mtime_ms = await api.writeNote(doc.path, doc.text);
      doc.savedText = doc.text;
    } catch (e) {
      this.say(api.errorMessage(e));
    }
  }

  async externalChange(c: api.Change) {
    const doc = this.docs[c.path];
    if (!doc) return;
    if (c.kind === "removed") {
      doc.conflict = doc.text !== doc.savedText;
      return;
    }
    const n = await api.readNote(c.path);
    if (n.text === doc.savedText) return;
    if (doc.text === doc.savedText) {
      doc.text = n.text;
      doc.savedText = n.text;
      doc.mtime_ms = n.mtime_ms;
    } else {
      doc.conflict = true;
    }
  }

  async resolveConflict(doc: Doc, keepMine: boolean) {
    if (keepMine) {
      doc.savedText = "";
      await this.save(doc);
    } else {
      const n = await api.readNote(doc.path);
      doc.text = n.text;
      doc.savedText = n.text;
      doc.mtime_ms = n.mtime_ms;
    }
    doc.conflict = false;
  }

  // Without a watcher, gaining focus is when outside edits are picked up.
  async rescan() {
    await api.rescan();
    await this.refresh();
    for (const path of Object.keys(this.docs)) {
      try {
        await this.externalChange({ path, kind: "changed" });
      } catch {
        await this.externalChange({ path, kind: "removed" });
      }
    }
  }

  persist() {
    if (!this.root) return;
    void api.setWorkspace({ layout: $state.snapshot(this.layout), activePane: this.activePane });
  }

  say(msg: string) {
    this.toast = msg;
    setTimeout(() => (this.toast = null), 4000);
  }
}

export const app = new AppStateStore();
