# Obsidian parity, third pass: the frameless window, Ctrl+K search and the graph

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking.

**Goal:** Close the rest of the gap the user found beside Obsidian: take the
system titlebar away so the header band is the top of the window, move search
to a Ctrl+K modal with an icon beside it, make graph nodes clickable and able
to open into the other split, and fix the five smaller measured findings.

**Architecture:** The window becomes frameless (`decorations: false`) and the
band grows a drag region and our own minimise / maximise / close buttons —
WebKitGTK ignores `-webkit-app-region`, so dragging goes through Tauri's
`startDragging()`. Search stops being a sidebar view and becomes a third mode
on the `Palette` modal that already serves Ctrl+P and Ctrl+O, which lets the
left sidebar collapse to one view and hand its actions to the band, as
Obsidian's does. The graph keeps its 2D canvas; what it gains is a hit target
you can actually hit, a view that fits the graph, an eased zoom, and Ctrl-click
into the other pane.

**Tech Stack:** Svelte 5 (runes), TypeScript, vitest, `@lucide/svelte`,
`@tauri-apps/api/window`, d3-force, plain CSS in `ui/src/app.css`.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`

**Branch:** `feat/obsidian-parity-ui`, already checked out and pushed at
`270c638`. Continue on it.

---

## Global Constraints

- Rust 2024, `cargo fmt` and `cargo clippy -D warnings` clean.
- `core` has no Tauri dependency; the frontend never touches the filesystem.
- KISS. One trait per seam, one implementation until a second exists.
- Where a feature is Obsidian's, match Obsidian by measuring it.
- Tests run without a model, a window or the network. Anything reaching for
  `@tauri-apps/api` stays out of the tested modules: the pure helpers go in
  `ui/src/lib/*.ts` and the window calls stay in components.
- Comments short, saying why.
- Conventional commit subjects under 72 characters.
- `cd ui && pnpm check` and `pnpm test` clean before every commit.
- Every UI change verified with `ui/scripts/shot.mjs` in both themes, then in
  the real window.

---

## Departures, and what was measured

**The one thing that is not a copy.** Obsidian renders its graph with WebGL —
`/lib/pixi.min.js` ships in `obsidian.asar`. Ours is a 2D canvas, and the spec
does not ask for a renderer. Matching Obsidian's frame pacing exactly would
mean adopting a WebGL renderer, which is its own project; this plan takes the
four concrete causes of the roughness the user saw and leaves the renderer
alone. **Tell the user this; it is the one item here that is improved rather
than matched.**

**The spec is silent** on the window frame, on where search lives and on graph
interaction, so none of this departs from it. The spec's *Layout* line says
"left sidebar with file explorer … and search"; search remains, reached by
Ctrl+K and by the ribbon's magnifier, which is a change of surface, not of
capability. **Record it in the docs.**

Measured from Obsidian 1.13.7 (`obsidian.asar`, extracted as the previous plan
describes):

| thing | value |
| --- | --- |
| `--file-line-width` | `700px` (ours is 760px) |
| `.status-bar` | items sit at the right edge |
| `.workspace-ribbon .sidebar-toggle-button` | `position: absolute; top: 0; left: 0; width: var(--ribbon-width)` — the toggle fills the band's top-left corner |
| `--header-height` / `--ribbon-width` | `40px` / `44px`, already ours |
| `--frame-right-space` | `126px` on Windows/Linux — the width reserved for the window buttons |
| `.workspace-tab-header-container-inner` | `-webkit-app-region: drag` — the empty part of the tab strip drags the window |

---

## File structure

| file | change | responsibility after this plan |
| --- | --- | --- |
| `src-tauri/tauri.conf.json` | modify | `decorations: false` on the main window |
| `src-tauri/capabilities/default.json` | modify | the window permissions dragging and the buttons need |
| `ui/src/components/WindowControls.svelte` | create | minimise, maximise/restore, close, at the band's right end |
| `ui/src/components/Ribbon.svelte` | modify | the corner sidebar toggle above the ribbon's buttons |
| `ui/src/lib/layout.ts` | modify | adds `otherPaneId(root, activeId)` |
| `ui/src/lib/layout.test.ts` | modify | covers `otherPaneId` |
| `ui/src/lib/state.svelte.ts` | modify | `palette` gains `"search"`; adds `openInOtherPane`; loses `leftPane` |
| `ui/src/components/SearchResults.svelte` | create | the hit list, divider and Associated group, used by the palette |
| `ui/src/components/Search.svelte` | delete | its markup moves into `SearchResults.svelte` |
| `ui/src/components/Palette.svelte` | modify | the third mode |
| `ui/src/lib/commands.ts` | modify | `search` opens the modal on Ctrl+K |
| `ui/src/components/Explorer.svelte` | modify | its actions move into the band's 40px row |
| `ui/src/App.svelte` | modify | no left pane strip, no Search view, window controls in the band |
| `ui/src/components/StatusBar.svelte` | modify | items to the right |
| `ui/src/lib/graph.ts` | modify | adds `hitRadius` and `fitView` |
| `ui/src/lib/graph.test.ts` | modify | covers both |
| `ui/src/components/GraphView.svelte` | modify | eased zoom, fit on load, Ctrl-click to the other pane |
| `ui/src/app.css` | modify | `--file-line-width`, the band's controls, the status bar, the explorer header |

---

### Task 1: The frameless window

Take the system titlebar away and put its three buttons in the band, with the
rest of the band dragging the window.

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Create: `ui/src/components/WindowControls.svelte`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/components/Ribbon.svelte`
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: `getCurrentWindow` from `@tauri-apps/api/window`, already imported
  in `ui/src/lib/state.svelte.ts`.
- Produces: `WindowControls.svelte`, a component taking no props, rendered once
  by `App.svelte` at the right end of the band.

- [ ] **Step 1: Turn the decorations off**

In `src-tauri/tauri.conf.json`, add `"decorations": false` to the window
object, so the line reads:

```json
      { "title": "engram-notes", "width": 1280, "height": 820, "minWidth": 720, "minHeight": 480, "dragDropEnabled": false, "decorations": false }
```

- [ ] **Step 2: Grant the window permissions the buttons need**

`core:default` does not carry them. In
`src-tauri/capabilities/default.json`, replace the permissions array with:

```json
  "permissions": [
    "core:default",
    "core:window:allow-start-dragging",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-is-maximized",
    "core:window:allow-close",
    "dialog:default",
    "opener:default"
  ]
```

- [ ] **Step 3: Write the window controls**

Create `ui/src/components/WindowControls.svelte`:

```svelte
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import Copy from "@lucide/svelte/icons/copy";
  import X from "@lucide/svelte/icons/x";

  const win = getCurrentWindow();
  let maximized = $state(false);
  // The frame is ours, so the restore glyph has to track the real state.
  $effect(() => {
    void win.isMaximized().then((m) => (maximized = m));
    const off = win.onResized(() => void win.isMaximized().then((m) => (maximized = m)));
    return () => void off.then((f) => f());
  });
</script>

<div class="wincontrols">
  <button class="icon" title="Minimise" aria-label="Minimise" onclick={() => win.minimize()}><Minus size={15} strokeWidth={2} /></button>
  <button class="icon" title={maximized ? "Restore" : "Maximise"} aria-label="Maximise" onclick={() => win.toggleMaximize()}>
    {#if maximized}<Copy size={13} strokeWidth={2} />{:else}<Square size={13} strokeWidth={2} />{/if}
  </button>
  <button class="icon close" title="Close" aria-label="Close" onclick={() => win.close()}><X size={16} strokeWidth={2} /></button>
</div>
```

- [ ] **Step 4: Put them in the band and make the band drag**

In `ui/src/App.svelte`, add the imports:

```ts
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import WindowControls from "./components/WindowControls.svelte";
```

and, in the `<script>`, the drag handler — WebKitGTK ignores
`-webkit-app-region`, so the move is asked for by hand:

```ts
  // Only a press on the band's own background drags; a press on a control does not.
  function dragWindow(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, input, a")) return;
    void getCurrentWindow().startDragging();
  }
```

Then, inside `<div class="layout" …>`, immediately after `<Ribbon />`, add the
strip that covers the band's right end:

```svelte
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="bandgrip" onpointerdown={dragWindow} ondblclick={() => getCurrentWindow().toggleMaximize()}></div>
    <WindowControls />
```

- [ ] **Step 5: Style the grip, the controls and the ribbon corner**

Append to `ui/src/app.css`:

```css
/* The band is the titlebar: its empty right end drags the window. */
.bandgrip { position: fixed; top: 0; right: 126px; left: 0; height: var(--header-height); z-index: 0; }
.wincontrols { position: fixed; top: 0; right: 0; height: var(--header-height); display: flex; align-items: stretch; z-index: 5; }
.wincontrols .icon { width: 42px; border-radius: 0; }
.wincontrols .icon:hover { background: var(--bg-3); }
.wincontrols .close:hover { background: #c42b1c; color: #fff; }
```

The grip sits behind everything (`z-index: 0`) so the tab strip, both sidebar
headers and the controls still take their own clicks; it only catches the
background between them.

- [ ] **Step 6: Give the ribbon's corner Obsidian's sidebar toggle**

In `ui/src/components/Ribbon.svelte`, add the import

```ts
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
```

and put the toggle above the `{#each}`, inside `<nav class="ribbon">`:

```svelte
  <button class="icon corner" title="Toggle left sidebar" onclick={() => (app.showLeft = !app.showLeft)}>
    <PanelLeftClose size={18} strokeWidth={1.75} />
  </button>
```

Then style it into the corner the ribbon's `::before` paints — replace that
`::before` rule with a positioned button, since the corner now holds something:

```css
.ribbon .corner { position: absolute; top: calc(var(--header-height) * -1); left: 0; width: 100%; height: var(--header-height); border-radius: 0; background: var(--bg-2); border-bottom: 1px solid var(--border); z-index: 5; }
```

and delete the `.ribbon::before` rule, which the button now replaces.

- [ ] **Step 7: Check, test and shoot**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/frame-light.png 1400 900
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/frame-dark.png 1400 900
```

Read both PNGs. Expected: three window buttons at the band's right end, a
sidebar-toggle icon in the ribbon's top-left corner, and the tab strip
unchanged. The headless shot cannot show the decorations going away — that is
Task 6's job.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json ui/src/components/WindowControls.svelte ui/src/App.svelte ui/src/components/Ribbon.svelte ui/src/app.css
git commit -m "feat(ui): a frameless window with the band as its titlebar"
```

---

### Task 2: Search as a Ctrl+K modal

Move the search results out of the sidebar and into the palette that already
answers Ctrl+P and Ctrl+O, so the three look and behave alike.

**Files:**
- Create: `ui/src/components/SearchResults.svelte`
- Delete: `ui/src/components/Search.svelte`
- Modify: `ui/src/components/Palette.svelte`
- Modify: `ui/src/lib/state.svelte.ts`
- Modify: `ui/src/lib/commands.ts`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: `search(query, limit?)` and `SearchResults` from
  `ui/src/lib/api.ts`; `app.openFromSearch(path, line, query)` from the store.
- Produces: `SearchResults.svelte`, props
  `{ results: SearchResults; query: string; sel: number; onOpen: (path: string, line?: number) => void }`;
  the store's `palette` widens to `"none" | "files" | "commands" | "search"`.

- [ ] **Step 1: Lift the result list into its own component**

Create `ui/src/components/SearchResults.svelte` — the markup is
`Search.svelte`'s, with the query and the results handed in rather than fetched,
and with a selected row so the keyboard can drive it:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { SearchResults } from "../lib/api";

  let { results, sel, onOpen }: {
    results: SearchResults;
    sel: number;
    onOpen: (path: string, line?: number) => void;
  } = $props();

  // The first hit past the fall gets the divider above it.
  const divider = $derived(results.hits.findIndex((h) => h.past_divider));
</script>

<div class="items">
  {#each results.hits as h, i (h.path)}
    {#if i === divider}<div class="divider-line">loose</div>{/if}
    <button class="linkrow" class:active={i === sel} class:loose={h.past_divider} onclick={() => onOpen(h.path, h.line)}>
      <div class="src">
        {h.title}
        {#if h.heading}<span class="dim">— {h.heading}</span>{/if}
        {#if h.primed}<span class="badge" title="you reach for this one">primed</span>{/if}
      </div>
      <!-- escaped in core; only <mark> survives -->
      <div class="ctx">{@html h.snippet}</div>
    </button>
  {/each}
  {#if results.associated.length}
    <div class="pane-title sub">Associated</div>
    {#each results.associated as a, j (a.path)}
      <button class="linkrow" class:active={results.hits.length + j === sel} onclick={() => onOpen(a.path)}>
        <div class="src">{a.title}</div>
        <div class="ctx dim">with {a.via}{a.cue ? ` · “${a.cue}”` : ""}</div>
      </button>
    {/each}
  {/if}
  {#if app.embed.state === "loading"}<div class="pane-note">downloading model…</div>{/if}
</div>
```

- [ ] **Step 2: Widen the store's palette state**

In `ui/src/lib/state.svelte.ts`, replace

```ts
  palette = $state<"none" | "files" | "commands">("none");
```

with

```ts
  palette = $state<"none" | "files" | "commands" | "search">("none");
```

and delete the line

```ts
  leftPane = $state<"files" | "search">("files");
```

- [ ] **Step 3: Teach the palette to search**

In `ui/src/components/Palette.svelte`, extend the imports:

```ts
  import { createNote, errorMessage, recordEvent, search, type SearchResults } from "../lib/api";
  import SearchResults_ from "./SearchResults.svelte";
```

Add the search state and its debounce after `let input = $state<HTMLInputElement>();`:

```ts
  let found = $state<SearchResults>({ hits: [], associated: [] });
  let timer: ReturnType<typeof setTimeout> | undefined;
  const rows = $derived(found.hits.length + found.associated.length);

  $effect(() => {
    if (app.palette !== "search") return;
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      found = query.trim() ? await search(query) : { hits: [], associated: [] };
      if (query.trim()) void recordEvent("search", undefined, query).catch(() => {});
    }, 150);
    return () => clearTimeout(timer);
  });

  // A search row opens its note; the other two modes run their item.
  function openHit(path: string, line?: number) {
    app.palette = "none";
    void app.openFromSearch(path, line, q).catch((e) => app.say(errorMessage(e)));
  }

  function chooseHit(i: number) {
    const hit = found.hits[i];
    if (hit) return openHit(hit.path, hit.line);
    const assoc = found.associated[i - found.hits.length];
    if (assoc) return openHit(assoc.path);
  }
```

Change the reset effect so a fresh open clears the results too:

```ts
  $effect(() => {
    if (app.palette !== "none") {
      q = "";
      sel = 0;
      found = { hits: [], associated: [] };
      setTimeout(() => input?.focus());
    }
  });
```

Make the keyboard handler count the right list:

```ts
  function onKey(e: KeyboardEvent) {
    const last = (app.palette === "search" ? rows : items.length) - 1;
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, last); e.preventDefault(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter") { e.preventDefault(); if (app.palette === "search") chooseHit(sel); else void choose(sel); }
    else if (e.key === "Escape") { app.palette = "none"; }
    e.stopPropagation();
  }
```

And branch the body. Replace the `<div class="items">…</div>` block with:

```svelte
      {#if app.palette === "search"}
        <SearchResults_ results={found} {sel} onOpen={openHit} />
      {:else}
        <div class="items">
          {#each items as it, i (it.label + it.detail)}
            <button class:active={i === sel} onclick={() => choose(i)}><span>{it.label}</span><span class="detail">{it.detail}</span></button>
          {/each}
        </div>
      {/if}
```

and give the input the third placeholder:

```svelte
      <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder={app.palette === "files" ? "Open note…" : app.palette === "search" ? "Search the vault…" : "Run command…"} />
```

Guard the `items` derivation so it does no work in search mode — change its
first line from `if (app.palette === "commands")` to:

```ts
    if (app.palette === "search") return [];
    if (app.palette === "commands") {
```

- [ ] **Step 4: Bind Ctrl+K and keep the icon**

In `ui/src/lib/commands.ts`, replace the `search` command with:

```ts
  { id: "search", name: "Search in all files", hotkey: "Ctrl+K", run: () => (app.palette = "search") },
```

The ribbon's magnifier already runs the `search` command by id, so the icon
opens the modal with no further change — that is the discoverable entry for
anyone who has not learned the chord.

- [ ] **Step 5: Take the search view out of the sidebar**

In `ui/src/App.svelte`, delete the `Search` import and replace the whole left
`<aside>` body with the explorer alone:

```svelte
    <aside class="sidebar">
      {#if app.showLeft}<Explorer />{/if}
    </aside>
```

Then delete `ui/src/components/Search.svelte`:

```bash
rm /home/user01/Projekte/engram-notes/ui/src/components/Search.svelte
```

- [ ] **Step 6: Let the palette hold a taller list**

In `ui/src/app.css`, add after the `.palette .items` rule:

```css
/* Search rows are two lines each, so the modal is given more room than a command list. */
.palette .items .linkrow { border-bottom: none; padding: 6px 14px; }
.palette .items .linkrow.active { background: var(--accent-bg); }
.palette .items .divider-line { margin: 6px 14px 2px; }
.palette .items .pane-title.sub { padding: 8px 14px 3px; margin-top: 4px; }
```

- [ ] **Step 7: Check, test and shoot the modal**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
EVAL='window.__app_search()' node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/search-light.png 1400 900
```

`EVAL` has no handle on the store, so drive it through the ribbon instead —
the magnifier is the ribbon's third button:

```bash
EVAL='document.querySelectorAll(".ribbon button")[2].click()' \
  node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/search-light.png 1400 900
EVAL='document.querySelectorAll(".ribbon button")[2].click()' \
  node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/search-dark.png 1400 900
```

Read both PNGs. Expected: the palette opens with the placeholder *Search the
vault…*, and the left sidebar shows the file tree with no Files/Search strip
above it. The fixture's stub answers `search` from its `search` key, which this
fixture does not set, so the list is empty — that is correct here; Task 6 tries
a real query in the real window.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add -A ui/src ui/src/lib
git commit -m "feat(ui): search as a Ctrl+K modal beside the palette"
```

---

### Task 3: The left sidebar's single row

With only one view left in the left sidebar, its actions belong in the band, as
Obsidian puts them there. This deletes a whole 26px row.

**Files:**
- Modify: `ui/src/components/Explorer.svelte`
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing other components read.

- [ ] **Step 1: Make the explorer's header the band's row**

In `ui/src/components/Explorer.svelte`, replace the whole
`<div class="pane-title explorer-head">…</div>` block with a band-height row of
icons, as Obsidian's file explorer has:

```svelte
<div class="explorer-head">
  <button class="icon" title="New note" aria-label="New note" onclick={() => guarded(() => newNote())}><FilePlus size={16} strokeWidth={1.75} /></button>
  <button class="icon" title="New folder" aria-label="New folder" onclick={() => newFolderIn("")}><FolderPlus size={16} strokeWidth={1.75} /></button>
  <button class="icon" title="Collapse all" aria-label="Collapse all" onclick={collapseAll}><ChevronsDownUp size={16} strokeWidth={1.75} /></button>
</div>
```

Add the three icon imports beside the chevron import:

```ts
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import ChevronsDownUp from "@lucide/svelte/icons/chevrons-down-up";
```

and the action the third button runs, after `focusSelect`:

```ts
  // Obsidian's collapse-all: every folder in the vault, not only the open ones.
  function collapseAll() {
    const next: Record<string, boolean> = {};
    for (const d of app.folders) next[d] = true;
    collapsed = next;
  }
```

- [ ] **Step 2: Style it to the band**

In `ui/src/app.css`, replace the two `.explorer-head` rules with:

```css
/* The explorer's actions live in the band, as Obsidian's do; there is no second header. */
.explorer-head { display: flex; align-items: center; gap: 2px; height: var(--header-height); padding: 0 6px; background: var(--bg-2); border-bottom: 1px solid var(--border); }
```

and change the `.tree` rule's height allowance, which assumed two rows:

```css
.tree { min-height: calc(100% - var(--header-height)); padding: 4px 0 24px; }
```

- [ ] **Step 3: Check, test and shoot**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/sidebar-light.png 1400 900
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/sidebar-dark.png 1400 900
```

Read both PNGs. Expected: the left sidebar has exactly one 40px row, holding
three icons, level with the tab strip and the right sidebar's icons; the first
file sits directly beneath it; no *FILES* caption anywhere.

- [ ] **Step 4: Shoot collapse-all**

```bash
EVAL='document.querySelectorAll(".explorer-head button")[2].click()' \
  node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/sidebar-collapsed.png 1400 900
```

Read the PNG. Expected: `Kitchen` and `Rust` both closed, their chevrons
pointing right, `Index` still visible.

- [ ] **Step 5: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/components/Explorer.svelte ui/src/app.css
git commit -m "feat(ui): the explorer's actions move into the band"
```

---

### Task 4: The three remaining measurements

**Files:**
- Modify: `ui/src/app.css`
- Modify: `ui/src/components/StatusBar.svelte`

**Interfaces:** none.

- [ ] **Step 1: Obsidian's text column**

In `ui/src/app.css`, add to `:root`, beside `--header-height`:

```css
  --file-line-width: 700px;
```

and replace both hard-coded widths with it — the editor's content:

```css
.cm-editor .cm-content { max-width: var(--file-line-width); margin: 0 auto; padding: 24px 32px; }
```

and the properties block, which shares the note's column:

```css
.propblock { max-width: var(--file-line-width); margin: 0 auto; padding: 8px 32px 2rem; font-size: var(--font-ui-small); }
```

Check there is no third 760 left:

```bash
grep -n "760px" /home/user01/Projekte/engram-notes/ui/src/app.css
```

Expected: no output.

- [ ] **Step 2: The status bar to the right**

Obsidian's status bar items sit at the right edge. In
`ui/src/components/StatusBar.svelte`, move the spacer from the middle to the
front: delete the line

```svelte
  <span style="flex:1"></span>
```

and add it as the first child of the status bar, immediately after the opening
`<div class="statusbar">` tag.

- [ ] **Step 3: Check, test and shoot**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/measure-light.png 1400 900
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/measure-dark.png 1400 900
```

Read both PNGs. Expected: the note's paragraph wraps 60px sooner, the
properties block keeps the same column as the text, and every status-bar item
sits at the window's right edge.

- [ ] **Step 4: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/app.css ui/src/components/StatusBar.svelte
git commit -m "feat(ui): Obsidian's 700px column and a right-aligned status bar"
```

---

### Task 5: The graph — reachable nodes, a fitted view, an eased zoom

Four concrete causes of what the user saw. The renderer stays a 2D canvas;
say so.

**Files:**
- Modify: `ui/src/lib/graph.ts`
- Modify: `ui/src/lib/graph.test.ts`
- Modify: `ui/src/lib/layout.ts`
- Modify: `ui/src/lib/layout.test.ts`
- Modify: `ui/src/lib/state.svelte.ts`
- Modify: `ui/src/components/GraphView.svelte`

**Interfaces:**
- Consumes: `panes(root)` and `Node` from `ui/src/lib/layout.ts`; `radius`,
  `GraphSettings` from `ui/src/lib/graph.ts`.
- Produces:
  - `hitRadius(r: number, k: number): number` in `graph.ts` — how close a
    pointer must come, in world units.
  - `fitView(points: {x: number, y: number, r: number}[], w: number, h: number): {x: number, y: number, k: number}` in `graph.ts`.
  - `otherPaneId(root: Node, activeId: number): number | null` in `layout.ts`.
  - `app.openInOtherPane(path: string): Promise<void>` on the store.

- [ ] **Step 1: Write the failing tests for the two graph helpers**

Append to `ui/src/lib/graph.test.ts`, inside the outermost `describe`:

```ts
  it("gives a small node a hit target the pointer can actually reach", () => {
    // A 2px node at half zoom is 1px on screen; the target stays 10 screen px.
    expect(hitRadius(2, 0.5)).toBeCloseTo(20, 5);
    // A large node keeps its own radius plus a little slack.
    expect(hitRadius(40, 1)).toBeCloseTo(44, 5);
    expect(hitRadius(40, 2)).toBeCloseTo(42, 5);
  });

  it("fits the view to the points it is given", () => {
    const pts = [
      { x: -100, y: -50, r: 4 },
      { x: 100, y: 50, r: 4 },
    ];
    const v = fitView(pts, 800, 400);
    // The 208x108 box fits 800x400 less its padding; height binds first here.
    expect(v.k).toBeGreaterThan(1);
    expect(v.k).toBeLessThanOrEqual(2);
    // The box is centred on the origin, so the view centres the canvas on it.
    expect(v.x).toBeCloseTo(400, 5);
    expect(v.y).toBeCloseTo(200, 5);
  });

  it("leaves the view alone when there is nothing to fit", () => {
    expect(fitView([], 800, 400)).toEqual({ x: 400, y: 200, k: 1 });
  });
```

and add both names to that file's import from `"./graph"`.

- [ ] **Step 2: Run them and watch them fail**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test graph
```

Expected: FAIL — `hitRadius is not a function`, or a TypeScript error that
`"./graph"` has no exported member `hitRadius`.

- [ ] **Step 3: Write the two helpers**

Append to `ui/src/lib/graph.ts`:

```ts
/** How near a pointer must come to a node, in world units: its own radius, or 10 screen pixels. */
export function hitRadius(r: number, k: number): number {
  return Math.max(r + 4 / k, 10 / k);
}

/** The view that shows every point with a margin, centred; the identity view when there are none. */
export function fitView(
  points: { x: number; y: number; r: number }[],
  w: number,
  h: number,
): { x: number; y: number; k: number } {
  if (points.length === 0) return { x: w / 2, y: h / 2, k: 1 };
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of points) {
    x0 = Math.min(x0, p.x - p.r);
    y0 = Math.min(y0, p.y - p.r);
    x1 = Math.max(x1, p.x + p.r);
    y1 = Math.max(y1, p.y + p.r);
  }
  // Room for the labels under the lowest nodes, and a little air all round.
  const pad = 48;
  const k = Math.min(2, Math.max(0.05, Math.min((w - pad * 2) / Math.max(1, x1 - x0), (h - pad * 2) / Math.max(1, y1 - y0))));
  return { x: w / 2 - ((x0 + x1) / 2) * k, y: h / 2 - ((y0 + y1) / 2) * k, k };
}
```

- [ ] **Step 4: Run them and watch them pass**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test graph
```

Expected: PASS, with the pre-existing graph tests still passing.

- [ ] **Step 5: Write the failing test for the other pane**

Append to `ui/src/lib/layout.test.ts`, inside its outermost `describe`:

```ts
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
```

and add `otherPaneId` to that file's import from `"./layout"`. If the file does
not already import the `Node` type, add it:

```ts
import { ..., otherPaneId, type Node } from "./layout";
```

- [ ] **Step 6: Run it and watch it fail**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test layout
```

Expected: FAIL — no exported member `otherPaneId`.

- [ ] **Step 7: Write it**

Append to `ui/src/lib/layout.ts`:

```ts
/** The pane a note should open into beside `activeId`, or null when that is the only one. */
export function otherPaneId(root: Node, activeId: number): number | null {
  const all = panes(root);
  const other = all.find((p) => p.id !== activeId);
  return other?.id ?? null;
}
```

- [ ] **Step 8: Run it and watch it pass**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test layout
```

Expected: PASS.

- [ ] **Step 9: Give the store the move**

In `ui/src/lib/state.svelte.ts`, add after the `split(dir: Dir)` method:

```ts
  /** Ctrl-click from the graph: the note lands in the other pane, and one is made when there is none. */
  async openInOtherPane(path: string) {
    const other = L.otherPaneId(this.layout, this.activePane);
    if (other === null) {
      const fresh: Pane = { kind: "pane", id: this.nextId++, tabs: [], active: -1 };
      this.layout = L.split(this.layout, this.pane.id, "row", fresh, this.nextId++);
      this.activePane = fresh.id;
    } else {
      this.activePane = other;
    }
    await this.openNote(path);
  }
```

- [ ] **Step 10: Make the graph's nodes reachable and its view fit**

In `ui/src/components/GraphView.svelte`, extend the `graph` import to carry the
two new helpers:

```ts
  import { DEFAULTS, filterGraph, fitView, forces, hitRadius, labelAlpha, radius, readSettings, searchWords, type GraphSettings, type ViewGraph, type ViewNode } from "../lib/graph";
```

Replace `nodeAt`'s tolerance so a small node is still catchable:

```ts
  function nodeAt(x: number, y: number): Node | null {
    let best: Node | null = null;
    let dist = Infinity;
    for (const n of nodes) {
      const d = Math.hypot(n.x! - x, n.y! - y);
      if (d < hitRadius(n.r, view.k) && d < dist) {
        best = n;
        dist = d;
      }
    }
    return best;
  }
```

In `rebuild`, delete the guessed initial zoom

```ts
    if (old.size === 0) view.k = Math.min(1.5, Math.max(0.15, Math.sqrt(30 / Math.max(1, nodes.length))));
```

and instead fit once the first layout has settled. Add a flag beside the other
simulation state, near `let frame = 0;`:

```ts
  let fitPending = false;
```

set it in `rebuild` where the old line was:

```ts
    if (old.size === 0) fitPending = true;
```

and add `.on("end", fit)` to the simulation chain, after `.on("tick", draw)`:

```ts
      .on("tick", draw)
      .on("end", fit);
```

Then add the two functions after `resize`:

```ts
  // Obsidian opens a graph showing all of it; ours does the same once it settles.
  function fit() {
    if (!fitPending || !nodes.length || !size.w) return;
    fitPending = false;
    target = fitView(nodes.map((n) => ({ x: n.x!, y: n.y!, r: n.r })), size.w, size.h);
    ease();
  }
```

- [ ] **Step 11: Ease the zoom instead of jumping**

Still in `GraphView.svelte`, add the eased view beside `let view`:

```ts
  let target: { x: number; y: number; k: number } | null = null;
  let easing = 0;
```

and the loop, after `fit`:

```ts
  // A wheel notch or a fit moves the view over a few frames; a drag stays immediate.
  function ease() {
    cancelAnimationFrame(easing);
    const step = () => {
      const t = target;
      if (!t) return;
      const d = { x: t.x - view.x, y: t.y - view.y, k: t.k - view.k };
      if (Math.abs(d.k) < 1e-4 && Math.hypot(d.x, d.y) < 0.5) {
        view = { ...t };
        target = null;
      } else {
        view = { x: view.x + d.x * 0.28, y: view.y + d.y * 0.28, k: view.k + d.k * 0.28 };
        easing = requestAnimationFrame(step);
      }
      draw();
    };
    easing = requestAnimationFrame(step);
  }
```

Point the wheel at it — replace `wheel`'s body:

```ts
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const p = point(e);
    const from = target ?? view;
    const k = Math.min(8, Math.max(0.05, from.k * Math.exp(-e.deltaY * 0.0015)));
    // Zoom about the pointer: the world point under it stays under it.
    const wx = (p.sx - from.x) / from.k;
    const wy = (p.sy - from.y) / from.k;
    target = { x: p.sx - wx * k, y: p.sy - wy * k, k };
    ease();
  }
```

A drag must cancel a running ease, so add to `down`, as its first statement:

```ts
    cancelAnimationFrame(easing);
    target = null;
```

and to the `onMount` cleanup, beside `cancelAnimationFrame(frame)`:

```ts
      cancelAnimationFrame(easing);
```

- [ ] **Step 12: Ctrl-click into the other pane**

Still in `GraphView.svelte`, carry the modifier through the press. Change the
`press` declaration to hold it:

```ts
  let press: { sx: number; sy: number; vx: number; vy: number; node: Node | null; moved: boolean; other: boolean } | null = null;
```

set it in `down`:

```ts
    press = { sx: p.sx, sy: p.sy, vx: view.x, vy: view.y, node, moved: false, other: e.ctrlKey || e.metaKey };
```

use it in `up`:

```ts
    if (!p.moved) void openNode(p.node.data, p.other);
```

and take it in `openNode`:

```ts
  // As in Obsidian: a note opens, an unresolved link creates its note, a tag searches for itself.
  // Ctrl-click sends the note to the other split instead of this one.
  async function openNode(n: ViewNode, other = false) {
    try {
      if (n.kind === "tag") {
        settings.search = `tag:${n.id}`;
      } else if (n.kind === "unresolved") {
        const path = /\.md$/i.test(n.id) ? n.id : `${n.id}.md`;
        await createNote(path);
        await app.refresh();
        if (other) await app.openInOtherPane(path);
        else await app.openNote(path);
      } else if (other) {
        await app.openInOtherPane(n.id);
      } else {
        await app.openNote(n.id);
      }
    } catch (e) {
      say(e);
    }
  }
```

Finally, tell the user the modifier exists: add a line to the settings panel,
just under the `<h4>Display</h4>` heading:

```svelte
      <div class="pane-note">Click a node to open it; Ctrl-click opens it in the other split.</div>
```

- [ ] **Step 13: Check, test and shoot the graph**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
cat > /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/graphfix.json <<'EOF'
{
  "root": "/vault",
  "config": {
    "editor": { "default_mode": "live" },
    "daily_notes": { "folder": "Daily", "template": null, "format": "%Y-%m-%d" },
    "hotkeys": {}, "theme": "light",
    "search": { "limit": 20, "semantic": true, "similarity_floor": 0.83 },
    "memory": { "enabled": true }, "embed": { "model_dir": null }
  },
  "folders": ["Rust", "Kitchen"],
  "files": [
    { "path": "Rust/Borrowing.md", "text": "# Borrowing\n\n[[Rust/Ownership]]\n" },
    { "path": "Rust/Ownership.md", "text": "# Ownership\n\n[[Rust/Borrowing]] and [[Index]]\n" },
    { "path": "Rust/Lifetimes.md", "text": "# Lifetimes\n\n[[Rust/Borrowing]]\n" },
    { "path": "Kitchen/Sourdough.md", "text": "# Sourdough\n\n[[Index]]\n" },
    { "path": "Kitchen/Espresso.md", "text": "# Espresso\n\n[[Index]]\n" },
    { "path": "Index.md", "text": "# Index\n\n[[Rust/Ownership]] and [[Kitchen/Sourdough]]\n" }
  ],
  "graph": {
    "nodes": [
      { "id": "Rust/Borrowing.md", "title": "Borrowing", "kind": "note", "tags": [], "inbound": 2 },
      { "id": "Rust/Ownership.md", "title": "Ownership", "kind": "note", "tags": [], "inbound": 2 },
      { "id": "Rust/Lifetimes.md", "title": "Lifetimes", "kind": "note", "tags": [], "inbound": 0 },
      { "id": "Kitchen/Sourdough.md", "title": "Sourdough", "kind": "note", "tags": [], "inbound": 1 },
      { "id": "Kitchen/Espresso.md", "title": "Espresso", "kind": "note", "tags": [], "inbound": 0 },
      { "id": "Index.md", "title": "Index", "kind": "note", "tags": [], "inbound": 3 }
    ],
    "edges": [
      { "source": "Rust/Borrowing.md", "target": "Rust/Ownership.md" },
      { "source": "Rust/Ownership.md", "target": "Rust/Borrowing.md" },
      { "source": "Rust/Ownership.md", "target": "Index.md" },
      { "source": "Rust/Lifetimes.md", "target": "Rust/Borrowing.md" },
      { "source": "Kitchen/Sourdough.md", "target": "Index.md" },
      { "source": "Kitchen/Espresso.md", "target": "Index.md" },
      { "source": "Index.md", "target": "Rust/Ownership.md" },
      { "source": "Index.md", "target": "Kitchen/Sourdough.md" }
    ]
  },
  "workspace": {
    "layout": { "kind": "pane", "id": 1, "tabs": [{ "path": "graph:global", "mode": "live" }], "active": 0 },
    "activePane": 1
  }
}
EOF
SETTLE=6000 node ui/scripts/shot.mjs /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/graphfix.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/graph-fit.png 1400 900
```

Read the PNG. Expected: the six nodes fill the pane with their labels legible
and well apart, rather than clustering in the middle. If the graph is still a
knot, the fit ran before the simulation settled — raise `SETTLE` and look
again before changing any code.

- [ ] **Step 14: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/lib/graph.ts ui/src/lib/graph.test.ts ui/src/lib/layout.ts ui/src/lib/layout.test.ts ui/src/lib/state.svelte.ts ui/src/components/GraphView.svelte
git commit -m "feat(ui): graph nodes you can hit, a fitted view and an eased zoom"
```

---

### Task 6: The real window

**Files:** `docs/memory.md` (one paragraph).

- [ ] **Step 1: Record the departure**

Add to `docs/memory.md`, under a new final heading:

```markdown
## Where search lives

The spec's *Layout* line puts search in the left sidebar. It is reached by
Ctrl+K and by the ribbon's magnifier instead, on the same modal as the command
palette and the quick switcher, so the three behave alike; the sidebar holds
the file explorer alone and hands its actions to the header band, as Obsidian
does with a single-view sidebar. Nothing about what search *does* changed.
```

- [ ] **Step 2: Run everything**

```bash
cd /home/user01/Projekte/engram-notes
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --workspace
cd ui && pnpm check && pnpm test
```

Expected: clean, 122 Rust tests and at least 74 frontend tests.

- [ ] **Step 3: Build the real window**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm tauri build --debug --no-bundle
ls -l --time-style=+%H:%M:%S /home/user01/Projekte/engram-notes/target/debug/engram-notes; date +%H:%M:%S
```

Expected: the binary's timestamp is from this minute. The Tauri CLI exits 0
even on failure when piped — trust the timestamp.

- [ ] **Step 4: Ask the user to run it**

> `target/debug/engram-notes ~/engram-demo-vault` — I need: the top of the
> window (no system titlebar, our three buttons at the right), dragging the
> window by the band, Ctrl+K with a real query, a graph node clicked and
> Ctrl-clicked, and the explorer's one-row header. Both themes.

The frameless window is the one thing the headless shot cannot check at all,
and dragging on KDE Wayland is the risk: if `startDragging()` does nothing,
say so rather than working around it.

- [ ] **Step 5: Fix what the screenshots show, then push**

```bash
cd /home/user01/Projekte/engram-notes && git push
```

---

## Self-review

**Spec coverage.** The spec's *The editor and UI* section asks for the
explorer, search, tabs, splits, the right sidebar's four views and themes on
stable custom properties. All survive; search changes surface only, which is
recorded in Task 6 Step 1 and flagged to the user above. The *graph* section
asks for pan, zoom, hover and click-to-open — all still present, with click now
reachable and Ctrl-click added. Nothing in the spec speaks to the window frame.

**Placeholders.** None: every step carries its code or its command, and the
graph fixture is written out in full rather than referenced.

**Type consistency.** `hitRadius(r, k)` and `fitView(points, w, h)` are used in
`GraphView.svelte` exactly as declared; `fitView` returns the same
`{x, y, k}` shape as `view` and `target`, so `target = fitView(...)` type-checks.
`otherPaneId(root, activeId)` returns `number | null` and its null branch is the
one that splits. `press` gains `other: boolean`, set in `down` and read in `up`,
and `openNode(n, other = false)` defaults so no other caller breaks.
`palette` widens to four members, and every `app.palette === …` comparison in
`Palette.svelte` and `App.svelte` is against one of them.
