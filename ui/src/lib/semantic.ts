import type { GraphEdge, SemanticEdge } from "./api";

export interface DrawEdge {
  source: string;
  target: string;
  kind: "link" | "assoc" | "similar";
  weight: number;
}

const key = (a: string, b: string) => (a < b ? `${a} ${b}` : `${b} ${a}`);

/** Explicit links, then the dashed ones, with no pair drawn twice. */
export function mergeEdges(links: GraphEdge[], semantic: SemanticEdge[], on: boolean): DrawEdge[] {
  const out: DrawEdge[] = links.map((e) => ({ source: e.source, target: e.target, kind: "link", weight: 1 }));
  if (!on) return out;
  const seen = new Set(out.map((e) => key(e.source, e.target)));
  for (const e of semantic) {
    const k = key(e.source, e.target);
    if (seen.has(k)) continue;
    seen.add(k);
    out.push({ source: e.source, target: e.target, kind: e.kind, weight: e.weight });
  }
  return out;
}

/**
 * Three widths for a dashed edge: a stronger tie draws thicker. A cosine and an
 * association strength are different scales, so each is read on its own.
 */
export function weightBucket(e: { kind: DrawEdge["kind"]; weight: number }): 0 | 1 | 2 {
  if (e.kind === "assoc") return e.weight >= 6 ? 2 : e.weight >= 3 ? 1 : 0;
  return e.weight >= 0.85 ? 2 : e.weight >= 0.7 ? 1 : 0;
}
