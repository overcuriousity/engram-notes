import { describe, expect, it } from "vitest";
import { indentOf, indentSubtree, moveSubtree, nextSibling, outdentSubtree, outlinePaths, parseItem, prevSibling, renumber, subtreeEnd } from "./outline";

const L = (s: string) => s.split("\n");

describe("outline items", () => {
  it("parses bullets, numbers and tasks, a tab counting four columns", () => {
    expect(parseItem("- a")).toEqual({ indent: 0, marker: "-", task: null, text: "a" });
    expect(parseItem("  3) b")).toEqual({ indent: 2, marker: "3)", task: null, text: "b" });
    expect(parseItem("\t- [x] c")).toEqual({ indent: 4, marker: "-", task: "[x]", text: "c" });
    expect(parseItem("- ")).toEqual({ indent: 0, marker: "-", task: null, text: "" });
    expect(parseItem("plain")).toBeNull();
    expect(parseItem("-no space")).toBeNull();
    expect(indentOf("\t  x")).toBe(6);
  });

  it("ends a subtree where indentation returns, blank lines included when deeper text follows", () => {
    const l = L("- a\n  - b\n\n    more b\n- c");
    expect(subtreeEnd(l, 0)).toBe(4);
    expect(subtreeEnd(l, 1)).toBe(4);
    expect(subtreeEnd(l, 4)).toBe(5);
  });

  it("finds siblings across a subtree and stops at prose", () => {
    const l = L("- a\n  - a1\n- b\n\ntext\n- c");
    expect(prevSibling(l, 2)).toBe(0);
    expect(nextSibling(l, 0)).toBe(2);
    expect(nextSibling(l, 2)).toBeNull();
    expect(prevSibling(l, 5)).toBeNull();
    expect(prevSibling(l, 1)).toBeNull();
  });

  it("keys every item by list block and sibling indices", () => {
    const l = L("- a\n  - a1\n  - a2\n- b\n\ntext\n\n1. c\n   text under c\n2. d");
    expect([...outlinePaths(l)]).toEqual([
      [0, "0:0"], [1, "0:0.0"], [2, "0:0.1"], [3, "0:1"], [7, "1:0"], [9, "1:1"],
    ]);
  });
});

describe("indent and outdent", () => {
  it("moves the item with its subtree and normalises tabs it touches to spaces", () => {
    expect(indentSubtree(L("- a\n- b\n\t- c\n- d"), 1, 2)).toEqual(L("- a\n  - b\n      - c\n- d"));
    expect(outdentSubtree(L("- a\n  - b\n    - c\n- d"), 1, 2)).toEqual(L("- a\n- b\n  - c\n- d"));
    expect(outdentSubtree(L("- a"), 0, 2)).toBeNull();
  });

  it("outdents by what is there when the indent is short of a unit", () => {
    expect(outdentSubtree(L("- a\n - b"), 1, 2)).toEqual(L("- a\n- b"));
  });

  it("renumbers the ordered list it changed, each run from one", () => {
    expect(outdentSubtree(L("1. a\n  1. b\n  2. c\n2. d"), 1, 2)).toEqual(L("1. a\n2. b\n  1. c\n3. d"));
    expect(renumber(L("3. x\n\ntext\n\n1. a\n1. b"), 4)).toEqual(L("3. x\n\ntext\n\n1. a\n2. b"));
    expect(renumber(L("- a\n1. b\n1. c"), 1)).toEqual(L("- a\n1. b\n2. c"));
  });
});

describe("move", () => {
  it("swaps a subtree with its sibling and keeps the blank between them", () => {
    const l = L("- a\n  - a1\n\n- b\n- c");
    expect(moveSubtree(l, 3, -1)).toEqual({ lines: L("- b\n\n- a\n  - a1\n- c"), line: 0 });
    expect(moveSubtree(l, 0, 1)).toEqual({ lines: L("- b\n\n- a\n  - a1\n- c"), line: 2 });
    expect(moveSubtree(l, 4, 1)).toBeNull();
    expect(moveSubtree(l, 1, -1)).toBeNull();
  });

  it("renumbers after a move", () => {
    expect(moveSubtree(L("1. a\n2. b"), 1, -1)).toEqual({ lines: L("1. b\n2. a"), line: 0 });
  });
});
