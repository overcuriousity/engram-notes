import { describe, expect, it } from "vitest";
import type { Graph } from "./api";
import { DEFAULTS, filterGraph, labelAlpha, parseSearch, readSettings, searchWords, type GraphSettings, type ViewGraph } from "./graph";

const g: Graph = {
  nodes: [
    { id: "A.md", title: "A", kind: "note", tags: ["proj"] },
    { id: "B.md", title: "B", kind: "note", tags: ["proj/sub"] },
    { id: "C.md", title: "C", kind: "note", tags: [] },
    { id: "Lonely.md", title: "Lonely", kind: "note", tags: [] },
    { id: "Ghost", title: "Ghost", kind: "unresolved", tags: [] },
    { id: "img/p.png", title: "p.png", kind: "attachment", tags: [] },
  ],
  edges: [
    { source: "A.md", target: "B.md" },
    { source: "B.md", target: "C.md" },
    { source: "A.md", target: "Ghost" },
    { source: "C.md", target: "img/p.png" },
  ],
};
const ids = (v: ViewGraph) => v.nodes.map((n) => n.id);
const s = (o: Partial<GraphSettings> = {}): GraphSettings => ({ ...DEFAULTS, ...o });

describe("graph filters", () => {
  it("hides attachments by default; unresolved and orphans on request", () => {
    expect(ids(filterGraph(g, s(), null))).toEqual(["A.md", "B.md", "C.md", "Lonely.md", "Ghost"]);
    expect(ids(filterGraph(g, s({ showAttachments: true, hideUnresolved: true, showOrphans: false }), null))).toEqual(["A.md", "B.md", "C.md", "img/p.png"]);
  });

  it("drops edges to hidden nodes and counts inbound links", () => {
    const v = filterGraph(g, s(), null);
    expect(v.edges).toHaveLength(3);
    expect(v.nodes.find((n) => n.id === "B.md")!.inbound).toBe(1);
    expect(v.nodes.find((n) => n.id === "A.md")!.inbound).toBe(0);
  });

  it("parses Obsidian's search operators", () => {
    expect(parseSearch('tag:#proj path:"my dir" file:x hello')).toEqual({ words: ["hello"], tags: ["proj"], paths: ["my dir"], files: ["x"] });
    expect(searchWords('tag:#proj hello "big world"')).toBe('hello "big world"');
  });

  it("filters by tag including nested tags, by words, and by full-text hits", () => {
    expect(ids(filterGraph(g, s({ search: "tag:proj" }), null))).toEqual(["A.md", "B.md"]);
    expect(ids(filterGraph(g, s({ search: "ghost" }), null))).toEqual(["Ghost"]);
    expect(ids(filterGraph(g, s({ search: "needle" }), new Set(["C.md"])))).toEqual(["C.md"]);
    expect(ids(filterGraph(g, s({ search: "path:img file:p.png", showAttachments: true }), null))).toEqual(["img/p.png"]);
  });

  it("adds tag nodes linked to their notes", () => {
    const v = filterGraph(g, s({ showTags: true }), null);
    expect(ids(v)).toContain("#proj");
    expect(ids(v)).toContain("#proj/sub");
    expect(v.edges).toContainEqual({ source: "A.md", target: "#proj", kind: "link", weight: 1 });
  });

  it("draws semantic edges only where the toggle for that graph says so", () => {
    const sem = [{ source: "A.md", target: "B.md", weight: 0.8, kind: "similar" as const }];
    const bare = { nodes: g.nodes, edges: [] };
    expect(filterGraph(bare, s({}), null, null, sem).edges).toHaveLength(0);
    const localEdges = filterGraph(bare, s({}), null, "A.md", sem).edges;
    expect(localEdges).toHaveLength(1);
    expect(localEdges[0].kind).toBe("similar");
  });

  it("keeps a local graph's neighbourhood to the chosen depth", () => {
    expect(ids(filterGraph(g, s({ localDepth: 1 }), null, "A.md"))).toEqual(["A.md", "B.md", "Ghost"]);
    expect(ids(filterGraph(g, s({ localDepth: 2 }), null, "A.md"))).toEqual(["A.md", "B.md", "C.md", "Ghost"]);
  });

  it("reads graph.json over the defaults, ignoring wrong types", () => {
    const r = readSettings({ showOrphans: false, repelStrength: "x", other: 1 });
    expect(r.showOrphans).toBe(false);
    expect(r.repelStrength).toBe(10);
  });

  it("fades labels in as the view zooms", () => {
    expect(labelAlpha(0.5, 0)).toBe(0);
    expect(labelAlpha(2, 0)).toBe(1);
    expect(labelAlpha(1, 3)).toBe(1);
  });
});
