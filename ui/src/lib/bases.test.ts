import { describe, expect, it } from "vitest";
import { cellText, nextSort, sortMark } from "./bases";

describe("base table helpers", () => {
  it("sorts by a column, flipping it when it already leads", () => {
    expect(nextSort([], "note.a")).toEqual([{ property: "note.a", direction: "ASC" }]);
    expect(nextSort([{ property: "a", direction: "ASC" }], "note.a")).toEqual([{ property: "a", direction: "DESC" }]);
    expect(nextSort([{ property: "note.a", direction: "DESC" }], "note.a")).toEqual([{ property: "note.a", direction: "ASC" }]);
    expect(nextSort([{ property: "note.a", direction: "ASC" }], "file.name")).toEqual([{ property: "file.name", direction: "ASC" }]);
  });

  it("marks the column that leads the sort", () => {
    expect(sortMark([{ property: "rating", direction: "DESC" }], "note.rating")).toBe(" ↓");
    expect(sortMark([{ property: "note.rating", direction: "ASC" }], "note.rating")).toBe(" ↑");
    expect(sortMark([], "note.rating")).toBe("");
  });

  it("shows read-only cells as text", () => {
    expect(cellText(null)).toBe("");
    expect(cellText(["a", 1])).toBe("a, 1");
    expect(cellText(true)).toBe("true");
    expect(cellText({ a: 1 })).toBe('{"a":1}');
  });
});
