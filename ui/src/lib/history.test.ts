import { describe, expect, it } from "vitest";
import { back, canBack, canForward, forward, visit } from "./history";
import type { TabRef } from "./layout";

const tab = (path: string): TabRef => ({ path, mode: "live" });

describe("tab history", () => {
  it("remembers where it came from", () => {
    const t = tab("A.md");
    visit(t, "B.md");
    expect(t.path).toBe("B.md");
    expect(canBack(t)).toBe(true);
    expect(back(t)).toBe("A.md");
    expect(t.path).toBe("A.md");
    expect(canForward(t)).toBe(true);
    expect(forward(t)).toBe("B.md");
  });

  it("has nowhere to go at either end", () => {
    const t = tab("A.md");
    expect(canBack(t)).toBe(false);
    expect(back(t)).toBe(null);
    expect(forward(t)).toBe(null);
  });

  it("drops the forward trail once you go somewhere new", () => {
    const t = tab("A.md");
    visit(t, "B.md");
    back(t);
    visit(t, "C.md");
    expect(canForward(t)).toBe(false);
    expect(back(t)).toBe("A.md");
  });

  it("visiting the note already shown is not a move", () => {
    const t = tab("A.md");
    visit(t, "A.md");
    expect(canBack(t)).toBe(false);
  });
});
