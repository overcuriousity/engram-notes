export type Mode = "live" | "source" | "reading";
export type Dir = "row" | "column";
export interface TabRef { path: string; mode: Mode }
export interface Pane { kind: "pane"; id: number; tabs: TabRef[]; active: number }
export interface Split { kind: "split"; id: number; dir: Dir; sizes: number[]; children: Node[] }
export type Node = Pane | Split;

export function panes(n: Node): Pane[] {
  return n.kind === "pane" ? [n] : n.children.flatMap(panes);
}

export function findPane(n: Node, id: number): Pane | undefined {
  return panes(n).find((p) => p.id === id);
}

export function findSplit(n: Node, id: number): Split | undefined {
  if (n.kind === "pane") return undefined;
  if (n.id === id) return n;
  for (const c of n.children) {
    const s = findSplit(c, id);
    if (s) return s;
  }
  return undefined;
}

export function maxId(n: Node): number {
  return n.kind === "pane" ? n.id : Math.max(n.id, ...n.children.map(maxId));
}

export function isNode(v: unknown): v is Node {
  const k = (v as Node | null)?.kind;
  return k === "pane" || k === "split";
}

/** Puts `fresh` after pane `paneId`: inside its split when that runs in `dir`, else in a new split. */
export function split(root: Node, paneId: number, dir: Dir, fresh: Pane, splitId: number): Node {
  const go = (n: Node): Node => {
    if (n.kind === "pane") {
      return n.id === paneId ? { kind: "split", id: splitId, dir, sizes: [0.5, 0.5], children: [n, fresh] } : n;
    }
    const i = n.children.findIndex((c) => c.kind === "pane" && c.id === paneId);
    if (i >= 0 && n.dir === dir) {
      const half = n.sizes[i] / 2;
      return {
        ...n,
        children: [...n.children.slice(0, i + 1), fresh, ...n.children.slice(i + 1)],
        sizes: [...n.sizes.slice(0, i), half, half, ...n.sizes.slice(i + 1)],
      };
    }
    return { ...n, children: n.children.map(go) };
  };
  return go(root);
}

/** Its space goes to the neighbour before it; a split left with one child becomes that child. */
export function removePane(root: Node, id: number): Node {
  const go = (n: Node): Node => {
    if (n.kind === "pane") return n;
    const i = n.children.findIndex((c) => c.kind === "pane" && c.id === id);
    if (i < 0) return { ...n, children: n.children.map(go) };
    const children = n.children.filter((_, k) => k !== i);
    const sizes = n.sizes.filter((_, k) => k !== i);
    sizes[Math.max(0, i - 1)] += n.sizes[i];
    return children.length === 1 ? children[0] : { ...n, children, sizes };
  };
  return go(root);
}

function mapPanes(root: Node, f: (p: Pane) => Pane): Node {
  return root.kind === "pane" ? f(root) : { ...root, children: root.children.map((c) => mapPanes(c, f)) };
}

// The tab that slides into a closed active tab's place becomes active.
function keepTabs(p: Pane, keep: (t: TabRef, i: number) => boolean): Pane {
  const tabs = p.tabs.filter(keep);
  const before = p.tabs.slice(0, Math.max(0, p.active)).filter(keep).length;
  return { ...p, tabs, active: tabs.length ? Math.min(before, tabs.length - 1) : -1 };
}

/** As in Obsidian, closing a pane's last tab closes the pane unless it is the only one. */
export function closeTab(root: Node, paneId: number, index: number): Node {
  const out = mapPanes(root, (p) => (p.id === paneId ? keepTabs(p, (_, i) => i !== index) : p));
  const p = findPane(out, paneId);
  return p && p.tabs.length === 0 && panes(out).length > 1 ? removePane(out, paneId) : out;
}

export function withoutPath(root: Node, path: string): Node {
  const emptied = panes(root)
    .filter((p) => p.tabs.length > 0 && p.tabs.every((t) => t.path === path))
    .map((p) => p.id);
  let out = mapPanes(root, (p) => keepTabs(p, (t) => t.path !== path));
  for (const id of emptied) if (panes(out).length > 1) out = removePane(out, id);
  return out;
}

export function renamePath(root: Node, from: string, to: string): Node {
  return mapPanes(root, (p) => ({ ...p, tabs: p.tabs.map((t) => (t.path === from ? { ...t, path: to } : t)) }));
}
