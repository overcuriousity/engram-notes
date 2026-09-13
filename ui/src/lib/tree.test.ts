import { describe, expect, it } from "vitest";
import { buildTree, dropTarget, fileTag, freeName, renameTarget } from "./tree";

const f = (path: string) => ({ path, mtime_ms: 0, size: 0, is_markdown: path.endsWith(".md") });

describe("explorer tree", () => {
  it("shows empty folders, folders first, notes without .md", () => {
    const t = buildTree([f("b/n.md"), f("a.md"), f("pic.png")], ["b", "b/empty", "c"]);
    expect(t.dirs.map((d) => d.path)).toEqual(["b", "c"]);
    expect(t.dirs[0].dirs.map((d) => d.path)).toEqual(["b/empty"]);
    expect(t.dirs[0].files).toEqual([{ name: "n", path: "b/n.md" }]);
    expect(t.files.map((x) => x.name)).toEqual(["a", "pic.png"]);
  });

  it("drops into another folder, never into itself or where it already is", () => {
    expect(dropTarget("a/n.md", "b")).toBe("b/n.md");
    expect(dropTarget("a/n.md", "")).toBe("n.md");
    expect(dropTarget("a/n.md", "a")).toBeNull();
    expect(dropTarget("a", "a/sub")).toBeNull();
    expect(dropTarget("a", "a")).toBeNull();
    expect(dropTarget("ab", "a")).toBe("a/ab");
  });

  it("renames in place, implying .md for notes", () => {
    expect(renameTarget("d/Old.md", "New", true)).toBe("d/New.md");
    expect(renameTarget("pic.png", "photo.png", false)).toBe("photo.png");
    expect(renameTarget("d/sub", "other", false)).toBe("d/other");
    expect(renameTarget("d/Old.md", " Old ", true)).toBeNull();
    expect(renameTarget("d/Old.md", "", true)).toBeNull();
    expect(() => renameTarget("d/Old.md", "a/b", true)).toThrow();
  });

  it("finds a free Untitled name", () => {
    expect(freeName(["Untitled.md", "d/Untitled.md"], "", "Untitled", ".md")).toBe("Untitled 1.md");
    expect(freeName(["d/Untitled"], "d", "Untitled")).toBe("d/Untitled 1");
  });

  it("badges a file with its extension, and a note with nothing", () => {
    expect(fileTag("Kitchen/crumb.png")).toBe("PNG");
    expect(fileTag("Kitchen/Recipes.base")).toBe("BASE");
    expect(fileTag("a/b/paper.PDF")).toBe("PDF");
    expect(fileTag("Index.md")).toBeNull();
    expect(fileTag("Index.MD")).toBeNull();
    expect(fileTag("LICENSE")).toBeNull();
  });
});
