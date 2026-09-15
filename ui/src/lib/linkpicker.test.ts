import { describe, expect, it } from "vitest";
import { blockTrigger, firstLine, linkText, stemOf } from "./linkpicker";
import type { Block } from "./api";

const para: Block = { kind: "paragraph", first: 3, last: 4, text: "para one\nstill one", heading: null };
const head: Block = { kind: "heading", first: 1, last: 1, text: "Title", heading: "Title" };

describe("linkText", () => {
  it("writes a block link with the alias", () => {
    expect(linkText("a/Note.md", para, "ab12cd", "my words")).toBe("[[Note#^ab12cd|my words]]");
  });
  it("writes a heading link without an anchor", () => {
    expect(linkText("Note.md", head, null, null)).toBe("[[Note#Title]]");
    expect(linkText("Note.md", head, null, "w")).toBe("[[Note#Title|w]]");
  });
  it("stems a path", () => {
    expect(stemOf("x/y/Z.md")).toBe("Z");
    expect(stemOf("Z.md")).toBe("Z");
  });
});

describe("blockTrigger", () => {
  it("recognises [[^^ and what follows", () => {
    expect(blockTrigger("text [[^^car")).toEqual({ from: 5, query: "car" });
    expect(blockTrigger("[[^^")).toEqual({ from: 0, query: "" });
  });
  it("ignores a plain link or a closed one", () => {
    expect(blockTrigger("[[car")).toBeNull();
    expect(blockTrigger("[[^^car]] x")).toBeNull();
    expect(blockTrigger("[[Note#^ab")).toBeNull();
  });
});

describe("firstLine", () => {
  it("is the first line, trimmed of list markers", () => {
    expect(firstLine(para)).toBe("para one");
    expect(firstLine({ ...para, text: "- item\n  - child" })).toBe("item");
    expect(firstLine(head)).toBe("Title");
  });
});
