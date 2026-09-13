import { describe, expect, it } from "vitest";
import { mergeEdges, weightBucket } from "./semantic";

const links = [{ source: "a.md", target: "b.md" }];
const semantic = [
  { source: "a.md", target: "c.md", weight: 0.9, kind: "similar" as const },
  { source: "a.md", target: "b.md", weight: 4, kind: "assoc" as const },
];

describe("mergeEdges", () => {
  it("returns links alone when semantic edges are off", () => {
    expect(mergeEdges(links, semantic, false)).toEqual([
      { source: "a.md", target: "b.md", kind: "link", weight: 1 },
    ]);
  });

  it("adds semantic edges and never doubles a pair a link already draws", () => {
    const out = mergeEdges(links, semantic, true);
    expect(out).toHaveLength(2);
    expect(out[0].kind).toBe("link");
    expect(out[1]).toEqual({ source: "a.md", target: "c.md", kind: "similar", weight: 0.9 });
  });

  it("reads a pair the same way round as the link that drew it", () => {
    const out = mergeEdges([{ source: "b.md", target: "a.md" }], semantic, true);
    expect(out.filter((e) => e.kind === "assoc")).toHaveLength(0);
  });
});

describe("weightBucket", () => {
  it("reads a cosine and an association strength on their own scales", () => {
    expect(weightBucket({ kind: "similar", weight: 0.9 })).toBe(2);
    expect(weightBucket({ kind: "similar", weight: 0.5 })).toBe(0);
    expect(weightBucket({ kind: "assoc", weight: 0.9 })).toBe(0);
    expect(weightBucket({ kind: "assoc", weight: 8 })).toBe(2);
  });
});
