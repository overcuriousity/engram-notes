import { describe, expect, it } from "vitest";
import { fileKind, imageSize, resolveFile, tabTitle } from "./files";

describe("fileKind", () => {
  it("sorts files by what can show them", () => {
    expect(fileKind("a/B.MD")).toBe("note");
    expect(fileKind("pics/x.PNG")).toBe("image");
    expect(fileKind("scan.webp")).toBe("image");
    expect(fileKind("paper.pdf")).toBe("pdf");
    expect(fileKind("data.csv")).toBe("other");
    expect(fileKind("Makefile")).toBe("other");
  });
});

describe("file resolution and titles", () => {
  const files = ["deep/er/pic.png", "img/pic.png", "Notes/a.md"].map((path) => ({ path, mtime_ms: 0, size: 0, is_markdown: path.endsWith(".md") }));
  it("resolves exact paths, then the shortest path with that name", () => {
    expect(resolveFile(files, "deep/er/pic.png")).toBe("deep/er/pic.png");
    expect(resolveFile(files, "PIC.png")).toBe("img/pic.png");
    expect(resolveFile(files, "gone.png")).toBeNull();
  });
  it("reads Obsidian's image sizes", () => {
    expect(imageSize("300")).toEqual({ width: 300 });
    expect(imageSize("300x200")).toEqual({ width: 300, height: 200 });
    expect(imageSize("a caption")).toEqual({});
    expect(imageSize()).toEqual({});
  });
  it("knows bases and graph tabs", () => {
    expect(fileKind("x/Books.base")).toBe("base");
    expect(fileKind("graph:local")).toBe("graph");
    expect(tabTitle("x/Books.base")).toBe("Books");
    expect(tabTitle("graph:global")).toBe("Graph view");
    expect(tabTitle("graph:local")).toBe("Local graph");
    expect(tabTitle("a/Note.md")).toBe("Note");
  });
});
