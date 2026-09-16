import { describe, expect, it } from "vitest";
import { displayText, findWikilinks, linkTarget } from "./wikilink";

describe("findWikilinks", () => {
  it("parses every form with positions", () => {
    const t = "a [[Note]] b ![[img.png]] c [[X#H|Y]]";
    const l = findWikilinks(t);
    expect(l.map((x) => [x.target, x.heading, x.alias, x.embed])).toEqual([
      ["Note", undefined, undefined, false],
      ["img.png", undefined, undefined, true],
      ["X", "H", "Y", false],
    ]);
    expect(t.slice(l[2].from, l[2].to)).toBe("[[X#H|Y]]");
  });
  it("chooses display text", () => {
    const [a, b, c] = findWikilinks("[[N]] [[N#H]] [[N|A]]");
    expect([displayText(a), displayText(b), displayText(c)]).toEqual(["N", "N › H", "A"]);
  });
});

describe("block links", () => {
  it("separates a block from a heading", () => {
    const [b, h] = findWikilinks("[[N#^b1]] [[N#H]]");
    expect([b.block, b.heading]).toEqual(["b1", undefined]);
    expect([h.block, h.heading]).toEqual([undefined, "H"]);
    expect([displayText(b), linkTarget(b), linkTarget(h)]).toEqual(["N", "N#^b1", "N#H"]);
  });
  it("never shows the id, with or without an alias", () => {
    const [plain, aliased] = findWikilinks("[[N#^ab12cd]] [[N#^ab12cd|the words]]");
    expect(displayText(plain)).toBe("N");
    expect(displayText(aliased)).toBe("the words");
  });
});
