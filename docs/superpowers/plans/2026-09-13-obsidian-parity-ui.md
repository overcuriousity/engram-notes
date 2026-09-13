# Obsidian-parity UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four gaps the user found comparing engram-notes with
Obsidian side by side: inline properties, a per-tab header with a ribbon,
Obsidian's density and graph style, and switchable right-sidebar views with a
fuller status bar.

**Architecture:** Mostly frontend. The pane takes over scrolling from
CodeMirror so a properties block can sit in the note's flow; tabs gain a history
so back and forward mean something; the right sidebar becomes a view switcher
like the left one. `core` gains one query (properties with counts) and
`src-tauri` one command for it.

**Tech Stack:** Svelte 5, TypeScript, CodeMirror 6, CSS custom properties,
d3-force, Tauri 2, Rust 2024.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md` — *The
editor and UI* and *The graph*. The four items come from the user's own
comparison, recorded in `docs/superpowers/plans/2026-09-13-handoff.md` under
*Next steps*, item 2.

## Global Constraints

- Rust 2024, `cargo fmt --all --check` and `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `pnpm check` and `pnpm test` clean.
- `core` has no Tauri dependency; `src-tauri` is thin; the frontend never
  touches the filesystem.
- Tests run without a model, a window or the network.
- Comments short: why, not what.
- **Where a feature comes from Obsidian, match Obsidian's behaviour and look.**
  Every one of these four does.
- Every UI change is verified with `node ui/scripts/shot.mjs <fixture> <out.png>`
  before it is called done, in both themes where colour is involved. The Vite
  dev server must be running (`cd ui && pnpm dev`).
- Commit messages: conventional prefix, imperative subject under 72 characters.

## Two decisions taken before writing

1. **Following a link navigates the current tab.** Today every link opens
   another tab, so a tab has no history and back and forward would have nothing
   to do. Obsidian replaces the current tab's content and remembers where it
   was. Task 5 changes this, and Task 7 puts new tabs behind the `+` button and
   `Ctrl`-click, as Obsidian does. This is the largest behavioural change in the
   plan; if the user dislikes it, back and forward go with it.
2. **The pane owns the scroll container** (Task 3), so the properties block
   scrolls away with the note as it does in Obsidian, rather than sitting fixed
   above the editor. CodeMirror runs `height: auto` inside it. The cost is some
   of CodeMirror's viewport virtualisation on very long notes; Task 3 measures a
   5 000-line note before and after so the cost is known rather than assumed.

A settings dialog (Task 12) is a fifth item, added because the user asked for
the ribbon's settings icon and we have nowhere for it to lead.

---

## File Structure

**Created (ui):** `src/components/Ribbon.svelte`, `src/components/TabHeader.svelte`,
`src/components/PropertiesBlock.svelte`, `src/components/AllProperties.svelte`,
`src/components/Settings.svelte`, `src/lib/history.ts` + `src/lib/history.test.ts`.

**Modified (ui):** `src/app.css` (density, ribbon, header, properties block),
`src/App.svelte`, `src/lib/state.svelte.ts`, `src/lib/layout.ts` + test,
`src/lib/api.ts`, `src/lib/commands.ts`, `src/lib/graph.ts` + test,
`src/components/{Pane,Tabs,NoteView,Editor,Reading,StatusBar,Properties,GraphView}.svelte`,
`scripts/shot.mjs`.

**Modified (rust):** `core/src/index/query.rs` (`property_counts`),
`src-tauri/src/commands.rs` + `src-tauri/src/lib.rs` (`all_properties`).

**Docs:** `docs/smoke.md`.

---

### Task 1: Obsidian's density

**Files:**
- Modify: `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Produces the type scale every later task uses: `--font-ui` (13px, on `body`),
  `--font-ui-small` (12px), `--font-ui-smaller` (11px), `--row-pad` (2px 8px).
  The editor's own text stays 16px — Obsidian's default — and is untouched.

- [ ] **Step 1: Add the scale to the root tokens**

In `ui/src/app.css`, in the `:root` block beside `--radius`:

```css
  /* Obsidian's interface is smaller than its text and its rows are tight. */
  --font-ui-small: 12px;
  --font-ui-smaller: 11px;
  --row-pad: 2px 8px;
```

- [ ] **Step 2: Shrink the interface**

Change these existing rules only — the editor and reading text keep their size:

```css
body { background: var(--bg); color: var(--fg); font: 13px/1.4 var(--font-ui); overflow: hidden; }
.layout { display: grid; grid-template-columns: var(--left, 240px) 1fr var(--right, 280px); grid-template-rows: 1fr 22px; height: 100%; }
.pane-title { font-size: var(--font-ui-smaller); text-transform: uppercase; letter-spacing: .06em; color: var(--fg-muted); padding: 6px 10px 2px; }
.tree button { display: block; width: 100%; text-align: left; padding: var(--row-pad); border-radius: var(--radius); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tabs button { padding: 4px 10px; border-right: 1px solid var(--border); color: var(--fg-muted); white-space: nowrap; font-size: var(--font-ui-small); }
.statusbar { grid-column: 1 / -1; background: var(--bg-2); border-top: 1px solid var(--border); font-size: var(--font-ui-smaller); color: var(--fg-muted); padding: 0 10px; display: flex; gap: 14px; align-items: center; }
.linkrow { display: block; width: 100%; text-align: left; padding: 4px 10px; border-bottom: 1px solid var(--border); }
.linkrow .ctx { color: var(--fg-muted); font-size: var(--font-ui-smaller); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.props { padding: 4px 10px; display: grid; grid-template-columns: minmax(60px, 35%) minmax(0, 1fr) auto; gap: 3px 8px; font-size: var(--font-ui-small); align-items: center; }
.modes { display: flex; gap: 4px; padding: 3px 8px; border-bottom: 1px solid var(--border); font-size: var(--font-ui-smaller); }
.panestrip button { flex: 1; padding: 4px; color: var(--fg-muted); font-size: var(--font-ui-small); }
.menu button { display: block; width: 100%; text-align: left; padding: 4px 10px; }
.palette .detail { color: var(--fg-muted); font-size: var(--font-ui-small); }
```

- [ ] **Step 3: Verify against the previous density**

Run the dev server if it is not up (`cd ui && pnpm dev`), then take a shot of a
vault with a few folders and an open note:

```bash
cd ui && node scripts/shot.mjs scripts/fixture.example.json /tmp/density.png 1200 800
```

Expected: the explorer rows and tab strip are visibly tighter, the note's own
text is unchanged at 16px, and nothing clips or overlaps. Read the PNG. Take a
second shot with `"theme": "dark"` in the fixture's config.

- [ ] **Step 4: Commit**

```bash
git add ui/src/app.css
git commit -m "feat(ui): Obsidian's interface density"
```

---

### Task 2: Obsidian's graph style

**Files:**
- Modify: `ui/src/lib/graph.ts`, `ui/src/lib/graph.test.ts`, `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Changes `radius()` and `labelAlpha()` only; both stay pure and tested.
  `radius(n, s) = (2.5 + sqrt(inbound) * 1.5) * nodeSizeMultiplier`.
  `labelAlpha(scale, fade)` is 0 below 1.1 and reaches 1 by about 1.35.

- [ ] **Step 1: Write the failing tests**

Replace the existing `radius` and `labelAlpha` cases in
`ui/src/lib/graph.test.ts` with:

```ts
it("draws small nodes that grow slowly with links", () => {
  const n = (inbound: number) => ({ id: "a", title: "a", kind: "note" as const, tags: [], inbound });
  expect(radius(n(0), DEFAULTS)).toBeCloseTo(2.5);
  expect(radius(n(4), DEFAULTS)).toBeCloseTo(5.5);
  // A hub is bigger, not enormous.
  expect(radius(n(100), DEFAULTS)).toBeCloseTo(17.5);
});

it("keeps labels hidden until the view is zoomed in", () => {
  expect(labelAlpha(1.0, 0)).toBe(0);
  expect(labelAlpha(1.1, 0)).toBe(0);
  expect(labelAlpha(1.35, 0)).toBe(1);
  // The slider still moves the threshold.
  expect(labelAlpha(1.0, 3)).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd ui && pnpm test graph`
Expected: FAIL — the old radius returns 4 at inbound 0 and labels appear at 0.8.

- [ ] **Step 3: Make them pass**

In `ui/src/lib/graph.ts`:

```ts
/** Obsidian's nodes are small and grow slowly; a hub is bigger, not enormous. */
export function radius(n: ViewNode, s: GraphSettings): number {
  return (2.5 + Math.sqrt(n.inbound) * 1.5) * s.nodeSizeMultiplier;
}
```

```ts
/** Labels stay off until the view is zoomed in, as Obsidian's do. */
export function labelAlpha(scale: number, fade: number): number {
  return Math.min(1, Math.max(0, (scale - 1.1 + fade * 0.05) * 4));
}
```

- [ ] **Step 4: Pale the node colours**

In `ui/src/app.css`, in both the light `:root` and the two dark blocks, lighten
the graph tokens one step so nodes read as pale against the canvas:

```css
  --graph-note: #b3b9c2; --graph-line: #e2e5ea; --graph-tag: #86c39a; --graph-attachment: #dcc07a;
```

for light, and for both dark blocks:

```css
  --graph-note: #767d88; --graph-line: #33383f; --graph-tag: #4e8a62; --graph-attachment: #97803a;
```

- [ ] **Step 5: Run the tests and verify by eye**

Run: `cd ui && pnpm test graph`
Expected: PASS.

Then shoot the graph fixture from the last plan (a `graph` key with a dozen
notes) at the default zoom and again after zooming in:

```bash
cd ui && SETTLE=5500 node scripts/shot.mjs /tmp/graph.json /tmp/graph-far.png 1100 780
cd ui && SETTLE=5500 EVAL='(()=>{const c=document.querySelector("canvas");for(let i=0;i<8;i++)c.dispatchEvent(new WheelEvent("wheel",{deltaY:-120,clientX:550,clientY:390,bubbles:true}));return "zoomed"})()' node scripts/shot.mjs /tmp/graph.json /tmp/graph-near.png 1100 780
```

Expected: far view shows small pale dots and **no labels**; the zoomed view
shows labels. Read both PNGs.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/graph.ts ui/src/lib/graph.test.ts ui/src/app.css
git commit -m "feat(graph): Obsidian's small pale nodes and zoom-gated labels"
```

---

### Task 3: The pane owns the scroll container

**Files:**
- Modify: `ui/src/app.css`, `ui/src/components/Editor.svelte`
- Verify: `ui/scripts/shot.mjs`, and a 5 000-line note by hand

**Interfaces:**
- Produces: `.note` is the scrolling element for both editor and reading modes.
  CodeMirror runs `height: auto` with `.cm-scroller { overflow: visible }`, so
  anything rendered beside it inside `.note` scrolls with the document. Task 4
  puts the properties block there.
- No visible change on its own: this task is the surgery, checked for
  regressions only.

- [ ] **Step 1: Hand scrolling to `.note`**

In `ui/src/app.css`:

```css
/* The pane scrolls, not CodeMirror: the properties block has to scroll with
   the note, and it is a sibling of the editor rather than part of the document. */
.note { flex: 1; min-height: 0; overflow: auto; }
.cm-editor { height: auto; font-family: var(--font-text); font-size: 16px; }
.cm-editor .cm-scroller { overflow: visible; font-family: var(--font-text); }
```

Delete the old `.cm-editor { height: 100% }` and any `.note { overflow }` rule
that contradicts these; keep `.cm-editor .cm-content { max-width: 760px; margin:
0 auto; padding: 24px 32px; }`.

- [ ] **Step 2: Scroll the pane, not the editor, when jumping to a line**

CodeMirror's own `scrollIntoView` targets its scroller, which no longer
scrolls. In `ui/src/components/Editor.svelte`, replace the body of the jump
effect:

```svelte
  $effect(() => {
    if (!view || !jump) return;
    const doc = view.state.doc;
    const line = doc.line(Math.min(Math.max(jump.line, 1), doc.lines));
    view.dispatch({ selection: { anchor: line.from } });
    // The pane is the scroller now, so scroll the line's own element into it.
    const rect = view.coordsAtPos(line.from);
    const box = host.closest(".note");
    if (rect && box) {
      const top = rect.top - box.getBoundingClientRect().top + box.scrollTop;
      box.scrollTo({ top: Math.max(0, top - 24) });
    }
    view.focus();
    onJumped();
  });
```

- [ ] **Step 3: Verify nothing regressed**

Run the existing editor tests: `cd ui && pnpm test` — expected PASS.

Then check scrolling and jump-to-line in a rendered window. Build a fixture with
one note of 400 numbered lines and a `search` result pointing at line 300:

```bash
cd ui && EVAL='(()=>{const n=document.querySelector(".note");n.scrollTop=4000;return n.scrollTop+"/"+n.scrollHeight})()' node scripts/shot.mjs /tmp/long.json /tmp/long.png 1100 800
```

Expected: the EVAL prints a non-zero scrollTop against a tall scrollHeight —
proof `.note` is the scroller — and the PNG shows the note scrolled, the tab
strip and mode row still fixed above it.

- [ ] **Step 4: Measure what the change costs**

Write a 5 000-line note into the fixture and time the first paint:

```bash
cd ui && EVAL='(()=>{const t=performance.now();const n=document.querySelector(".note");n.scrollTop=n.scrollHeight;return "lines="+document.querySelectorAll(".cm-line").length+" t="+Math.round(performance.now()-t)})()' SETTLE=4000 node scripts/shot.mjs /tmp/huge.json /tmp/huge.png 1100 800
```

Record the reported line count in the commit message. CodeMirror renders the
viewport plus margin; if it reports all 5 000 lines, virtualisation is off and
that is worth knowing before the user meets it on a real note. If the window is
visibly sluggish, stop and report rather than pressing on.

- [ ] **Step 5: Commit**

```bash
git add ui/src/app.css ui/src/components/Editor.svelte
git commit -m "refactor(ui): the pane scrolls, so blocks can sit in the note's flow"
```

---

### Task 4: The properties block in the note

**Files:**
- Create: `ui/src/components/PropertiesBlock.svelte`
- Modify: `ui/src/components/NoteView.svelte`, `ui/src/editor/livePreview.ts`,
  `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `api.properties`, `api.setProperty`, `PropertyValue.svelte`,
  `lib/properties.parseValue`.
- Produces: `<PropertiesBlock path={string} />` — the frontmatter of `path` as
  Obsidian's Properties view: one row per key, a typed editor per value, an
  *Add property* row, and nothing at all when the note has no frontmatter and
  the user has not asked to add one.
- `livePreview` stops rendering its "Properties" fold placeholder: the
  frontmatter is replaced by a zero-height widget instead, because the block
  above it is now the real thing.

- [ ] **Step 1: Write the block**

Create `ui/src/components/PropertiesBlock.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { parseValue } from "../lib/properties";
  import PropertyValue from "./PropertyValue.svelte";

  let { path }: { path: string } = $props();
  let props = $state<Record<string, unknown>>({});
  let adding = $state(false);
  let newKey = $state("");

  $effect(() => {
    const p = path;
    void app.files; // re-read whenever the index changed
    properties(p).then((r) => (props = r));
  });

  // null removes the key.
  async function commit(key: string, value: unknown) {
    const doc = app.docs[path];
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would conflict with the rewrite
      await setProperty(path, key, value);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  function add(raw: string) {
    if (!newKey) return;
    void commit(newKey, parseValue(raw) ?? "");
    newKey = "";
    adding = false;
  }
</script>

<div class="propblock">
  {#each Object.entries(props) as [k, v] (k)}
    <div class="prow">
      <span class="key" title={k}>{k}</span>
      <PropertyValue value={v} onCommit={(x) => commit(k, x)} />
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    </div>
  {/each}
  {#if adding}
    <div class="prow">
      <!-- svelte-ignore a11y_autofocus -->
      <input class="key" placeholder="Property" autofocus bind:value={newKey} />
      <input placeholder="Value" onchange={(e) => add(e.currentTarget.value)} />
      <button class="remove" onclick={() => (adding = false)}>×</button>
    </div>
  {:else}
    <button class="addprop" onclick={() => (adding = true)}>+ Add property</button>
  {/if}
</div>
```

- [ ] **Step 2: Style it like Obsidian's**

Append to `ui/src/app.css`:

```css
/* Obsidian's Properties view: in the note's flow, the same column as its text. */
.propblock { max-width: 760px; margin: 0 auto; padding: 16px 32px 4px; font-size: var(--font-ui-small); }
.propblock .prow { display: grid; grid-template-columns: minmax(80px, 26%) minmax(0, 1fr) auto; gap: 2px 8px; align-items: center; padding: 1px 0; }
.propblock .key { color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.propblock input { border-color: transparent; background: transparent; padding: 2px 6px; }
.propblock .prow:hover input, .propblock input:focus { border-color: var(--border); background: var(--bg); }
.propblock .remove { color: transparent; padding: 0 4px; }
.propblock .prow:hover .remove { color: var(--fg-muted); }
.addprop { color: var(--fg-muted); padding: 3px 6px; font-size: var(--font-ui-small); }
.addprop:hover { color: var(--fg); }
```

- [ ] **Step 3: Put it in the note, above both views**

In `ui/src/components/NoteView.svelte`, import it and place it inside `.note`,
before the mode branch, so it scrolls with the content:

```svelte
  <div class="note">
    <PropertiesBlock {path} />
    {#if mode === "reading"}
```

Leave source mode alone: it shows the file as it is, frontmatter included, so
guard the block with `{#if mode !== "source"}`.

- [ ] **Step 4: Hide the frontmatter in live preview**

In `ui/src/editor/livePreview.ts`, `PropertiesFold` should no longer draw a
placeholder — the block above is the placeholder now. Replace its `toDOM`:

```ts
class PropertiesFold extends WidgetType {
  eq() { return true; }
  // The properties block above the editor shows them; here they only take space.
  toDOM() {
    const d = document.createElement("div");
    d.className = "cm-props-hidden";
    return d;
  }
  ignoreEvent() { return true; }
}
```

and in `ui/src/app.css` replace the `.cm-props-fold` rule with:

```css
.cm-props-hidden { height: 0; }
```

- [ ] **Step 5: Verify in a rendered window**

Fixture: a note with `title`, `tags` (a list), `done` (a checkbox) and a `due`
date, opened in live preview.

```bash
cd ui && node scripts/shot.mjs /tmp/props.json /tmp/props.png 1200 800
```

Expected: the properties sit above the note title in the same 760px column, one
row per key, chips for the list, a checkbox for the boolean, *+ Add property*
below them, and no raw `---` fences in the editor. Read the PNG, then check
three things by EVAL:

```bash
cd ui && EVAL='(()=>{document.querySelector(".addprop").click();return new Promise(r=>setTimeout(()=>r(!!document.querySelector(".propblock input.key")),200))})()' node scripts/shot.mjs /tmp/props.json /tmp/props-add.png 1200 800
```

Expected: `true`, and the PNG shows the new-property row. Then scroll `.note`
and confirm the block moves with the text (the point of Task 3):

```bash
cd ui && EVAL='(()=>{const n=document.querySelector(".note");n.scrollTop=600;const b=document.querySelector(".propblock").getBoundingClientRect();return "top="+Math.round(b.top)})()' node scripts/shot.mjs /tmp/props-long.json /tmp/props-scrolled.png 1200 800
```

Expected: a negative or small `top`, proving it scrolled away. Also shoot the
dark theme.

- [ ] **Step 6: Commit**

```bash
git add ui/src/components/PropertiesBlock.svelte ui/src/components/NoteView.svelte ui/src/editor/livePreview.ts ui/src/app.css
git commit -m "feat(ui): properties edited inline at the top of the note"
```

---

### Task 5: A history per tab

**Files:**
- Create: `ui/src/lib/history.ts`, `ui/src/lib/history.test.ts`
- Modify: `ui/src/lib/layout.ts`, `ui/src/lib/layout.test.ts`,
  `ui/src/lib/state.svelte.ts`

**Interfaces:**
- Consumes: `layout.TabRef`.
- Produces: `TabRef` gains `back: string[]` and `fwd: string[]`, both optional
  so a `workspace.json` from an earlier session still opens.
  `history.visit(tab, path)`, `history.back(tab)`, `history.forward(tab)`, each
  returning the tab's new path or `null` when there is nowhere to go; all pure.
  `app.openNote(path, line?, kind?, query?, newTab = false)` navigates the
  active tab unless `newTab`; `app.back(paneId)` and `app.forward(paneId)`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/history.test.ts`:

```ts
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
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd ui && pnpm test history`
Expected: FAIL — cannot resolve `./history`.

- [ ] **Step 3: Write it**

In `ui/src/lib/layout.ts`, extend the tab type:

```ts
export interface TabRef {
  path: string;
  mode: Mode;
  /** Where this tab has been, and where it came back from. Obsidian's per-tab history. */
  back?: string[];
  fwd?: string[];
}
```

Create `ui/src/lib/history.ts`:

```ts
import type { TabRef } from "./layout";

export const canBack = (t: TabRef) => (t.back?.length ?? 0) > 0;
export const canForward = (t: TabRef) => (t.fwd?.length ?? 0) > 0;

/** Navigate the tab to `path`, remembering where it was. */
export function visit(t: TabRef, path: string): void {
  if (t.path === path) return;
  t.back = [...(t.back ?? []), t.path];
  t.fwd = [];
  t.path = path;
}

export function back(t: TabRef): string | null {
  const from = t.back ?? [];
  if (from.length === 0) return null;
  t.fwd = [t.path, ...(t.fwd ?? [])];
  t.back = from.slice(0, -1);
  t.path = from[from.length - 1];
  return t.path;
}

export function forward(t: TabRef): string | null {
  const to = t.fwd ?? [];
  if (to.length === 0) return null;
  t.back = [...(t.back ?? []), t.path];
  t.fwd = to.slice(1);
  t.path = to[0];
  return t.path;
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ui && pnpm test history`
Expected: PASS, 4 tests.

- [ ] **Step 5: Navigate the current tab**

In `ui/src/lib/state.svelte.ts`, replace `openNote`'s body so it reuses the
active tab, and add the two moves:

```ts
  /** Opens `path`: in the active tab, as Obsidian does, unless `newTab`. */
  async openNote(path: string, line?: number, kind: api.EventKind = "open", query?: string, newTab = false) {
    const p = this.pane;
    const i = p.tabs.findIndex((t) => t.path === path);
    if (i >= 0) {
      this.activate(p.id, i);
    } else if (fileKind(path) !== "note" || newTab || p.active < 0) {
      // A graph or a base gets its own tab; so does an explicit new one.
      if (fileKind(path) === "note") await this.load(path);
      p.tabs.push({ path, mode: this.config?.editor.default_mode ?? "live" });
      p.active = p.tabs.length - 1;
      this.persist();
    } else {
      await this.load(path);
      H.visit(p.tabs[p.active], path);
      this.persist();
    }
    if (line) this.jump = { pane: p.id, path, line };
    void api.recordEvent(kind, path, query).catch(() => {});
  }

  async step(paneId: number, dir: "back" | "forward") {
    const p = L.findPane(this.layout, paneId);
    const t = p && p.active >= 0 ? p.tabs[p.active] : null;
    if (!t) return;
    const path = dir === "back" ? H.back(t) : H.forward(t);
    if (!path) return;
    try {
      if (fileKind(path) === "note") await this.load(path);
    } catch {
      return; // the note is gone; the history entry is stale
    }
    this.persist();
  }
```

with `import * as H from "./history";` at the top. A buffer released when its
tab closed is reloaded by `load`, so history survives a closed-and-reopened
note.

- [ ] **Step 6: Run every frontend test**

Run: `cd ui && pnpm check && pnpm test`
Expected: PASS. `layout.test.ts` may assert on tab objects; `back` and `fwd`
are optional, so equality checks that build tabs without them still hold.

- [ ] **Step 7: Commit**

```bash
git add ui/src/lib/history.ts ui/src/lib/history.test.ts ui/src/lib/layout.ts ui/src/lib/state.svelte.ts
git commit -m "feat(ui): a history per tab, and links navigate in place"
```

---

### Task 6: The tab header

**Files:**
- Create: `ui/src/components/TabHeader.svelte`
- Modify: `ui/src/components/Pane.svelte`, `ui/src/components/NoteView.svelte`,
  `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `history.canBack`, `history.canForward`, `app.step`, `app.setMode`.
- Produces: `<TabHeader pane={Pane} />` — back, forward, the note's title, and a
  reading/editing toggle on the right. It replaces the `.modes` row inside
  `NoteView`, so live/source/reading move behind one icon that alternates
  between the editing mode and reading.

- [ ] **Step 1: Write it**

Create `ui/src/components/TabHeader.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { canBack, canForward } from "../lib/history";
  import { tabTitle } from "../lib/files";
  import { fileKind } from "../lib/files";
  import type { Pane } from "../lib/layout";

  let { pane }: { pane: Pane } = $props();
  const tab = $derived(pane.active >= 0 ? pane.tabs[pane.active] : null);
  const note = $derived(tab && fileKind(tab.path) === "note" ? tab : null);
  // Obsidian's one icon: reading shows a pencil, editing shows a book.
  const reading = $derived(note?.mode === "reading");

  function toggle() {
    if (!note) return;
    app.setMode(pane.id, note.path, reading ? "live" : "reading");
  }
</script>

<div class="tabheader">
  <button class="icon" title="Back" disabled={!tab || !canBack(tab)} onclick={() => app.step(pane.id, "back")}>‹</button>
  <button class="icon" title="Forward" disabled={!tab || !canForward(tab)} onclick={() => app.step(pane.id, "forward")}>›</button>
  <span class="title">{tab ? tabTitle(tab.path) : ""}</span>
  {#if note}
    <button class="icon" title={reading ? "Edit" : "Read"} onclick={toggle}>{reading ? "✎" : "▤"}</button>
  {/if}
</div>
```

- [ ] **Step 2: Style it**

Append to `ui/src/app.css`:

```css
.tabheader { display: flex; align-items: center; gap: 2px; padding: 2px 6px; border-bottom: 1px solid var(--border); }
.tabheader .title { flex: 1; text-align: center; color: var(--fg-muted); font-size: var(--font-ui-small); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.icon { color: var(--fg-muted); padding: 2px 6px; border-radius: var(--radius); line-height: 1; }
.icon:hover:not(:disabled) { background: var(--bg-3); color: var(--fg); }
.icon:disabled { opacity: .35; cursor: default; }
```

- [ ] **Step 3: Put it under the tab strip and take the mode row out**

In `ui/src/components/Pane.svelte`, after `<Tabs {pane} />`:

```svelte
  <TabHeader {pane} />
```

In `ui/src/components/NoteView.svelte`, delete the whole `.modes` block and the
`modes` constant. Nothing is lost: `toggle-mode` (`Ctrl+E`) already switches
live preview and source, and `toggle-reading` switches reading, both from the
command palette.

- [ ] **Step 4: Verify**

Fixture: two notes, one open, with a link followed so history has a step.

```bash
cd ui && EVAL='(()=>{const b=[...document.querySelectorAll(".tabheader .icon")];return b.map(x=>x.title+":"+x.disabled).join(" ")})()' node scripts/shot.mjs /tmp/header.json /tmp/header.png 1100 700
```

Expected: `Back:true Forward:true Read:false` on a fresh tab, the title centred
in the header, and the PNG showing one row of chrome where there were two.
Then click a wikilink and re-check that Back reads `false` (enabled).

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/TabHeader.svelte ui/src/components/Pane.svelte ui/src/components/NoteView.svelte ui/src/app.css
git commit -m "feat(ui): a header per tab with back, forward and a reading toggle"
```

---

### Task 7: A new tab button

**Files:**
- Modify: `ui/src/components/Tabs.svelte`, `ui/src/lib/state.svelte.ts`,
  `ui/src/components/Explorer.svelte`, `ui/src/app.css`

**Interfaces:**
- Produces: `app.newTab()` opens an empty tab in the active pane; `+` at the end
  of the tab strip calls it; `Ctrl`-click in the explorer opens in a new tab
  rather than navigating the current one.

- [ ] **Step 1: Add the store method**

In `ui/src/lib/state.svelte.ts`:

```ts
  /** Obsidian's `+`: an empty tab, ready for the quick switcher. */
  newTab() {
    const p = this.pane;
    p.tabs.push({ path: "", mode: this.config?.editor.default_mode ?? "live" });
    p.active = p.tabs.length - 1;
    this.persist();
  }
```

An empty path renders the pane's existing *No note open* state; `Pane.svelte`
guards on `tab`, so give that guard `tab?.path` instead. `tabTitle("")` returns
an empty string, so give it a name in `ui/src/lib/files.ts`:

```ts
export function tabTitle(path: string): string {
  if (path === "") return "New tab";
  if (path === "graph:global") return "Graph view";
```

and add the case to `ui/src/lib/files.test.ts` if that file tests `tabTitle`.

- [ ] **Step 2: Add the button**

In `ui/src/components/Tabs.svelte`, after the `{#each}`:

```svelte
  <button class="newtab icon" title="New tab" onclick={() => app.newTab()}>+</button>
```

and in `ui/src/app.css`:

```css
.tabs { display: flex; align-items: stretch; }
.tabs .newtab { border-right: none; padding: 2px 8px; align-self: center; }
```

- [ ] **Step 3: Ctrl-click opens a tab**

In `ui/src/components/Explorer.svelte`, the file row's click handler takes the
event and passes the flag:

```svelte
  onclick={(e) => app.openNote(f.path, undefined, "open", undefined, e.ctrlKey || e.metaKey)}
```

- [ ] **Step 4: Verify**

```bash
cd ui && EVAL='(()=>{document.querySelector(".newtab").click();return new Promise(r=>setTimeout(()=>r(document.querySelectorAll(".tabs button").length),200))})()' node scripts/shot.mjs /tmp/header.json /tmp/tabs.png 1100 700
```

Expected: the count grows by one and the PNG shows an empty tab with *No note
open*. Check `pnpm check && pnpm test` too.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/Tabs.svelte ui/src/components/Explorer.svelte ui/src/lib/state.svelte.ts ui/src/app.css
git commit -m "feat(ui): a new tab button, and Ctrl-click opens in one"
```

---

### Task 8: The left ribbon

**Files:**
- Create: `ui/src/components/Ribbon.svelte`
- Modify: `ui/src/App.svelte`, `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `commands.defaults` by id — `new-note`, `switcher`, `search`,
  `graph`, `daily`, `palette`, and `settings` from Task 12.
- Produces: a fixed 40px column left of the sidebar, six icons at the top and
  the settings icon at the bottom. The `.layout` grid gains a first column.

- [ ] **Step 1: Write it**

Create `ui/src/components/Ribbon.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { errorMessage } from "../lib/api";

  // Obsidian's ribbon: the commands worth one click, in its order.
  const top = [
    ["new-note", "🖉", "New note"],
    ["switcher", "⌕", "Quick switcher"],
    ["search", "▤", "Search"],
    ["graph", "◍", "Graph view"],
    ["daily", "▦", "Daily note"],
    ["palette", "⌘", "Command palette"],
  ] as const;

  function run(id: string) {
    const c = allCommands().find((x) => x.id === id);
    if (!c) return;
    Promise.resolve(c.run()).catch((e) => app.say(errorMessage(e)));
  }
</script>

<nav class="ribbon">
  {#each top as [id, glyph, title] (id)}
    <button class="icon" {title} onclick={() => run(id)}>{glyph}</button>
  {/each}
  <span class="spacer"></span>
  <button class="icon" title="Settings" onclick={() => (app.settings = true)}>⚙</button>
</nav>
```

All six ids exist in `commands.ts` today — `new-note`, `switcher`, `search`,
`graph`, `daily`, `palette` — so `run` finds each of them.

- [ ] **Step 2: Style it and make room in the grid**

In `ui/src/app.css`:

```css
.layout { display: grid; grid-template-columns: 40px var(--left, 240px) 1fr var(--right, 280px); grid-template-rows: 1fr 22px; height: 100%; }
.ribbon { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 4px 0; background: var(--bg-2); border-right: 1px solid var(--border); }
.ribbon .icon { width: 30px; height: 30px; display: grid; place-items: center; font-size: 15px; }
.ribbon .spacer { flex: 1; }
```

- [ ] **Step 3: Put it in the layout**

In `ui/src/App.svelte`, import `Ribbon` and make it the first child of
`.layout`, before `<aside class="sidebar">`. Add `settings = $state(false)` to
the store for Task 12 to use; the button above already sets it.

- [ ] **Step 4: Verify**

```bash
cd ui && node scripts/shot.mjs scripts/fixture.example.json /tmp/ribbon.png 1200 800
```

Expected: a narrow icon column at the far left, six icons at the top and the
gear at the bottom, the explorer beside it and nothing else shifted or clipped.
Check the dark theme too, and click one icon by EVAL to confirm it runs:

```bash
cd ui && EVAL='(()=>{document.querySelector(".ribbon .icon[title=\"Graph view\"]").click();return new Promise(r=>setTimeout(()=>r(document.querySelectorAll(".tabs button").length),300))})()' node scripts/shot.mjs scripts/fixture.example.json /tmp/ribbon-graph.png 1200 800
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/Ribbon.svelte ui/src/App.svelte ui/src/app.css ui/src/lib/state.svelte.ts
git commit -m "feat(ui): the left icon ribbon"
```

---

### Task 9: Properties with counts

**Files:**
- Modify: `core/src/index/query.rs`, `src-tauri/src/commands.rs`,
  `src-tauri/src/lib.rs`, `ui/src/lib/api.ts`, `ui/scripts/shot.mjs`
- Test: inline in `core/src/index/query.rs`

**Interfaces:**
- Produces: `Index::property_counts() -> Vec<PropertyCount>` with
  `PropertyCount { key: String, count: i64 }`, ordered by count descending then
  key; the Tauri command `all_properties`; `api.allProperties()`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `core/src/index/query.rs`:

```rust
    #[test]
    fn property_counts_are_ordered_by_use() {
        let (_d, ix) = indexed(&[
            ("A.md", "---\nstatus: open\ntags: [x]\n---\na"),
            ("B.md", "---\nstatus: done\n---\nb"),
            ("C.md", "no frontmatter"),
        ]);
        let counts = ix.property_counts().unwrap();
        assert_eq!(counts[0].key, "status");
        assert_eq!(counts[0].count, 2);
        assert_eq!(counts[1].key, "tags");
        assert_eq!(counts[1].count, 1);
        assert_eq!(counts.len(), 2);
    }
```

`query.rs` has no test module yet, so this creates one. Put it at the end of
the file with its own fixture:

```rust
#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;

    fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for (path, text) in files {
            std::fs::write(d.path().join(path), text).unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    // the test from step 1 goes here
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p engram-notes-core property_counts`
Expected: FAIL — no method named `property_counts`.

- [ ] **Step 3: Write it**

In `core/src/index/query.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PropertyCount {
    pub key: String,
    pub count: i64,
}
```

```rust
    /// Every property key in the vault and how many notes carry it.
    pub fn property_counts(&self) -> Result<Vec<PropertyCount>> {
        Ok(self
            .conn
            .prepare(
                "SELECT key, count(*) FROM properties GROUP BY key
                 ORDER BY count(*) DESC, key",
            )?
            .query_map([], |r| {
                Ok(PropertyCount {
                    key: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
```

- [ ] **Step 4: Expose it**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn all_properties(state: State<AppState>) -> CmdResult<Vec<PropertyCount>> {
    with_open(&state, |o| Ok(o.index.property_counts()?))
}
```

with `PropertyCount` added to the `engram_core::index::query` import, and
`commands::all_properties` registered in `src-tauri/src/lib.rs`.

In `ui/src/lib/api.ts`:

```ts
export interface PropertyCount { key: string; count: number }
export const allProperties = () => invoke<PropertyCount[]>("all_properties");
```

In `ui/scripts/shot.mjs`, add the stub:

```js
        case "all_properties": return FX.allProperties ?? [];
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --workspace && cd ui && pnpm check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add core/src/index/query.rs src-tauri/src ui/src/lib/api.ts ui/scripts/shot.mjs
git commit -m "feat(index): every property key with the number of notes using it"
```

---

### Task 10: Switchable right sidebar views

**Files:**
- Create: `ui/src/components/AllProperties.svelte`
- Modify: `ui/src/App.svelte`, `ui/src/lib/state.svelte.ts`, `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Produces: `app.rightPane` — one of `"note" | "props" | "all" | "related"` —
  persisted in `workspace.json` alongside the layout, and an icon strip at the
  top of the right sidebar that switches between them.
- `"note"` shows Backlinks and Outgoing together, as Obsidian's linked mentions;
  `"props"` the current note's properties; `"all"` the new *All properties*;
  `"related"` the Related pane.

- [ ] **Step 1: Write the All properties view**

Create `ui/src/components/AllProperties.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allProperties, errorMessage, type PropertyCount } from "../lib/api";

  let rows = $state<PropertyCount[]>([]);
  $effect(() => {
    void app.files; // re-read whenever the index changed
    allProperties().then((r) => (rows = r)).catch((e) => app.say(errorMessage(e)));
  });
</script>

<div class="pane-title">All properties</div>
{#each rows as r (r.key)}
  <div class="countrow"><span>{r.key}</span><span class="n">{r.count}</span></div>
{:else}
  <div class="pane-note">No properties in this vault.</div>
{/each}
```

```css
.countrow { display: flex; justify-content: space-between; gap: 8px; padding: 2px 10px; font-size: var(--font-ui-small); }
.countrow:hover { background: var(--bg-3); }
.countrow .n { color: var(--fg-muted); font-variant-numeric: tabular-nums; }
```

- [ ] **Step 2: Add the switch to the store**

In `ui/src/lib/state.svelte.ts`:

```ts
  rightPane = $state<"note" | "props" | "all" | "related">("note");
```

Persist and restore it beside the layout: in `persist()` add `rightPane:
this.rightPane`, and in `restore(ws)` read it back when it is one of the four.

- [ ] **Step 3: Switch the sidebar**

In `ui/src/App.svelte`, replace the right `<aside>`'s contents:

```svelte
    <aside class="sidebar right">
      {#if app.showRight}
        <div class="panestrip">
          <button class:active={app.rightPane === "note"} title="Links" onclick={() => (app.rightPane = "note")}>⇄</button>
          <button class:active={app.rightPane === "props"} title="Properties" onclick={() => (app.rightPane = "props")}>▤</button>
          <button class:active={app.rightPane === "all"} title="All properties" onclick={() => (app.rightPane = "all")}>≣</button>
          <button class:active={app.rightPane === "related"} title="Related" onclick={() => (app.rightPane = "related")}>◍</button>
        </div>
        {#if app.rightPane === "note"}<Backlinks /><Outgoing />
        {:else if app.rightPane === "props"}<Properties />
        {:else if app.rightPane === "all"}<AllProperties />
        {:else}<Related />{/if}
      {/if}
    </aside>
```

- [ ] **Step 4: Verify**

Fixture with `backlinks`, `outgoing`, `properties`, `allProperties` and
`related` keys. Shoot each of the four views by clicking its icon:

```bash
cd ui && for i in 1 2 3 4; do EVAL="document.querySelectorAll('.sidebar.right .panestrip button')[$((i-1))].click()" node scripts/shot.mjs /tmp/right.json /tmp/right-$i.png 1200 800; done
```

Expected: four PNGs, each showing one view, the icon strip highlighting the
active one. Read all four.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/AllProperties.svelte ui/src/App.svelte ui/src/lib/state.svelte.ts ui/src/app.css
git commit -m "feat(ui): switchable right sidebar views with All properties"
```

---

### Task 11: A fuller status bar

**Files:**
- Modify: `ui/src/components/StatusBar.svelte`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `api.backlinks`, `api.properties`.
- Produces: backlinks, properties, words and characters for the active note,
  beside the note count, the embedding queue and the memory toggle. Counts are
  fetched when the note changes, not on every keystroke; words and characters
  come from the buffer and so update live.

- [ ] **Step 1: Add the counts**

In `ui/src/components/StatusBar.svelte`, above the markup:

```ts
  const chars = $derived(app.activeDoc?.text.length ?? 0);
  let links = $state(0);
  let propCount = $state(0);

  // Both are index reads: on the note and on the index, never on a keystroke.
  $effect(() => {
    const p = app.activeDoc?.path;
    void app.files;
    if (!p) {
      links = 0;
      propCount = 0;
      return;
    }
    backlinks(p).then((r) => (links = r.length)).catch(() => (links = 0));
    properties(p).then((r) => (propCount = Object.keys(r).length)).catch(() => (propCount = 0));
  });
```

with `backlinks` and `properties` added to the `../lib/api` import, and in the
markup, after the note count:

```svelte
  {#if app.activeDoc}
    <span>{links} backlinks</span>
    <span>{propCount} properties</span>
    <span>{words} words</span>
    <span>{chars} characters</span>
  {/if}
```

replacing the existing lone `{words} words`.

- [ ] **Step 2: Verify**

```bash
cd ui && EVAL='document.querySelector(".statusbar").textContent.replace(/\s+/g," ").trim()' node scripts/shot.mjs /tmp/right.json /tmp/status.png 1200 300
```

Expected: the EVAL prints something like
`☰ 2 notes 1 backlinks 3 properties 12 words 68 characters memory on ☰`, and the
bar is not crowded at 1200px. If it is, drop *characters* to a `title` on the
words item and say so.

- [ ] **Step 3: Commit**

```bash
git add ui/src/components/StatusBar.svelte
git commit -m "feat(ui): backlinks, properties, words and characters in the status bar"
```

---

### Task 12: The settings dialog

**Files:**
- Create: `ui/src/components/Settings.svelte`
- Modify: `ui/src/App.svelte`, `ui/src/lib/commands.ts`, `ui/src/app.css`
- Verify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `app.config`, `api.setConfig`, `api.setModelDir`,
  `api.forgetMemory`, `app.settings` (from Task 8).
- Produces: a modal over the existing `.scrim`, with four sections —
  Appearance, Editor, Daily notes, Search and memory — writing `app.json`
  through `set_config` on every change. A `settings` command so `Ctrl+,` opens
  it, which is Obsidian's shortcut.

- [ ] **Step 1: Write the dialog**

Create `ui/src/components/Settings.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, forgetMemory, setConfig, setModelDir } from "../lib/api";
  import { open } from "@tauri-apps/plugin-dialog";

  const cfg = $derived(app.config);

  // Every field writes app.json; the backend is the only copy that matters.
  async function save() {
    if (!cfg) return;
    try {
      await setConfig($state.snapshot(cfg));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function pickModel() {
    const dir = await open({ directory: true });
    if (typeof dir !== "string" || !cfg) return;
    cfg.embed.model_dir = dir;
    await setModelDir(dir);
  }
</script>

{#if cfg}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (app.settings = false)}>
    <div class="settings" onclick={(e) => e.stopPropagation()}>
      <div class="pane-title">Appearance</div>
      <label>Theme
        <select bind:value={cfg.theme} onchange={save}>
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>

      <div class="pane-title">Editor</div>
      <label>Default mode
        <select bind:value={cfg.editor.default_mode} onchange={save}>
          <option value="live">Live preview</option>
          <option value="source">Source</option>
          <option value="reading">Reading</option>
        </select>
      </label>

      <div class="pane-title">Daily notes</div>
      <label>Folder <input bind:value={cfg.daily_notes.folder} onchange={save} /></label>
      <label>Date format <input bind:value={cfg.daily_notes.format} onchange={save} /></label>

      <div class="pane-title">Search and memory</div>
      <label>Memory <input type="checkbox" bind:checked={cfg.memory.enabled} onchange={save} /></label>
      <label>Similarity floor
        <input type="number" min="0" max="1" step="0.01" bind:value={cfg.search.similarity_floor} onchange={save} />
      </label>
      <label>Model folder
        <button class="pick" onclick={pickModel}>{cfg.embed.model_dir ?? "Downloaded"}</button>
      </label>
      <label>Learned links
        <button class="pick" onclick={async () => { await forgetMemory(); app.say("Memory forgotten."); }}>Forget everything</button>
      </label>

      <div class="row"><button onclick={() => (app.settings = false)}>Close</button></div>
    </div>
  </div>
{/if}
```

- [ ] **Step 2: Style it**

```css
.settings { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: 0 8px 30px rgba(0,0,0,.3); width: 460px; max-height: 76vh; overflow: auto; padding: 4px 0 12px; }
.settings label { display: grid; grid-template-columns: 1fr auto; gap: 10px; align-items: center; padding: 3px 14px; font-size: var(--font-ui-small); }
.settings input[type="number"], .settings select, .settings input[type="text"], .settings input:not([type]) { width: 200px; }
.settings .pick { border: 1px solid var(--border); border-radius: var(--radius); padding: 2px 8px; max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.settings .row { display: flex; justify-content: flex-end; padding: 8px 14px 0; }
```

- [ ] **Step 3: Mount it and give it a shortcut**

In `ui/src/App.svelte`, import `Settings` and render `{#if app.settings}<Settings />{/if}` beside `<Palette />`. Extend the Escape branch of `onKey` to close it:

```ts
    if (e.key === "Escape") {
      app.palette = "none";
      app.settings = false;
      return;
    }
```

In `ui/src/lib/commands.ts`, add to `defaults`:

```ts
  { id: "settings", name: "Open settings", hotkey: "Ctrl+,", run: () => (app.settings = true) },
```

- [ ] **Step 4: Verify**

```bash
cd ui && EVAL='(()=>{document.querySelector(".ribbon .icon[title=\"Settings\"]").click();return new Promise(r=>setTimeout(()=>r(document.querySelectorAll(".settings label").length),300))})()' node scripts/shot.mjs scripts/fixture.example.json /tmp/settings.png 1200 800
```

Expected: eight labels, and the PNG shows the four sections with native controls
styled for the theme. **Shoot the dark theme too** — the handoff records that
WebKitGTK paints `<select>` and number spinners light in either theme, and this
dialog is full of both; the existing dark rules in `app.css` must cover them.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/Settings.svelte ui/src/App.svelte ui/src/lib/commands.ts ui/src/app.css
git commit -m "feat(ui): a settings dialog behind the ribbon's gear"
```

---

### Task 13: Docs and the smoke list

**Files:**
- Modify: `docs/smoke.md`

- [ ] **Step 1: Add the lines**

Append before the final `cargo test` line, renumbering it last:

```markdown
42. The properties of a note are edited in a block above its text and scroll
    away with it; *+ Add property* adds one and it lands in the frontmatter.
43. Back and forward in the tab header walk the notes that tab has shown;
    following a link navigates in place, `+` and Ctrl-click open a tab.
44. The reading toggle in the tab header switches the note and back.
45. The ribbon's six icons run their commands and the gear opens settings;
    changing the theme there takes effect at once and survives a restart.
46. The right sidebar switches between links, properties, all properties with
    counts, and Related; the choice survives a restart.
47. The status bar shows backlinks, properties, words and characters for the
    open note.
48. The graph's nodes are small and pale and show no labels until zoomed in.
```

- [ ] **Step 2: Run the whole suite**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test --workspace && cd ui && pnpm check && pnpm test
```

- [ ] **Step 3: Commit**

```bash
git add docs/smoke.md
git commit -m "docs: smoke lines for the Obsidian-parity UI"
```

---

## After the last task

1. Push and wait for CI.
2. `cd ui && pnpm tauri build --debug --no-bundle`, run
   `target/debug/engram-notes ~/engram-demo-vault`, and ask the user for
   screenshots: a note with properties, the tab header mid-history, the ribbon,
   each right-sidebar view, the settings dialog in the dark theme, and the graph
   before and after zooming.
3. Ask whether to merge locally, open a pull request, or keep the branch.
