// The window has no decorations, so the frame is ours: the band drags it and
// its edges resize it.

export type Edge = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

/** How far from an edge a press still grabs it, in CSS pixels. */
export const RESIZE_MARGIN = 5;

/** What a press must not land on for the band to drag the window. */
export const CONTROL = "button, input, a, [role='button']";

/** Anything with `closest`: an Element, or a stand-in in a test. */
export interface Target {
  closest(selectors: string): unknown;
}

/** Whether a press in the band should drag the window rather than reach a control. */
export function bandDrag(target: Target | null, clientY: number, header: number): boolean {
  if (clientY >= header) return false;
  return !target?.closest(CONTROL);
}

export function resizeEdge(x: number, y: number, w: number, h: number, margin = RESIZE_MARGIN): Edge | null {
  const north = y <= margin;
  const south = y >= h - margin;
  const west = x <= margin;
  const east = x >= w - margin;
  if (north && west) return "NorthWest";
  if (north && east) return "NorthEast";
  if (south && west) return "SouthWest";
  if (south && east) return "SouthEast";
  if (north) return "North";
  if (south) return "South";
  if (west) return "West";
  if (east) return "East";
  return null;
}

const CURSORS: Record<Edge, string> = {
  North: "ns-resize",
  South: "ns-resize",
  East: "ew-resize",
  West: "ew-resize",
  NorthEast: "nesw-resize",
  SouthWest: "nesw-resize",
  NorthWest: "nwse-resize",
  SouthEast: "nwse-resize",
};

export const edgeCursor = (edge: Edge | null): string => (edge ? CURSORS[edge] : "");
