import { describe, expect, it } from "vitest";
import { bandDrag, resizeEdge, RESIZE_MARGIN } from "./frame";

// The band's background finds no control above it; a control, or an icon
// inside one, finds itself.
const background = { closest: () => null };
const control = { closest: () => ({}) };

describe("bandDrag", () => {
  it("drags on the band's own background", () => {
    expect(bandDrag(background, 10, 40)).toBe(true);
  });

  it("does not drag from a control in the band", () => {
    expect(bandDrag(control, 10, 40)).toBe(false);
  });

  it("drags when the press hit nothing at all", () => {
    expect(bandDrag(null, 10, 40)).toBe(true);
  });

  it("does not drag below the band", () => {
    expect(bandDrag(background, 41, 40)).toBe(false);
  });
});

describe("resizeEdge", () => {
  const w = 800;
  const h = 600;
  const m = RESIZE_MARGIN;

  it("finds each corner", () => {
    expect(resizeEdge(1, 1, w, h)).toBe("NorthWest");
    expect(resizeEdge(w - 1, 1, w, h)).toBe("NorthEast");
    expect(resizeEdge(1, h - 1, w, h)).toBe("SouthWest");
    expect(resizeEdge(w - 1, h - 1, w, h)).toBe("SouthEast");
  });

  it("finds each side", () => {
    expect(resizeEdge(400, 1, w, h)).toBe("North");
    expect(resizeEdge(400, h - 1, w, h)).toBe("South");
    expect(resizeEdge(1, 300, w, h)).toBe("West");
    expect(resizeEdge(w - 1, 300, w, h)).toBe("East");
  });

  it("is nothing inside the window", () => {
    expect(resizeEdge(400, 300, w, h)).toBe(null);
    expect(resizeEdge(m + 1, m + 1, w, h)).toBe(null);
  });

  // The band drags the window, so its top edge still has to resize it.
  it("wins over the band at the very top", () => {
    expect(resizeEdge(400, 0, w, h)).toBe("North");
  });
});
