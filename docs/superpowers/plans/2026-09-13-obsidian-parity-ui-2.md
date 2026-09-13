# Obsidian parity, second pass: the header band, typed properties and the explorer tree

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking.

**Goal:** Bring three parts of the interface to Obsidian's measured geometry —
a 40px header band that runs the whole width of the window, a properties block
headed *Properties* with a type icon per row, and an explorer with folder
chevrons and extension badges.

**Architecture:** Every number in this plan was measured out of Obsidian
1.13.7's own `app.css` and `app.js`, extracted from the installed flatpak's
`obsidian.asar` (see *Measurements* below). No backend change is needed: the
frontend's `propKind()` already mirrors `value_type()` in the index, so the
property type is derived where the value already is. The header band is built
by giving every region the same 40px top row and the same background rather
than by lifting the tab strip into the grid, which is how Obsidian does it and
which is what lets split panes keep their own strips with no special case.

**Tech Stack:** Svelte 5 (runes), TypeScript, vitest, `@lucide/svelte` 1.45,
plain CSS custom properties in `ui/src/app.css`.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`

**Branch:** `feat/obsidian-parity-ui`, already checked out, ten commits ahead of
`master`. Do not branch again.

---

## Global Constraints

From `AGENTS.md` and the spec, in force for every task:

- Rust 2024, `cargo fmt` and `cargo clippy -D warnings` clean. This plan
  touches no Rust, but CI runs both.
- `core` has no Tauri dependency; the frontend never touches the filesystem.
- KISS. One trait per seam, one implementation until a second exists.
- Where a feature is Obsidian-inspired, **match Obsidian's behaviour**. Match
  it by measuring, not by impression.
- Tests run without a model, a window or the network.
- Comments are short and say why, not what. One or two lines is the norm.
- Commit messages: conventional prefix, imperative subject under 72
  characters.
- `svelte-check` must stay clean: run `cd ui && pnpm check`.
- Frontend tests: `cd ui && pnpm test`. 65 pass today.
- Every UI change is verified with `node ui/scripts/shot.mjs <fixture>
  <out.png>` in **both** themes before it is called done.

---

## Measurements

Taken from Obsidian 1.13.7. Reproduce with:

```bash
python3 - <<'EOF'
import json, struct
p = "/var/lib/flatpak/app/md.obsidian.Obsidian/x86_64/stable/092bb11df3c993bd41aebf29b91228e3dc47ebb918c0224cea792006f123e084/files/resources/obsidian.asar"
f = open(p, 'rb'); jlen = struct.unpack('<4I', f.read(16))[3]
js = f.read(jlen).decode('utf-8', 'replace'); js = js[:js.rindex('}')+1]
tree = json.loads(js); base = f.tell()
for name, v in tree['files'].items():
    if name in ('app.css', 'app.js'):
        f.seek(base + int(v['offset'])); open('/tmp/' + name, 'wb').write(f.read(v['size']))
EOF
```

**The header band** (`app.css`)

| thing | value |
| --- | --- |
| `--header-height` | `40px` |
| `--ribbon-width` | `44px` |
| `.workspace-ribbon.mod-left` | `margin-top: var(--header-height)` — the ribbon starts **below** the band |
| `.workspace-ribbon.mod-left:before` | fills the top-left 44px corner with `--titlebar-background` |
| `.workspace-tab-header-container` | `height: 40px`, `border-bottom: 1px solid var(--tab-outline-color)`, `padding: 0 8px` — one per tab group, sidebars included |
| `.workspace-tabs.mod-top` | `--tab-container-background: var(--titlebar-background)` |
| `--tab-container-background` (elsewhere) | `var(--background-secondary)` |
| `.workspace-tab-header-container-inner` | `margin: 6px -5px -1px` |
| `.workspace-tab-header-inner` | `height: 100%`, `padding: 0 8px`, `gap: 2px`, `border-radius: var(--tab-radius)` |
| `--tab-radius-active` | `6px 6px 0 0` |
| `--tab-font-size` | `var(--font-ui-small)` = `13px` |
| `.view-header` | `height: 40px`, `gap: 8px`, `padding: 0 12px`, hidden in sidebars |
| `--file-header-font-size` | `13px`; `--file-header-justify: center` |

So Obsidian has **no** full-width tab-bar element. The band that spans the
window is three separate 40px rows — the ribbon's corner, each sidebar's own
header container, and the root split's tab strip — sharing one height and one
background. When the root splits, each group keeps its own strip and the band
stays continuous. That is the answer to "how do split panes keep their own
strips": they keep them because nothing is lifted out of `Pane.svelte`.

**Property types** (`app.js`, the widget registry)

| `propKind()` | key | Lucide icon |
| --- | --- | --- |
| `text` | | `text` |
| `number` | | `binary` |
| `date` | | `calendar` |
| `datetime` | | `clock` |
| `checkbox` | | `check-square` |
| `list` | | `list` |
| `list` or `text` | `tags` | `tags` |
| `list` or `text` | `aliases` | `forward` |

`tags` and `aliases` are key-driven in Obsidian
(`{aliases: {widget: "aliases"}, cssclasses: {widget: "multitext"}, tags:
{widget: "tags"}}`), not value-driven.

**The properties block** (`app.css`)

| thing | value |
| --- | --- |
| `--metadata-label-width` | `9em` |
| `--metadata-label-font-size` | `var(--font-smaller)` = `0.875em` |
| `--metadata-input-height` | `calc(var(--font-text-size) * 1.75)` = `28px` |
| `--metadata-gap` | `3px` |
| `--metadata-padding` | `8px 0`; `margin-block-end: 2rem` |
| `--metadata-property-radius` | `6px` |
| `.metadata-properties-title` | `font-size: max(13px, 1em)`, `font-weight: 500`, `color: var(--text-normal)` |
| `.metadata-properties-heading` | `padding: 4px`, `margin-bottom: 8px` |
| `.metadata-property-icon` | `height: var(--input-height)`, `padding: 4px 0`, `color: var(--icon-color)` |
| `--icon-color` / `--icon-opacity` | `var(--text-muted)` / `0.85` |

**The explorer** (`app.css` plus `app.js`)

| thing | value |
| --- | --- |
| `.tree-item-self` | `padding: 4px 8px 4px 24px`, `font-size: 13px`, `line-height: 1.3`, `border-radius: 4px`, `margin-bottom: 2px` |
| `.tree-item-self .tree-item-icon` | `position: absolute`, `margin-inline-start: -20px`, `width: 16px`, centred |
| `.collapse-icon svg` | `width: 10px`, `height: 10px`, `stroke-width: 4px`, `color: var(--text-faint)` |
| `.collapse-icon.is-collapsed svg` | `transform: rotate(-90deg)` |
| `.tree-item-children` | `padding-inline-start: 4px`, `margin-inline-start: 12px` — 16px per level |
| `.nav-file-tag` | `font-size: 9px`, `font-weight: 600`, `letter-spacing: 0.05em`, `text-transform: uppercase`, `color: var(--text-faint)`, `margin-inline-start: auto`, `padding: 0 4px` |

`.nav-file-icon` exists in the stylesheet but `app.js` never creates it: it is
dead CSS. What the explorer actually builds (constructor `ene`) is
`vb = ["md"]`, and any file whose extension is not in that list gets a
`.nav-file-tag` div containing the extension. **Markdown notes carry no icon.**
The user chose to match this exactly rather than add file icons.

---

## File structure

| file | change | responsibility after this plan |
| --- | --- | --- |
| `ui/src/lib/properties.ts` | modify | adds `propIcon(key, value)` beside `propKind` — the name of the Lucide icon for a property row |
| `ui/src/lib/properties.test.ts` | modify | covers `propIcon` |
| `ui/src/lib/tree.ts` | modify | adds `fileTag(path)` — the uppercase extension badge, or `null` for a note |
| `ui/src/lib/tree.test.ts` | modify | covers `fileTag` |
| `ui/src/components/PropertiesBlock.svelte` | modify | renders the *Properties* heading and a type icon per row |
| `ui/src/components/Properties.svelte` | modify | the sidebar view gets the same icons |
| `ui/src/components/Explorer.svelte` | modify | chevron component instead of `▸`/`▾`, extension badge, measured indent |
| `ui/src/app.css` | modify | `--header-height`, the band, the properties block, the tree |
| `ui/scripts/fixtures/parity2.json` | create | the fixture the three tasks are shot against |

No component is created or deleted. `Pane.svelte`, `Tabs.svelte`,
`Workspace.svelte` and `layout.ts` are **not** touched: the header band needs
no structural change, which is the point of the decision above.

---

### Task 1: The header band

Give the ribbon, both sidebars and every tab strip one 40px top row so the band
runs the whole window, as Obsidian's does. Split panes keep their own strips
because nothing moves out of `Pane.svelte`.

**Files:**
- Modify: `ui/src/app.css`
- Modify: `ui/src/components/Ribbon.svelte`
- Create: `ui/scripts/fixtures/parity2.json`
- Create: `ui/scripts/fixtures/parity2-dark.json`

**Interfaces:**
- Consumes: nothing.
- Produces: the CSS custom property `--header-height: 40px` on `:root`, used by
  Task 2 for nothing and by Task 3 for nothing — it is local to this task. The
  two fixtures are consumed by Tasks 2 and 3.

- [ ] **Step 1: Write the fixture the whole plan is shot against**

Create `ui/scripts/fixtures/parity2.json`. It carries a split layout (so the
band's behaviour under a split is visible), a folder tree two levels deep, a
non-markdown file for the extension badge, and a note with one property of
every type.

```json
{
  "root": "/vault",
  "config": {
    "editor": { "default_mode": "live" },
    "daily_notes": { "folder": "Daily", "template": null, "format": "%Y-%m-%d" },
    "hotkeys": {},
    "theme": "light",
    "search": { "limit": 20, "semantic": true, "similarity_floor": 0.83 },
    "memory": { "enabled": true },
    "embed": { "model_dir": null }
  },
  "folders": ["Rust", "Rust/Deep", "Kitchen"],
  "files": [
    { "path": "Rust/Borrowing.md", "text": "---\ntitle: Borrowing\ncount: 3\ndue: 2026-09-20\nstamped: 2026-09-20T10:30\ndone: true\ntags: [rust, memory]\naliases: [Borrow checker]\n---\n\n# Borrowing\n\nA reference borrows; see [[Rust/Deep/Lifetimes]].\n" },
    { "path": "Rust/Deep/Lifetimes.md", "text": "# Lifetimes\n\nThey outlive.\n" },
    { "path": "Kitchen/Sourdough.md", "text": "# Sourdough\n\nFlour, water, salt.\n" },
    { "path": "Kitchen/crumb.png", "size": 20481 },
    { "path": "Kitchen/Recipes.base", "text": "views:\n  - type: table\n    name: All\n" },
    { "path": "Index.md", "text": "# Index\n\nEverything starts here.\n" }
  ],
  "workspace": {
    "layout": {
      "kind": "split", "id": 3, "dir": "row", "sizes": [0.55, 0.45],
      "children": [
        { "kind": "pane", "id": 1, "tabs": [{ "path": "Rust/Borrowing.md", "mode": "live" }, { "path": "Index.md", "mode": "live" }], "active": 0 },
        { "kind": "pane", "id": 2, "tabs": [{ "path": "Kitchen/Sourdough.md", "mode": "live" }], "active": 0 }
      ]
    },
    "activePane": 1,
    "rightPane": "props"
  }
}
```

Then create `ui/scripts/fixtures/parity2-dark.json` as the same file with
`"theme": "dark"`:

```bash
cd /home/user01/Projekte/engram-notes
mkdir -p ui/scripts/fixtures
# write parity2.json as above, then:
sed 's/"theme": "light"/"theme": "dark"/' ui/scripts/fixtures/parity2.json > ui/scripts/fixtures/parity2-dark.json
```

- [ ] **Step 2: Shoot the current state, to have a before**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm dev &
sleep 4
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/before-light.png 1400 900
```

Read the PNG. Expected: the ribbon runs from the very top to the bottom, the
tab strip sits at 34px and does not line up with the sidebar's Files/Search
strip, and the top edge of the window is three different heights.

- [ ] **Step 3: Add the band's variables and geometry to `app.css`**

In the `:root` block, replace the line

```css
  --tab-height: 34px;
```

with Obsidian's measured names and values:

```css
  /* Obsidian's header band: one 40px row across the window, every region in it. */
  --header-height: 40px;
  --ribbon-width: 44px;
  --tab-height: var(--header-height);
```

- [ ] **Step 4: Push the ribbon below the band and fill its corner**

Replace the `.ribbon` rule (near the end of `app.css`) with:

```css
/* The ribbon starts under the band; its corner carries the band's colour. */
.ribbon { position: relative; margin-top: var(--header-height); display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 4px 0; background: var(--bg-2); border-right: 1px solid var(--border); }
.ribbon::before { content: ""; position: absolute; left: 0; top: calc(var(--header-height) * -1); width: 100%; height: var(--header-height); background: var(--bg-2); border-bottom: 1px solid var(--border); box-sizing: border-box; }
.ribbon .icon { width: 30px; height: 30px; display: grid; place-items: center; font-size: 15px; }
.ribbon .spacer { flex: 1; }
```

The grid still gives the ribbon column its full height; `margin-top` moves the
buttons down and the `::before` paints the corner it vacated.

- [ ] **Step 5: Give the tab strip, the sidebar strip and the breadcrumb the band's height**

Replace the three rules in `app.css`. The tab strip — delete the first, stale
`.tabs` rule (the one with `overflow-x: auto` and `border-bottom`) and keep one:

```css
.tabs { display: flex; height: var(--header-height); align-items: center; gap: 2px; padding: 0 8px; overflow-x: auto; background: var(--bg-2); border-bottom: 1px solid var(--border); }
.tabs button { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 8px; border-radius: var(--radius); color: var(--fg-muted); white-space: nowrap; font-size: var(--font-ui-small); }
```

The sidebar strip, so it meets the tabs at the same baseline:

```css
.panestrip { display: flex; height: var(--header-height); align-items: stretch; background: var(--bg-2); border-bottom: 1px solid var(--border); padding: 0 8px; }
.panestrip button { flex: 1; display: grid; place-items: center; color: var(--fg-muted); font-size: var(--font-ui-small); }
```

And the breadcrumb row, Obsidian's `.view-header`, which is a second 40px row
*beneath* the band and only in the centre:

```css
.tabheader { display: flex; align-items: center; gap: 8px; height: var(--header-height); padding: 0 12px; border-bottom: 1px solid var(--border); background: var(--bg); }
```

- [ ] **Step 6: Check and shoot, light**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/band-light.png 1400 900
```

Read the PNG. Expected: one unbroken 40px band across the whole window; the
ribbon icons begin beneath it; both split panes have their own tab strip
sitting in the band; the breadcrumb row runs under the centre only.

- [ ] **Step 7: Shoot dark**

```bash
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/band-dark.png 1400 900
```

Read the PNG. Expected: the same band, no light seam where the ribbon corner
meets the sidebar strip.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/app.css ui/src/components/Ribbon.svelte ui/scripts/fixtures/
git commit -m "feat(ui): Obsidian's 40px header band across the window"
```

---

### Task 2: Properties, labelled and typed

Head the block *Properties* and put Obsidian's own icon on each row, in both
the in-note block and the sidebar view.

**Files:**
- Modify: `ui/src/lib/properties.ts`
- Modify: `ui/src/lib/properties.test.ts`
- Modify: `ui/src/components/PropertiesBlock.svelte`
- Modify: `ui/src/components/Properties.svelte`
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: `propKind(v: unknown): PropKind` from `ui/src/lib/properties.ts`,
  already exported, returning `"checkbox" | "number" | "date" | "datetime" |
  "list" | "text"`.
- Produces: `propIcon(key: string, value: unknown): PropIcon`, where
  `type PropIcon = "text" | "binary" | "calendar" | "clock" | "check-square" |
  "list" | "tags" | "forward"` — the Lucide icon name for a property row.

- [ ] **Step 1: Write the failing test**

Append to `ui/src/lib/properties.test.ts`, inside the existing `describe`:

```ts
  it("names Obsidian's icon for a property row", () => {
    expect(propIcon("title", "hello")).toBe("text");
    expect(propIcon("count", 3)).toBe("binary");
    expect(propIcon("due", "2026-09-20")).toBe("calendar");
    expect(propIcon("stamped", "2026-09-20T10:30")).toBe("clock");
    expect(propIcon("done", true)).toBe("check-square");
    expect(propIcon("authors", ["a", "b"])).toBe("list");
  });

  it("reads tags and aliases off the key, as Obsidian's widgets do", () => {
    expect(propIcon("tags", ["rust"])).toBe("tags");
    expect(propIcon("tags", "rust")).toBe("tags");
    expect(propIcon("aliases", ["Borrow checker"])).toBe("forward");
    expect(propIcon("cssclasses", ["wide"])).toBe("list");
  });
```

and extend the import at the top of the file:

```ts
import { addItem, parseValue, propIcon, propKind, removeItem } from "./properties";
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test properties
```

Expected: FAIL — `propIcon is not a function`, or a TypeScript error that
`"./properties"` has no exported member `propIcon`.

- [ ] **Step 3: Write the implementation**

Append to `ui/src/lib/properties.ts`:

```ts
export type PropIcon =
  | "text" | "binary" | "calendar" | "clock" | "check-square" | "list" | "tags" | "forward";

const BY_KIND: Record<PropKind, PropIcon> = {
  text: "text",
  number: "binary",
  date: "calendar",
  datetime: "clock",
  checkbox: "check-square",
  list: "list",
};

// Obsidian's widget registry keys these two by name, whatever the value holds.
const BY_KEY: Record<string, PropIcon> = { tags: "tags", aliases: "forward" };

/** The Lucide icon Obsidian puts on a property row. */
export function propIcon(key: string, value: unknown): PropIcon {
  return BY_KEY[key] ?? BY_KIND[propKind(value)];
}
```

- [ ] **Step 4: Run the tests and watch them pass**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test properties
```

Expected: PASS, and the two pre-existing `properties` tests still pass.

- [ ] **Step 5: Put the heading and the icons in the in-note block**

In `ui/src/components/PropertiesBlock.svelte`, extend the imports:

```ts
  import { parseValue, propIcon } from "../lib/properties";
  import Text from "@lucide/svelte/icons/text";
  import Binary from "@lucide/svelte/icons/binary";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Clock from "@lucide/svelte/icons/clock";
  import CheckSquare from "@lucide/svelte/icons/check-square";
  import ListIcon from "@lucide/svelte/icons/list";
  import Tags from "@lucide/svelte/icons/tags";
  import Forward from "@lucide/svelte/icons/forward";
  import type { PropIcon } from "../lib/properties";
```

and add, after the `let newKey` declaration:

```ts
  const ICONS: Record<PropIcon, typeof Text> = {
    text: Text, binary: Binary, calendar: Calendar, clock: Clock,
    "check-square": CheckSquare, list: ListIcon, tags: Tags, forward: Forward,
  };
```

Then replace the markup's property row so it carries the icon, and put the
heading above the rows. The `.propblock` opening becomes:

```svelte
<div class="propblock">
  <div class="propheading">Properties</div>
  {#each Object.entries(values) as [k, v] (k)}
    {@const Icon = ICONS[propIcon(k, v)]}
    <div class="prow">
      <span class="key" title={k}><Icon size={16} strokeWidth={2} /><span class="kname">{k}</span></span>
      <PropertyValue value={v} onCommit={(x) => commit(k, x)} />
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    </div>
  {/each}
```

Leave the `{#if adding}` block and everything after it exactly as it is.

- [ ] **Step 6: Give the sidebar view the same icons**

In `ui/src/components/Properties.svelte`, extend the import line to

```ts
  import { parseValue, propIcon } from "../lib/properties";
```

add the same eight icon imports and the same `ICONS` map (repeat it; the two
components are small and a shared module for one constant is not worth a
seam), and replace the `{#each}` body with:

```svelte
    {#each Object.entries(props) as [k, v] (k)}
      {@const Icon = ICONS[propIcon(k, v)]}
      <span class="key" title={k}><Icon size={15} strokeWidth={2} /><span class="kname">{k}</span></span>
      <PropertyValue value={v} onCommit={(x) => commit(k, x)} />
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    {/each}
```

- [ ] **Step 7: Style them to Obsidian's measurements**

In `ui/src/app.css`, replace the `.propblock` group (the block under the
comment `/* Obsidian's Properties view: ... */`) with:

```css
/* Obsidian's Properties view: in the note's flow, the same column as its text. */
.propblock { max-width: 760px; margin: 0 auto; padding: 8px 32px 2rem; font-size: var(--font-ui-small); }
.propheading { font-size: 16px; font-weight: 500; color: var(--fg); padding: 4px; margin-bottom: 8px; }
.propblock .prow { display: grid; grid-template-columns: 9em minmax(0, 1fr) auto; gap: 0 8px; align-items: center; min-height: 28px; margin-bottom: 3px; border-radius: 6px; }
.propblock .key { display: flex; align-items: center; gap: 6px; color: var(--fg-muted); min-width: 0; }
.propblock .key > svg { flex: none; opacity: .85; }
.propblock .kname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.propblock input { border-color: transparent; background: transparent; padding: 2px 6px; }
.propblock .prow:hover input, .propblock input:focus { border-color: var(--border); background: var(--bg); }
.propblock .remove { color: transparent; padding: 0 4px; }
.propblock .prow:hover .remove { color: var(--fg-muted); }
```

and replace the `.props .key` rule (the sidebar view) with:

```css
.props .key { display: flex; align-items: center; gap: 6px; color: var(--fg-muted); min-width: 0; }
.props .key > svg { flex: none; opacity: .85; }
.props .kname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
```

- [ ] **Step 8: Check, test and shoot both themes**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/props-light.png 1400 900
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/props-dark.png 1400 900
```

Read both PNGs. Expected, in `Rust/Borrowing.md`: a *Properties* heading at
16px above the rows; `title` with a text icon, `count` with `binary`, `due`
with a calendar, `stamped` with a clock, `done` with a check square, `tags`
with the tags icon and `aliases` with `forward`; the same icons at 15px in the
right sidebar, whose *Properties* view is open in the fixture.

- [ ] **Step 9: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/lib/properties.ts ui/src/lib/properties.test.ts ui/src/components/PropertiesBlock.svelte ui/src/components/Properties.svelte ui/src/app.css
git commit -m "feat(ui): the Properties heading and Obsidian's type icons"
```

---

### Task 3: Explorer chevrons and extension badges

Replace the `▸`/`▾` text with a Lucide chevron at Obsidian's geometry, and give
non-markdown files the uppercase extension badge Obsidian shows instead of an
icon.

**Files:**
- Modify: `ui/src/lib/tree.ts`
- Modify: `ui/src/lib/tree.test.ts`
- Modify: `ui/src/components/Explorer.svelte`
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: `TreeDir`, `TreeFile`, `basename`, `parent` from
  `ui/src/lib/tree.ts`; `buildTree` already strips `.md` from a note's `name`
  but leaves `path` intact, so the badge is derived from `path`.
- Produces: `fileTag(path: string): string | null` — the uppercase extension
  badge, or `null` for a markdown note.

- [ ] **Step 1: Write the failing test**

Append to `ui/src/lib/tree.test.ts`, inside the existing `describe`:

```ts
  it("badges a file with its extension, and a note with nothing", () => {
    expect(fileTag("Kitchen/crumb.png")).toBe("PNG");
    expect(fileTag("Kitchen/Recipes.base")).toBe("BASE");
    expect(fileTag("a/b/paper.PDF")).toBe("PDF");
    expect(fileTag("Index.md")).toBeNull();
    expect(fileTag("Index.MD")).toBeNull();
    expect(fileTag("LICENSE")).toBeNull();
  });
```

and extend the import at the top of the file to include `fileTag`. Check the
existing import line first and add the name to it in alphabetical position.

- [ ] **Step 2: Run it and watch it fail**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test tree
```

Expected: FAIL — `fileTag is not a function`, or a TypeScript error that
`"./tree"` has no exported member `fileTag`.

- [ ] **Step 3: Write the implementation**

Append to `ui/src/lib/tree.ts`:

```ts
/** Obsidian badges every file that is not markdown with its extension, and shows no icon at all. */
export function fileTag(path: string): string | null {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = name.slice(dot + 1);
  return ext.toLowerCase() === "md" ? null : ext.toUpperCase();
}
```

A name with no dot, or one that starts with a dot, has no extension — hence
`dot <= 0`.

- [ ] **Step 4: Run the tests and watch them pass**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm test tree
```

Expected: PASS, and the pre-existing `tree` tests still pass.

- [ ] **Step 5: Put the chevron and the badge in the explorer**

In `ui/src/components/Explorer.svelte`, extend the `tree` import to carry
`fileTag` and add the chevron icon:

```ts
  import { buildTree, dropTarget, fileTag, freeName, parent, renameTarget, type TreeDir } from "../lib/tree";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
```

Replace the folder button's content — the line reading
`{collapsed[d.path] ? "▸" : "▾"} {d.name}` — with:

```svelte
        <span class="chev" class:collapsed={collapsed[d.path]}><ChevronDown size={10} strokeWidth={4} /></span>
        <span class="tname">{d.name}</span>
```

and replace the file button's content — the bare `{f.name}` — with:

```svelte
        <span class="tname">{f.name}</span>
        {#if fileTag(f.path)}<span class="ftag">{fileTag(f.path)}</span>{/if}
```

- [ ] **Step 6: Style the tree to Obsidian's measurements**

In `ui/src/app.css`, replace the three `.tree button` rules with:

```css
.tree button { position: relative; display: flex; align-items: center; width: 100%; text-align: left; font-size: var(--font-ui-small); line-height: 1.3; padding: 4px 8px 4px 24px; margin-bottom: 2px; border-radius: 4px; white-space: nowrap; }
.tree button:hover { background: var(--bg-3); }
.tree button.active { background: var(--accent-bg); }
.tree .tname { overflow: hidden; text-overflow: ellipsis; }
/* The chevron sits in the row's 24px gutter, as Obsidian's absolute one does. */
.tree .chev { position: absolute; left: 4px; display: grid; place-items: center; width: 16px; color: var(--fg-muted); opacity: .6; transition: transform 100ms ease-in-out; }
.tree .chev.collapsed { transform: rotate(-90deg); }
.tree .ftag { margin-left: auto; padding: 0 4px; font-size: 9px; font-weight: 600; letter-spacing: .05em; color: var(--fg-muted); opacity: .7; }
```

The per-level indent already comes from the inline
`padding-left: {8 + depth * 12}px` and `{20 + depth * 12}px` in the component;
those two inline styles now fight the 24px rule. Change them so the gutter is
constant and the depth is added to it: in the folder button replace

```svelte
        style="padding-left:{8 + depth * 12}px"
```

with

```svelte
        style="padding-left:{24 + depth * 16}px"
```

and in the file button replace

```svelte
        style="padding-left:{20 + depth * 12}px"
```

with

```svelte
        style="padding-left:{24 + depth * 16}px"
```

so files and folders share one text column at each depth, 16px per level as
Obsidian's `padding-inline-start: 4px` plus `margin-inline-start: 12px` gives.
Also move the chevron with the depth: change the `.chev` rule's `left` to a
custom property and set it inline. Replace the `.chev` rule with

```css
.tree .chev { position: absolute; left: var(--chev-left, 4px); display: grid; place-items: center; width: 16px; color: var(--fg-muted); opacity: .6; transition: transform 100ms ease-in-out; }
```

and make the folder button's style

```svelte
        style="padding-left:{24 + depth * 16}px;--chev-left:{4 + depth * 16}px"
```

Finally, the two `renameBox` calls pass an indent that must match. Change

```svelte
      {@render renameBox(d.path, true, d.name, 8 + depth * 12)}
```

to

```svelte
      {@render renameBox(d.path, true, d.name, 24 + depth * 16)}
```

and

```svelte
      {@render renameBox(f.path, false, f.name, 20 + depth * 12)}
```

to

```svelte
      {@render renameBox(f.path, false, f.name, 24 + depth * 16)}
```

- [ ] **Step 7: Check, test and shoot both themes**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm check && pnpm test
cd /home/user01/Projekte/engram-notes
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/tree-light.png 1400 900
node ui/scripts/shot.mjs ui/scripts/fixtures/parity2-dark.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/tree-dark.png 1400 900
```

Read both PNGs. Expected: `Rust` and `Kitchen` carry a small chevron pointing
down, `Rust/Deep` is indented 16px further with its own chevron at the matching
offset, note names sit in one column with their folders' names, `crumb.png`
shows a faint `PNG` at the right edge and `Recipes.base` a faint `BASE`, and
`Sourdough`, `Lifetimes`, `Borrowing` and `Index` show nothing but their names.

- [ ] **Step 8: Shoot a collapsed folder**

```bash
EVAL='document.querySelectorAll(".tree button.folder")[1].click()' \
  node ui/scripts/shot.mjs ui/scripts/fixtures/parity2.json /tmp/claude-1000/-home-user01-Projekte-engram-notes/0ed8339b-7e3f-402a-a75a-e5e60b471356/scratchpad/tree-collapsed.png 1400 900
```

Read the PNG. Expected: that folder's chevron now points right and its children
are gone.

- [ ] **Step 9: Commit**

```bash
cd /home/user01/Projekte/engram-notes
git add ui/src/lib/tree.ts ui/src/lib/tree.test.ts ui/src/components/Explorer.svelte ui/src/app.css
git commit -m "feat(ui): explorer chevrons and Obsidian's extension badges"
```

---

### Task 4: The real window

The headless check caught neither of last session's two real defects. Build the
window and put it in front of the user.

**Files:** none.

**Interfaces:** none.

- [ ] **Step 1: Confirm the build dependencies**

```bash
pkg-config --modversion glib-2.0
```

Expected: a version prints — `2.88.3` last session. If it fails, install
`webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel
libappindicator-gtk3-devel dbus-devel glib2-devel`.

- [ ] **Step 2: Run the whole suite**

```bash
cd /home/user01/Projekte/engram-notes
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd ui && pnpm check && pnpm test
```

Expected: all clean, 122 Rust tests and at least 68 frontend tests pass.

- [ ] **Step 3: Build the real window**

```bash
cd /home/user01/Projekte/engram-notes/ui && pnpm tauri build --debug --no-bundle
ls -l /home/user01/Projekte/engram-notes/target/debug/engram-notes
```

Expected: the binary's timestamp is from this minute. The Tauri CLI exits 0
even when it failed if its output is piped, so check the timestamp, not the
exit code.

- [ ] **Step 4: Ask the user to run it and send screenshots**

Tell the user:

> `target/debug/engram-notes ~/engram-demo-vault` — I need screenshots of the
> top of the window with the sidebars open, a note with properties, and the
> explorer with a folder collapsed, in both themes.

Wait for them. The user's KDE Wayland session cannot take desktop screenshots
from the shell, so this step is theirs.

- [ ] **Step 5: Fix what the screenshots show, then push**

```bash
cd /home/user01/Projekte/engram-notes
git push
```

---

## Self-review

**Spec coverage.** The spec's *The editor and UI* section asks for a left
sidebar with a file explorer and search, a centre with tabs and splits, a right
sidebar with backlinks, outgoing links, properties and *Related*, and themes on
CSS custom properties with stable names. All four already exist; this plan
changes only their measurements and adds icons, and it keeps every existing
custom property name while adding `--header-height` and `--ribbon-width`, both
Obsidian's own names. **No departure from the spec.** The one departure from
what the user originally asked for — no file icons on notes — was put to them
before the plan was written and they chose to match Obsidian.

**Placeholders.** None: every step carries the code or the command it needs,
and every measurement is a number taken from Obsidian rather than a judgement.

**Type consistency.** `propIcon(key, value)` returns `PropIcon`, whose eight
members are exactly the eight keys of `ICONS` in both components and exactly
the eight values in `BY_KIND` and `BY_KEY`. `PropKind`'s six members are
exactly `BY_KIND`'s six keys, so the `Record<PropKind, PropIcon>` is total and
`propIcon` cannot return `undefined`. `fileTag` returns `string | null` and is
used only in an `{#if}` and as text.
