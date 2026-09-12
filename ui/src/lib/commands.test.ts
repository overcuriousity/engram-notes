import { describe, expect, it } from "vitest";
import { allCommands, chord } from "./commands";
import { app } from "./state.svelte";

const key = (key: string, mods: Partial<KeyboardEvent> = {}) =>
  ({ key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...mods }) as KeyboardEvent;

describe("commands", () => {
  it("writes chords the way hotkeys are written", () => {
    expect(chord(key("f", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+F");
    expect(chord(key("o", { metaKey: true }))).toBe("Ctrl+O");
    expect(chord(key("Escape"))).toBe("Escape");
  });

  it("applies hotkey overrides from app.json", () => {
    app.config = {
      editor: { default_mode: "live" },
      daily_notes: { folder: "Daily", template: null, format: "%Y-%m-%d" },
      hotkeys: { "new-note": "Ctrl+Alt+N" },
      theme: "system",
    };
    const byId = Object.fromEntries(allCommands().map((c) => [c.id, c.hotkey]));
    expect(byId["new-note"]).toBe("Ctrl+Alt+N");
    expect(byId["switcher"]).toBe("Ctrl+O");
    expect(byId["split-right"]).toBe("");
  });
});
