import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { app } from "./state.svelte";

vi.mock("./api", async (orig) => ({
  ...(await orig<typeof api>()),
  writeNote: vi.fn(),
  listFiles: vi.fn(async () => []),
  listFolders: vi.fn(async () => []),
  titles: vi.fn(async () => []),
}));

const write = vi.mocked(api.writeNote);
// closeTab lets the release run on its own; a macrotask boundary drains it.
const settled = () => new Promise((r) => setTimeout(r, 0));
const edited = (path: string) => ({ path, text: "edited", savedText: "on disk", mtime_ms: 1, conflict: false });

// A buffer the user cannot see again is the only copy of what they typed: a
// write that failed must not be the end of it.
describe("a save that does not land", () => {
  beforeEach(() => {
    write.mockReset();
    app.docs = {};
    app.toast = null;
    app.layout = { kind: "pane", id: 1, tabs: [{ path: "a.md", mode: "live" }], active: 0 };
    app.activePane = 1;
  });

  it("keeps the buffer of a closed tab", async () => {
    write.mockRejectedValue(new Error("read-only file system"));
    app.docs["a.md"] = edited("a.md");
    app.closeTab(1, 0);
    await settled();
    expect(write).toHaveBeenCalled();
    expect(app.docs["a.md"]?.text).toBe("edited");
  });

  it("drops the buffer of a closed tab once it is on disk", async () => {
    write.mockResolvedValue(2);
    app.docs["a.md"] = edited("a.md");
    app.closeTab(1, 0);
    await settled();
    expect(app.docs["a.md"]).toBeUndefined();
  });

  it("does not switch vaults", async () => {
    write.mockRejectedValue(new Error("read-only file system"));
    app.docs["a.md"] = edited("a.md");
    const open = vi.spyOn(app, "open").mockResolvedValue();
    await app.switchVault("/other/vault");
    expect(open).not.toHaveBeenCalled();
    expect(app.docs["a.md"]?.text).toBe("edited");
    open.mockRestore();
  });
});
