import { describe, expect, it } from "vitest";
import { fileKind } from "./files";

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
