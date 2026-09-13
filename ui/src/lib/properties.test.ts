import { describe, expect, it } from "vitest";
import { addItem, parseValue, propIcon, propKind, removeItem } from "./properties";

describe("properties", () => {
  it("reads the kind from the value, as the index types it", () => {
    expect(propKind(true)).toBe("checkbox");
    expect(propKind(3.5)).toBe("number");
    expect(propKind(["a"])).toBe("list");
    expect(propKind("2026-09-12")).toBe("date");
    expect(propKind("2026-09-12T10:30")).toBe("datetime");
    expect(propKind("hello")).toBe("text");
    expect(propKind(null)).toBe("text");
  });

  it("parses a typed value", () => {
    expect(parseValue("")).toBeNull();
    expect(parseValue("true")).toBe(true);
    expect(parseValue("-2.5")).toBe(-2.5);
    expect(parseValue('["a", "b"]')).toEqual(["a", "b"]);
    expect(parseValue("[not json")).toBe("[not json");
    expect(parseValue("words")).toBe("words");
  });

  it("names Obsidian's icon for a property row", () => {
    expect(propIcon("title", "hello")).toBe("text");
    expect(propIcon("count", 3)).toBe("binary");
    expect(propIcon("due", "2026-09-20")).toBe("calendar");
    expect(propIcon("stamped", "2026-09-20T10:30")).toBe("clock");
    expect(propIcon("done", true)).toBe("check-square");
    expect(propIcon("authors", ["a", "b"])).toBe("list");
  });

  it("reads tags and aliases off the key, as Obsidian's widgets do", () => {
    expect(propIcon("tags", ["rust"])).toBe("tags");
    expect(propIcon("tags", "rust")).toBe("tags");
    expect(propIcon("aliases", ["Borrow checker"])).toBe("forward");
    expect(propIcon("cssclasses", ["wide"])).toBe("list");
  });

  it("adds and removes list items", () => {
    expect(addItem(["a"], "  b ")).toEqual(["a", "b"]);
    expect(addItem(["a"], "a")).toEqual(["a"]);
    expect(addItem(["a"], "   ")).toEqual(["a"]);
    expect(removeItem(["a", "b", "c"], 1)).toEqual(["a", "c"]);
  });
});
