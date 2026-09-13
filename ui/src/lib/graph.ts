import type { Graph, GraphNode, SemanticEdge } from "./api";
import { mergeEdges, type DrawEdge } from "./semantic";

/** Obsidian's graph.json keys, plus the local graph's depth. */
export interface GraphSettings {
  search: string;
  showTags: boolean;
  showAttachments: boolean;
  hideUnresolved: boolean;
  showOrphans: boolean;
  showArrow: boolean;
  // Dashed associations and near passages: off globally, on in a local graph.
  showSemantic: boolean;
  showSemanticLocal: boolean;
  textFadeMultiplier: number;
  nodeSizeMultiplier: number;
  lineSizeMultiplier: number;
  centerStrength: number;
  repelStrength: number;
  linkStrength: number;
  linkDistance: number;
  localDepth: number;
}

// Obsidian's defaults.
export const DEFAULTS: GraphSettings = {
  search: "",
  showTags: false,
  showAttachments: false,
  hideUnresolved: false,
  showOrphans: true,
  showArrow: false,
  showSemantic: false,
  showSemanticLocal: true,
  textFadeMultiplier: 0,
  nodeSizeMultiplier: 1,
  lineSizeMultiplier: 1,
  centerStrength: 0.518713248970312,
  repelStrength: 10,
  linkStrength: 1,
  linkDistance: 250,
  localDepth: 1,
};

export function readSettings(raw: Record<string, unknown>): GraphSettings {
  const out: Record<string, unknown> = { ...DEFAULTS };
  for (const [k, v] of Object.entries(DEFAULTS)) if (typeof raw[k] === typeof v) out[k] = raw[k];
  return out as unknown as GraphSettings;
}

export interface Search { words: string[]; tags: string[]; paths: string[]; files: string[] }

const TERM = /(\w+:)?(?:"([^"]*)"|(\S+))/g;

/** The subset of Obsidian's search operators the graph filter reads: `tag:`, `path:`, `file:` and plain words. */
export function parseSearch(q: string): Search {
  const s: Search = { words: [], tags: [], paths: [], files: [] };
  for (const m of q.matchAll(TERM)) {
    const value = (m[2] ?? m[3] ?? "").toLowerCase();
    if (!value) continue;
    if (m[1] === "tag:") s.tags.push(value.replace(/^#/, ""));
    else if (m[1] === "path:") s.paths.push(value);
    else if (m[1] === "file:") s.files.push(value);
    else s.words.push(m[1] ? m[0].toLowerCase() : value);
  }
  return s;
}

/** The plain words of a search, as the full-text search takes them. */
export function searchWords(q: string): string {
  return [...q.matchAll(TERM)].filter((m) => !["tag:", "path:", "file:"].includes(m[1] ?? "")).map((m) => m[0]).join(" ");
}

export interface ViewNode { id: string; title: string; kind: GraphNode["kind"] | "tag"; tags: string[]; inbound: number }
export interface ViewGraph { nodes: ViewNode[]; edges: DrawEdge[] }

function matches(n: GraphNode, s: Search, content: Set<string> | null): boolean {
  const path = n.id.toLowerCase();
  const name = path.slice(path.lastIndexOf("/") + 1);
  const title = n.title.toLowerCase();
  if (s.tags.some((t) => !n.tags.some((x) => x === t || x.startsWith(`${t}/`)))) return false;
  if (s.paths.some((p) => !path.includes(p))) return false;
  if (s.files.some((f) => !name.includes(f))) return false;
  if (s.words.length === 0 || content?.has(n.id)) return true;
  return s.words.every((w) => title.includes(w) || path.includes(w));
}

/** Nodes within `depth` links of `center`, following links either way. */
export function neighbourhood(edges: DrawEdge[], center: string, depth: number): Set<string> {
  const adj = new Map<string, string[]>();
  const add = (a: string, b: string) => adj.set(a, [...(adj.get(a) ?? []), b]);
  for (const e of edges) {
    add(e.source, e.target);
    add(e.target, e.source);
  }
  const seen = new Set([center]);
  let frontier = [center];
  for (let d = 0; d < depth; d++) {
    const next: string[] = [];
    for (const id of frontier) {
      for (const m of adj.get(id) ?? []) {
        if (!seen.has(m)) {
          seen.add(m);
          next.push(m);
        }
      }
    }
    frontier = next;
  }
  return seen;
}

/** What the graph draws. `content` holds the notes full-text search found for the plain words. */
export function filterGraph(
  g: Graph,
  s: GraphSettings,
  content: Set<string> | null,
  center?: string | null,
  semantic: SemanticEdge[] = [],
): ViewGraph {
  const search = parseSearch(s.search);
  const searching = Object.values(search).some((terms) => terms.length > 0);
  let nodes: ViewNode[] = g.nodes
    .filter((n) => (n.kind !== "attachment" || s.showAttachments) && (n.kind !== "unresolved" || !s.hideUnresolved))
    .filter((n) => !searching || matches(n, search, content))
    .map((n) => ({ ...n, inbound: 0 }));
  const kept = new Set(nodes.map((n) => n.id));
  const drawn = mergeEdges(g.edges, semantic, center ? s.showSemanticLocal : s.showSemantic);
  let edges = drawn.filter((e) => kept.has(e.source) && kept.has(e.target));
  if (s.showTags) {
    const tags = new Set<string>();
    for (const n of nodes) {
      for (const t of n.tags) {
        tags.add(t);
        edges.push({ source: n.id, target: `#${t}`, kind: "link", weight: 1 });
      }
    }
    for (const t of [...tags].sort()) nodes.push({ id: `#${t}`, title: `#${t}`, kind: "tag", tags: [], inbound: 0 });
  }
  if (center) {
    const near = neighbourhood(edges, center, s.localDepth);
    nodes = nodes.filter((n) => near.has(n.id));
    edges = edges.filter((e) => near.has(e.source) && near.has(e.target));
  }
  if (!s.showOrphans) {
    const linked = new Set(edges.flatMap((e) => [e.source, e.target]));
    nodes = nodes.filter((n) => linked.has(n.id) || n.id === center);
  }
  const inbound = new Map<string, number>();
  for (const e of edges) inbound.set(e.target, (inbound.get(e.target) ?? 0) + 1);
  for (const n of nodes) n.inbound = inbound.get(n.id) ?? 0;
  return { nodes, edges };
}

/** Obsidian's nodes are small and grow slowly; a hub is bigger, not enormous. */
export function radius(n: ViewNode, s: GraphSettings): number {
  return (2.5 + Math.sqrt(n.inbound) * 1.5) * s.nodeSizeMultiplier;
}

/** Obsidian's slider values in d3-force units, scaled so the defaults space a vault comfortably. */
export function forces(s: GraphSettings) {
  return { center: s.centerStrength * 0.1, charge: -s.repelStrength * 20, link: s.linkStrength, distance: s.linkDistance / 5 };
}

/** Labels stay off until the view is zoomed in, as Obsidian's do. */
export function labelAlpha(scale: number, fade: number): number {
  return Math.min(1, Math.max(0, (scale - 1.1 + fade * 0.05) * 4));
}

/** How near a pointer must come to a node, in world units: its own radius, or 10 screen pixels. */
export function hitRadius(r: number, k: number): number {
  return Math.max(r + 4 / k, 10 / k);
}

/** The view that shows every point with a margin, centred; the identity view when there are none. */
export function fitView(
  points: { x: number; y: number; r: number }[],
  w: number,
  h: number,
): { x: number; y: number; k: number } {
  if (points.length === 0) return { x: w / 2, y: h / 2, k: 1 };
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of points) {
    x0 = Math.min(x0, p.x - p.r);
    y0 = Math.min(y0, p.y - p.r);
    x1 = Math.max(x1, p.x + p.r);
    y1 = Math.max(y1, p.y + p.r);
  }
  // Room for the labels under the lowest nodes, and a little air all round.
  const pad = 48;
  const k = Math.min(2, Math.max(0.05, Math.min((w - pad * 2) / Math.max(1, x1 - x0), (h - pad * 2) / Math.max(1, y1 - y0))));
  return { x: w / 2 - ((x0 + x1) / 2) * k, y: h / 2 - ((y0 + y1) / 2) * k, k };
}
