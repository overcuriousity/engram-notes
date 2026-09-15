import type { Graph, GraphNode, SemanticEdge } from "./api";
import { mergeEdges, type DrawEdge } from "./semantic";

/** One of Obsidian's colour groups: nodes the query matches take the colour. `rgb` is Obsidian's packed 0xRRGGBB. */
export interface ColorGroup { query: string; color: { a: number; rgb: number } }

/** Obsidian's graph.json keys, plus the local graph's depth. */
export interface GraphSettings {
  search: string;
  colorGroups: ColorGroup[];
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
  colorGroups: [],
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
  for (const [k, v] of Object.entries(DEFAULTS)) if (!Array.isArray(v) && typeof raw[k] === typeof v) out[k] = raw[k];
  out.colorGroups = readColorGroups(raw.colorGroups);
  return out as unknown as GraphSettings;
}

/** Obsidian's `colorGroups`, with anything malformed dropped rather than drawn. */
export function readColorGroups(raw: unknown): ColorGroup[] {
  if (!Array.isArray(raw)) return [];
  const out: ColorGroup[] = [];
  for (const g of raw) {
    if (!g || typeof g !== "object") continue;
    const { query, color } = g as { query?: unknown; color?: { a?: unknown; rgb?: unknown } };
    if (typeof query !== "string" || !color || typeof color !== "object" || typeof color.rgb !== "number") continue;
    out.push({ query, color: { a: typeof color.a === "number" ? color.a : 1, rgb: color.rgb & 0xffffff } });
  }
  return out;
}

/** Obsidian's packed rgb as CSS. */
export const hexColor = (rgb: number): string => `#${(rgb & 0xffffff).toString(16).padStart(6, "0")}`;

/** A colour input's `#rrggbb` as Obsidian's packed rgb. */
export const rgbInt = (hex: string): number => parseInt(hex.replace(/^#/, ""), 16) || 0;

// Obsidian gives a new group a random colour; ours cycle, so a second group is never the first's twin.
export const GROUP_COLORS = [0xe06c75, 0x61afef, 0x98c379, 0xe5c07b, 0xc678dd, 0x56b6c2, 0xd19a66, 0x7f8c8d];

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

export interface ViewNode { id: string; title: string; kind: GraphNode["kind"] | "tag"; tags: string[]; inbound: number; color: string | null }
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

/** The first group whose query matches, as Obsidian's higher group wins. Words match the title and path only. */
export function groupColor(n: GraphNode, groups: ColorGroup[]): string | null {
  for (const g of groups) {
    const q = parseSearch(g.query);
    if (Object.values(q).every((terms) => terms.length === 0)) continue;
    if (matches(n, q, null)) return hexColor(g.color.rgb);
  }
  return null;
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
    .map((n) => ({ ...n, inbound: 0, color: groupColor(n, s.colorGroups) }));
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
    for (const t of [...tags].sort()) nodes.push({ id: `#${t}`, title: `#${t}`, kind: "tag", tags: [], inbound: 0, color: null });
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

export interface View { x: number; y: number; k: number }

/** One frame of the slide toward `target`; `done` once the rest is too small to see. */
export function easeStep(view: View, target: View): { view: View; done: boolean } {
  const d = { x: target.x - view.x, y: target.y - view.y, k: target.k - view.k };
  if (Math.abs(d.k) < 1e-4 && Math.hypot(d.x, d.y) < 0.5) return { view: { ...target }, done: true };
  return { view: { x: view.x + d.x * 0.28, y: view.y + d.y * 0.28, k: view.k + d.k * 0.28 }, done: false };
}
