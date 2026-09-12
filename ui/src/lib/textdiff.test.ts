import { describe, expect, it } from "vitest";
import { textDiff } from "./textdiff";

describe("textDiff", () => {
  it("replaces only the changed middle", () => {
    expect(textDiff("hello world", "hello brave world")).toEqual({ from: 6, to: 6, insert: "brave " });
    expect(textDiff("aaa", "aa")).toEqual({ from: 2, to: 3, insert: "" });
    expect(textDiff("same", "same")).toEqual({ from: 4, to: 4, insert: "" });
    expect(textDiff("", "new")).toEqual({ from: 0, to: 0, insert: "new" });
  });
});
