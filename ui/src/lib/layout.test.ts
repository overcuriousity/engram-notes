import { describe, expect, it } from "vitest";
import { closeTab, findPane, maxId, otherPaneId, panes, removePane, renamePath, split, withoutPath, type Node, type Pane } from "./layout";

const pane = (id: number, ...paths: string[]): Pane => ({
  kind: "pane",
  id,
  tabs: paths.map((path) => ({ path, mode: "live" as const })),
  active: paths.length ? 0 : -1,
});
const ids = (n: Node) => panes(n).map((p) => p.id);

describe("layout", () => {
  it("splits beside a pane, reusing a split that runs the same way", () => {
    let root: Node = pane(1, "a.md");
    root = split(root, 1, "row", pane(2, "a.md"), 10);
    expect(root).toMatchObject({ kind: "split", id: 10, dir: "row", sizes: [0.5, 0.5] });
    root = split(root, 2, "row", pane(3), 11);
    expect(root).toMatchObject({ id: 10, sizes: [0.5, 0.25, 0.25] });
    root = split(root, 1, "column", pane(4), 12);
    expect(ids(root)).toEqual([1, 4, 2, 3]);
    expect(root.kind === "split" && root.children[0]).toMatchObject({ kind: "split", id: 12, dir: "column" });
    expect(maxId(root)).toBe(12);
  });

  it("gives a removed pane's space away and collapses a lone child", () => {
    const root = split(split(pane(1), 1, "row", pane(2), 10), 2, "row", pane(3), 11);
    const out = removePane(root, 2);
    expect(out).toMatchObject({ id: 10, sizes: [0.75, 0.25] });
    expect(removePane(out, 3)).toEqual(pane(1));
    expect(removePane(pane(1), 1)).toEqual(pane(1));
  });

  it("activates the neighbour of a closed tab and closes an emptied pane", () => {
    const p = { ...pane(1, "a.md", "b.md", "c.md"), active: 1 };
    const one = closeTab(p, 1, 1) as Pane;
    expect(one.tabs.map((t) => t.path)).toEqual(["a.md", "c.md"]);
    expect(one.active).toBe(1);
    expect((closeTab(p, 1, 0) as Pane).active).toBe(0);
    const two = split(pane(1, "a.md"), 1, "row", pane(2, "b.md"), 10);
    expect(closeTab(two, 2, 0)).toEqual(pane(1, "a.md"));
    expect(closeTab(pane(1, "a.md"), 1, 0)).toEqual(pane(1));
  });

  it("drops and renames a path in every pane", () => {
    const root = split(pane(1, "a.md", "b.md"), 1, "row", pane(2, "a.md"), 10);
    expect(withoutPath(root, "a.md")).toEqual(pane(1, "b.md"));
    const renamed = renamePath(root, "a.md", "z.md");
    expect(panes(renamed).flatMap((p) => p.tabs.map((t) => t.path))).toEqual(["z.md", "b.md", "z.md"]);
    expect(findPane(renamed, 2)?.tabs[0].path).toBe("z.md");
  });
});

describe("folders in the layout", () => {
  it("renames and closes everything under a folder", () => {
    const root: Node = {
      kind: "pane", id: 1, active: 2,
      tabs: [{ path: "d/a.md", mode: "live" }, { path: "d.md", mode: "live" }, { path: "d/e/b.md", mode: "live" }],
    };
    expect(panes(renamePath(root, "d", "x/d"))[0].tabs.map((t) => t.path)).toEqual(["x/d/a.md", "d.md", "x/d/e/b.md"]);
    expect(panes(withoutPath(root, "d"))[0].tabs.map((t) => t.path)).toEqual(["d.md"]);
  });

  it("names the pane a note should open into beside the active one", () => {
    const one: Node = { kind: "pane", id: 1, tabs: [], active: -1 };
    expect(otherPaneId(one, 1)).toBeNull();
    const two: Node = {
      kind: "split", id: 9, dir: "row", sizes: [0.5, 0.5],
      children: [one, { kind: "pane", id: 2, tabs: [], active: -1 }],
    };
    expect(otherPaneId(two, 1)).toBe(2);
    expect(otherPaneId(two, 2)).toBe(1);
    // An id that is not in the tree falls back to the first pane.
    expect(otherPaneId(two, 99)).toBe(1);
  });
});
