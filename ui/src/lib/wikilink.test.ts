import { describe, expect, it } from "vitest";
import { displayText, findWikilinks } from "./wikilink";

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
