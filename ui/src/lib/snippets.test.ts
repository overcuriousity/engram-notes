import { describe, expect, it } from "vitest";
import { activeSnippets, resolvedTheme, toggleSnippet } from "./snippets";

const all = [
  { name: "a", css: "a{}" },
  { name: "b", css: "b{}" },
];

describe("activeSnippets", () => {
  it("keeps file order and drops names with no file", () => {
    expect(activeSnippets(all, ["b", "gone", "a"]).map((s) => s.name)).toEqual(["a", "b"]);
    expect(activeSnippets(all, [])).toEqual([]);
  });
  it("drops a snippet that would not read, so the rest keep applying", () => {
    const withBad = [...all, { name: "bad", css: "", error: "stream did not contain valid UTF-8" }];
    expect(activeSnippets(withBad, ["a", "bad", "b"]).map((s) => s.name)).toEqual(["a", "b"]);
  });
});

describe("toggleSnippet", () => {
  it("adds once and removes", () => {
    expect(toggleSnippet(["a"], "b", true)).toEqual(["a", "b"]);
    expect(toggleSnippet(["a", "b"], "b", true)).toEqual(["a", "b"]);
    expect(toggleSnippet(["a", "b"], "a", false)).toEqual(["b"]);
  });
});

describe("resolvedTheme", () => {
  it("follows the OS only for system", () => {
    expect(resolvedTheme("system", true)).toBe("dark");
    expect(resolvedTheme("system", false)).toBe("light");
    expect(resolvedTheme("light", true)).toBe("light");
    expect(resolvedTheme("dark", false)).toBe("dark");
  });
});
