import { describe, expect, it } from "vitest";
import { addItem, parseValue, propKind, removeItem } from "./properties";

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

  it("adds and removes list items", () => {
    expect(addItem(["a"], "  b ")).toEqual(["a", "b"]);
    expect(addItem(["a"], "a")).toEqual(["a"]);
    expect(addItem(["a"], "   ")).toEqual(["a"]);
    expect(removeItem(["a", "b", "c"], 1)).toEqual(["a", "c"]);
  });
});
