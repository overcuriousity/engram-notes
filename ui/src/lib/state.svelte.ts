import * as api from "./api";

export interface Tab {
  path: string;
  text: string;
  savedText: string;
  mtime_ms: number;
  mode: "live" | "source" | "reading";
  conflict: boolean;
}

class AppStateStore {
  root = $state<string | null>(null);
  config = $state<api.AppConfig | null>(null);
  files = $state<api.FileEntry[]>([]);
  tabs = $state<Tab[]>([]);
  active = $state<number>(-1);
  toast = $state<string | null>(null);
  titles = $state<[string, string][]>([]);
  showLeft = $state(true);
  showRight = $state(true);
  palette = $state<"none" | "files" | "commands">("none");
  leftPane = $state<"files" | "search">("files");

  get activeTab(): Tab | null {
    return this.active >= 0 ? this.tabs[this.active] : null;
  }

  async open(root: string) {
    const info = await api.openVault(root);
    this.root = info.root;
    this.config = info.config;
    await this.refresh();
    const ws = await api.getWorkspace();
    const open = (ws.tabs as string[] | undefined) ?? [];
    for (const p of open) {
      try { await this.openNote(p, false, false); } catch { /* the note went away since last session */ }
    }
    this.active = this.tabs.length ? Math.max(0, Math.min(Number(ws.active ?? 0), this.tabs.length - 1)) : -1;
    await api.onIndexChanged(() => this.refresh());
    await api.onFileChanged((c) => this.externalChange(c));
  }

  async refresh() {
    this.files = await api.listFiles();
    this.titles = await api.titles();
  }

  async openNote(path: string, activate = true, persist = true) {
    const i = this.tabs.findIndex((t) => t.path === path);
    if (i >= 0) {
      if (activate) {
        this.active = i;
        this.persist();
      }
      return;
    }
    const n = await api.readNote(path);
    this.tabs.push({
      path,
      text: n.text,
      savedText: n.text,
      mtime_ms: n.mtime_ms,
      mode: this.config?.editor.default_mode ?? "live",
      conflict: false,
    });
    if (activate) this.active = this.tabs.length - 1;
    if (persist) this.persist();
  }

  closeTab(i: number) {
    this.tabs.splice(i, 1);
    if (this.active >= this.tabs.length) this.active = this.tabs.length - 1;
    this.persist();
  }

  async save(tab: Tab) {
    if (tab.text === tab.savedText) return;
    try {
      tab.mtime_ms = await api.writeNote(tab.path, tab.text);
      tab.savedText = tab.text;
    } catch (e) {
      this.say(api.errorMessage(e));
    }
  }

  async externalChange(c: api.Change) {
    const tab = this.tabs.find((t) => t.path === c.path);
    if (!tab) return;
    if (c.kind === "removed") {
      tab.conflict = tab.text !== tab.savedText;
      return;
    }
    const n = await api.readNote(c.path);
    if (n.text === tab.savedText) return;
    if (tab.text === tab.savedText) {
      tab.text = n.text;
      tab.savedText = n.text;
      tab.mtime_ms = n.mtime_ms;
    } else {
      tab.conflict = true;
    }
  }

  async resolveConflict(tab: Tab, keepMine: boolean) {
    if (keepMine) {
      tab.savedText = "";
      await this.save(tab);
    } else {
      const n = await api.readNote(tab.path);
      tab.text = n.text;
      tab.savedText = n.text;
      tab.mtime_ms = n.mtime_ms;
    }
    tab.conflict = false;
  }

  persist() {
    if (!this.root) return;
    void api.setWorkspace({ tabs: this.tabs.map((t) => t.path), active: this.active });
  }

  say(msg: string) {
    this.toast = msg;
    setTimeout(() => (this.toast = null), 4000);
  }
}

export const app = new AppStateStore();
