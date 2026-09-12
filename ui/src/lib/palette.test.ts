import { describe, expect, it } from "vitest";
import { createName, matchCommands, matchNotes } from "./palette";
import type { Command } from "./commands";

const titles: [string, string][] = [
  ["Projects/Roadmap.md", "Roadmap"],
  ["Road trip.md", "Road trip"],
  ["Daily/2026-09-12.md", "2026-09-12"],
  ["Ideas.md", "Ideas about roads"],
];

describe("matchNotes", () => {
  it("ranks exact, then prefix, then substring title matches", () => {
    expect(matchNotes(titles, "road").map((m) => m.title)).toEqual(["Roadmap", "Road trip", "Ideas about roads"]);
    expect(matchNotes(titles, "roadmap")[0]).toEqual({ path: "Projects/Roadmap.md", title: "Roadmap" });
  });

  it("matches paths and returns everything for an empty query", () => {
    expect(matchNotes(titles, "daily").map((m) => m.path)).toEqual(["Daily/2026-09-12.md"]);
    expect(matchNotes(titles, "  ")).toHaveLength(4);
  });

  it("offers Create only when no title matches exactly", () => {
    expect(createName(matchNotes(titles, "road"), " road ")).toBe("road");
    expect(createName(matchNotes(titles, "roadmap"), "RoadMap")).toBeNull();
    expect(createName([], "   ")).toBeNull();
  });
});

describe("matchCommands", () => {
  it("filters by name, case-insensitively", () => {
    const cmds: Command[] = [
      { id: "a", name: "Split right", hotkey: "", run: () => {} },
      { id: "b", name: "New note", hotkey: "Ctrl+N", run: () => {} },
    ];
    expect(matchCommands(cmds, "SPLIT").map((c) => c.id)).toEqual(["a"]);
    expect(matchCommands(cmds, "")).toHaveLength(2);
  });
});
