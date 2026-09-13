import { describe, expect, it } from "vitest";
import { allCommands, chord, defaults } from "./commands";
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
      search: { candidate_multiplier: 3, rrf_k: 60, cliff_factor: 3, cliff_min_share: 0.01 },
      memory: {
        enabled: true, activation_half_life_days: 30, assoc_half_life_days: 90,
        sitting_gap_secs: 1800, assoc_window_secs: 600, assoc_show: 2,
        prime_margin: 0.5, prime_lift: 2, spread_max: 3,
      },
      embed: { model_dir: null, batch: 32 },
    };
    const byId = Object.fromEntries(allCommands().map((c) => [c.id, c.hotkey]));
    expect(byId["new-note"]).toBe("Ctrl+Alt+N");
    expect(byId["switcher"]).toBe("Ctrl+O");
    expect(byId["split-right"]).toBe("");
  });

  it("offers the memory and model commands", () => {
    const ids = defaults.map((c) => c.id);
    expect(ids).toContain("memory-toggle");
    expect(ids).toContain("memory-forget");
    expect(ids).toContain("model-dir");
  });
});
