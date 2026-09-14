# The editor's floor (0.2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the editor the list and bracket behaviour a markdown outliner
needs — wrapping a selection in brackets, continuing lists on Enter, indenting
and moving an item with its subtree, renumbering, folding with a gutter
chevron, and remembering folds per note — without changing the file format.

**Architecture:** All outline logic is a pure, line-array module in
`ui/src/lib/outline.ts`, tested with vitest and free of CodeMirror. A thin
CodeMirror layer in `ui/src/editor/outline.ts` turns those functions into
state commands, adds the fold rule and the fold-key mapping, and is tested at
the `EditorState` level without a DOM. `Editor.svelte` only wires extensions.
Fold state is cosmetic and per machine, so it lives in the derived index as a
`folds` table reached through two Tauri commands; a rename carries it exactly
as `move_memory` carries activation.

**Tech Stack:** Svelte 5 (runes), TypeScript, vitest, CodeMirror 6
(`@codemirror/autocomplete`, `@codemirror/lang-markdown`,
`@codemirror/language`, `@codemirror/commands`, `@codemirror/view`), Rust
2024 with `rusqlite`, Tauri 2.

**Spec:** `docs/superpowers/specs/2026-09-14-writing-first-direction-design.md`,
sections *The editor's floor* and *Order of work → 0.2*. The first design,
`docs/superpowers/specs/2026-09-12-engram-notes-design.md`, still governs
everything this plan does not touch.

**Branch:** `master` is clean at `15cec7a`. Create `feat/editor-floor` from
it in Task 1 and stay on it.

---

## Global Constraints

- Rust 2024 edition, stable toolchain, `cargo fmt` and `cargo clippy
  --workspace -- -D warnings` clean before every commit.
- `core` has no Tauri dependency; the frontend never touches the filesystem.
- Files are the truth. Fold state is never written into a note. A schema
  change bumps `SCHEMA_VERSION` and the index rebuilds; nothing migrates.
- "Every outline operation is an ordinary markdown list operation." No
  syntax of our own, no forced `- `, no metadata in the file.
- Indentation the editor writes is spaces, `editor.indent` of them (default
  `2`). Leading tabs on lines an operation touches are normalised to spaces;
  lines it does not touch are left alone.
- Spec's `Tab` precedence: an open completion popup takes it; otherwise, inside
  a list item, the outline takes it; otherwise it inserts indentation.
- No zoom (spec: *Non-goals*).
- Tests run without a model, a window or the network. Anything importing
  `@tauri-apps/api` stays out of tested modules.
- Comments short, saying why. Conventional commit subjects under 72 characters.
- `cd ui && pnpm check && pnpm test` and `cargo test --workspace` clean before
  every commit.
- Every commit message ends with these two lines:
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL`.
- **Not measured, and the user must be told:** Obsidian's flatpak is not on
  this machine, so the fold gutter's width, chevron and hover behaviour in
  Task 8 are a considered guess, not a measurement. Record that in
  `docs/memory.md` (Task 10) and ask the user for screenshots beside Obsidian.

---

## File structure

| file | responsibility |
| --- | --- |
| `ui/src/lib/outline.ts` (new) | Pure line-array model of a markdown list: parse an item, find a subtree and its siblings, indent, outdent, move, renumber, and key every item by outline path. No CodeMirror. |
| `ui/src/lib/outline.test.ts` (new) | Its tests. |
| `ui/src/editor/outline.ts` (new) | CodeMirror commands over `lib/outline`, the keymap with `Tab` precedence, the fold rule (`listOnlyFolding`), the fold gutter, and `foldKeys` / `foldTransaction`. |
| `ui/src/editor/outline.test.ts` (new) | State-level tests: bracket wrapping, Enter, Tab, Alt-arrows, folding, fold keys. |
| `ui/src/editor/theme.ts` | Gutter shown, only the fold gutter, chevron styling. |
| `ui/src/components/Editor.svelte` | Extension wiring: `closeBrackets`, `markdownKeymap`, the outline keymap, `indentUnit`, folding, fold restore and fold reporting. |
| `ui/src/components/NoteView.svelte` | Loads a note's fold keys, passes `indent`, saves fold keys debounced. |
| `ui/src/components/Settings.svelte` | *Indent width* field. |
| `ui/src/lib/api.ts` | `editor.indent` in `AppConfig`; `getFolds`, `setFolds`. |
| `ui/src/app.css` | The editor column centres as a whole so the gutter sits in its left inset. |
| `ui/scripts/shot.mjs`, `ui/scripts/fixtures/outline.json`, `outline-dark.json` | Screenshot fixture with a nested, ordered and task list, one item folded. |
| `core/src/config.rs` | `EditorConfig.indent`, default 2. |
| `core/src/index/schema.sql`, `core/src/index/mod.rs` | `folds` table; `SCHEMA_VERSION` 3 → 4. |
| `core/src/index/folds.rs` (new) | `folds`, `set_folds`, `move_folds` on `Index`, with tests. |
| `core/src/rename.rs` | A rename moves fold rows with memory rows. |
| `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` | `get_folds`, `set_folds`. |
| `docs/smoke.md`, `docs/memory.md` | Five smoke lines; the unmeasured-gutter note. |

---

### Task 1: The branch, and `editor.indent` in the config

**Files:**
- Modify: `core/src/config.rs:9-20` (`EditorConfig`)
- Modify: `ui/src/lib/api.ts:6-7` (`AppConfig.editor`)
- Modify: `ui/src/lib/commands.test.ts:17` (config literal)
- Modify: `ui/src/components/Settings.svelte:42-48` (Editor section)

**Interfaces:**
- Produces: `AppConfig.editor.indent: number` on both sides. Later tasks read
  `app.config?.editor.indent ?? 2` in the frontend.

- [ ] **Step 1: Create the branch**

```bash
cd /home/user01/engram-notes && git checkout -b feat/editor-floor
```

- [ ] **Step 2: Write the failing Rust test**

Append inside `mod tests` in `core/src/config.rs`:

```rust
    #[test]
    fn editor_indent_defaults_to_two_spaces() {
        assert_eq!(AppConfig::default().editor.indent, 2);
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"editor":{"default_mode":"source"}}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg.editor.default_mode, "source");
        assert_eq!(cfg.editor.indent, 2);
    }
```

- [ ] **Step 3: Run it to see it fail**

Run: `cargo test --workspace editor_indent`
Expected: compile error, `no field 'indent' on type EditorConfig`.

- [ ] **Step 4: Add the field**

Replace `EditorConfig` and its `Default` in `core/src/config.rs`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorConfig {
    pub default_mode: String,
    /// Spaces per outline level. Two is CommonMark-correct under `- `.
    pub indent: usize,
}

impl Default for EditorConfig {
    fn default() -> Self {
        EditorConfig {
            default_mode: "live".into(),
            indent: 2,
        }
    }
}
```

- [ ] **Step 5: Run the Rust tests**

Run: `cargo test --workspace config`
Expected: all pass, including `editor_indent_defaults_to_two_spaces`.

- [ ] **Step 6: Mirror it in the frontend**

In `ui/src/lib/api.ts` change the `editor` line of `AppConfig` to:

```ts
  editor: { default_mode: "live" | "source" | "reading"; indent: number };
```

In `ui/src/lib/commands.test.ts` change the literal `editor: { default_mode: "live" },` to:

```ts
      editor: { default_mode: "live", indent: 2 },
```

In `ui/src/components/Settings.svelte`, after the *Default mode* `</label>`
inside the Editor section, add:

```svelte
      <label>Indent width
        <input type="number" min="1" max="8" step="1" bind:value={cfg.editor.indent} onchange={save} />
      </label>
```

- [ ] **Step 7: Check and test the frontend**

Run: `cd ui && pnpm check && pnpm test`
Expected: `svelte-check` reports 0 errors; every vitest file passes.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/engram-notes && git add core/src/config.rs ui/src/lib/api.ts ui/src/lib/commands.test.ts ui/src/components/Settings.svelte
git commit -F - <<'MSG'
feat(config): editor.indent, the spaces per outline level

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 2: Brackets wrap a selection

**Files:**
- Create: `ui/src/editor/outline.ts` (only `markdownBrackets` for now)
- Create: `ui/src/editor/outline.test.ts`
- Modify: `ui/src/components/Editor.svelte:44-60` (extensions)

**Interfaces:**
- Produces: `markdownBrackets: Extension` — language data telling
  `closeBrackets` which characters pair and wrap in markdown.

- [ ] **Step 1: Write the failing test**

Create `ui/src/editor/outline.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { EditorState, type Transaction } from "@codemirror/state";
import { ensureSyntaxTree, indentUnit, codeFolding } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { closeBrackets, insertBracket } from "@codemirror/autocomplete";
import { markdownBrackets } from "./outline";

function stateOf(doc: string, cursor: number | { anchor: number; head: number }, indent = 2) {
  const state = EditorState.create({
    doc,
    selection: typeof cursor === "number" ? { anchor: cursor } : cursor,
    extensions: [
      markdown({ base: markdownLanguage }),
      markdownBrackets,
      indentUnit.of(" ".repeat(indent)),
      closeBrackets(),
      codeFolding(),
    ],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

// Runs a state command and reports the document and cursor it left behind.
function run(cmd: (t: { state: EditorState; dispatch: (tr: Transaction) => void }) => boolean, state: EditorState) {
  let out = state;
  const ok = cmd({ state, dispatch: (tr) => (out = tr.state) });
  return { ok, doc: out.doc.toString(), head: out.selection.main.head };
}

describe("brackets", () => {
  it("wraps a selection instead of replacing it, twice for a wikilink", () => {
    const s = stateOf("say word here", { anchor: 4, head: 8 });
    const once = insertBracket(s, "[")!;
    expect(once.state.doc.toString()).toBe("say [word] here");
    const twice = insertBracket(once.state, "[")!;
    expect(twice.state.doc.toString()).toBe("say [[word]] here");
    expect([twice.state.selection.main.from, twice.state.selection.main.to]).toEqual([6, 10]);
  });

  it("wraps with the markdown emphasis characters too", () => {
    const s = stateOf("say word here", { anchor: 4, head: 8 });
    for (const [ch, want] of [["*", "say *word* here"], ["_", "say _word_ here"], ["`", "say `word` here"], ['"', 'say "word" here'], ["(", "say (word) here"]]) {
      expect(insertBracket(s, ch)!.state.doc.toString()).toBe(want);
    }
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: FAIL — `./outline` does not exist.

- [ ] **Step 3: Create the module with the language data**

Create `ui/src/editor/outline.ts`:

```ts
import type { Extension } from "@codemirror/state";
import { markdownLanguage } from "@codemirror/lang-markdown";

// closeBrackets reads the pairing set from language data. Obsidian pairs the
// emphasis characters as well as brackets, and wraps a selection with any of them.
export const markdownBrackets: Extension = markdownLanguage.data.of({
  closeBrackets: { brackets: ["(", "[", "{", "'", '"', "*", "_", "`"] },
});
```

- [ ] **Step 4: Run the test to see it pass**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: PASS, 2 tests.

- [ ] **Step 5: Wire it into the editor**

In `ui/src/components/Editor.svelte`:

Change the autocomplete import to:

```ts
  import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
```

(add it as a new import line after the `@codemirror/search` import), and add
after the `completions` import:

```ts
  import { markdownBrackets } from "../editor/outline";
```

In the `extensions: [` array, insert directly after `highlightSelectionMatches(),`:

```ts
          closeBrackets(),
          markdownBrackets,
```

and change the keymap line to:

```ts
          keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
```

- [ ] **Step 6: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 7: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/editor/outline.ts ui/src/editor/outline.test.ts ui/src/components/Editor.svelte
git commit -F - <<'MSG'
fix(editor): a bracket typed over a selection wraps it

Without closeBrackets, CodeMirror replaced the selection with the
character, so marking a word and typing [[ lost the word.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 3: Enter continues a list; the indent unit is a setting

**Files:**
- Modify: `ui/src/editor/outline.test.ts`
- Modify: `ui/src/components/Editor.svelte`
- Modify: `ui/src/components/NoteView.svelte:83-97` (`<Editor …>`)

**Interfaces:**
- Produces: `Editor` prop `indent: number`.

- [ ] **Step 1: Write the failing test**

Add to `ui/src/editor/outline.test.ts` — extend the lang-markdown import and
add a block:

```ts
import { markdown, markdownLanguage, insertNewlineContinueMarkup } from "@codemirror/lang-markdown";
```

```ts
describe("enter", () => {
  it("continues a list, and removes the marker of an empty top-level item", () => {
    expect(run(insertNewlineContinueMarkup, stateOf("- a", 3)).doc).toBe("- a\n- ");
    expect(run(insertNewlineContinueMarkup, stateOf("1. a", 4)).doc).toBe("1. a\n2. ");
    expect(run(insertNewlineContinueMarkup, stateOf("- [ ] a", 7)).doc).toBe("- [ ] a\n- [ ] ");
    expect(run(insertNewlineContinueMarkup, stateOf("- a\n- ", 6)).doc).toBe("- a\n");
  });
});
```

- [ ] **Step 2: Run it**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: PASS — this pins the library behaviour the next task builds on. If
the last assertion's exact string differs (the library may leave a trailing
newline or spaces), copy the actual output into the assertion: the point is
that the marker is gone and no new item was made.

- [ ] **Step 3: Bind the markdown keymap and the indent unit**

In `ui/src/components/Editor.svelte`:

Change the lang-markdown import to:

```ts
  import { markdown, markdownLanguage, markdownKeymap } from "@codemirror/lang-markdown";
```

Add after the `@codemirror/language-data` import:

```ts
  import { indentUnit } from "@codemirror/language";
```

Add `indent: number;` to `Props` after `image`, and `indent` to the
destructuring `let { … } = $props();` list.

In `extensions`, after `EditorView.lineWrapping,` add:

```ts
          indentUnit.of(" ".repeat(indent)),
```

and change the keymap line to:

```ts
          keymap.of([...closeBracketsKeymap, ...markdownKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
```

In `ui/src/components/NoteView.svelte`, add to the `<Editor` element after
`{image}`:

```svelte
        indent={app.config?.editor.indent ?? 2}
```

- [ ] **Step 4: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 5: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/editor/outline.test.ts ui/src/components/Editor.svelte ui/src/components/NoteView.svelte
git commit -F - <<'MSG'
feat(editor): Enter continues a list, and the indent unit is a setting

The keymap had no markdown bindings at all, so a list ended at every
line break.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 4: The list model — items, subtrees, siblings, paths

**Files:**
- Create: `ui/src/lib/outline.ts`
- Create: `ui/src/lib/outline.test.ts`

**Interfaces:**
- Produces:
  - `interface Item { indent: number; marker: string; task: string | null; text: string }`
  - `parseItem(line: string): Item | null`
  - `indentOf(line: string): number` — columns, a tab counting four
  - `subtreeEnd(lines: string[], i: number): number` — exclusive
  - `prevSibling(lines: string[], i: number): number | null`
  - `nextSibling(lines: string[], i: number): number | null`
  - `outlinePaths(lines: string[]): Map<number, string>` — line → `"block:i.j.k"`

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/outline.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { indentOf, nextSibling, outlinePaths, parseItem, prevSibling, subtreeEnd } from "./outline";

const L = (s: string) => s.split("\n");

describe("outline items", () => {
  it("parses bullets, numbers and tasks, a tab counting four columns", () => {
    expect(parseItem("- a")).toEqual({ indent: 0, marker: "-", task: null, text: "a" });
    expect(parseItem("  3) b")).toEqual({ indent: 2, marker: "3)", task: null, text: "b" });
    expect(parseItem("\t- [x] c")).toEqual({ indent: 4, marker: "-", task: "[x]", text: "c" });
    expect(parseItem("- ")).toEqual({ indent: 0, marker: "-", task: null, text: "" });
    expect(parseItem("plain")).toBeNull();
    expect(parseItem("-no space")).toBeNull();
    expect(indentOf("\t  x")).toBe(6);
  });

  it("ends a subtree where indentation returns, blank lines included when deeper text follows", () => {
    const l = L("- a\n  - b\n\n    more b\n- c");
    expect(subtreeEnd(l, 0)).toBe(4);
    expect(subtreeEnd(l, 1)).toBe(4);
    expect(subtreeEnd(l, 4)).toBe(5);
  });

  it("finds siblings across a subtree and stops at prose", () => {
    const l = L("- a\n  - a1\n- b\n\ntext\n- c");
    expect(prevSibling(l, 2)).toBe(0);
    expect(nextSibling(l, 0)).toBe(2);
    expect(nextSibling(l, 2)).toBeNull();
    expect(prevSibling(l, 5)).toBeNull();
    expect(prevSibling(l, 1)).toBeNull();
  });

  it("keys every item by list block and sibling indices", () => {
    const l = L("- a\n  - a1\n  - a2\n- b\n\ntext\n\n1. c\n   text under c\n2. d");
    expect([...outlinePaths(l)]).toEqual([
      [0, "0:0"], [1, "0:0.0"], [2, "0:0.1"], [3, "0:1"], [7, "1:0"], [9, "1:1"],
    ]);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test src/lib/outline.test.ts`
Expected: FAIL — `./outline` does not exist.

- [ ] **Step 3: Write the module**

Create `ui/src/lib/outline.ts`:

```ts
// A markdown list as an array of lines. Every function here is a plain list
// operation on text, so a file it writes is one any markdown editor reads.

export interface Item {
  indent: number;
  marker: string;
  task: string | null;
  text: string;
}

const ITEM = /^( *)([-*+]|\d+[.)])( +)(\[[ xX]\] +)?(.*)$/;
const TABS = /^\t+/;

export const isBlank = (line: string) => line.trim() === "";

// Columns of leading whitespace, a tab counting four as CommonMark does.
export function indentOf(line: string): number {
  let n = 0;
  for (const ch of line) {
    if (ch === " ") n++;
    else if (ch === "\t") n += 4;
    else break;
  }
  return n;
}

export function parseItem(line: string): Item | null {
  const m = ITEM.exec(line.replace(TABS, (t) => "    ".repeat(t.length)));
  if (!m) return null;
  return { indent: m[1].length, marker: m[2], task: m[4] ? m[4].trim() : null, text: m[5] };
}

// The line after the item and everything indented under it. A blank line
// belongs to the subtree only when something deeper follows it.
export function subtreeEnd(lines: string[], i: number): number {
  const base = indentOf(lines[i]);
  let j = i + 1;
  while (j < lines.length) {
    if (isBlank(lines[j])) {
      let k = j;
      while (k < lines.length && isBlank(lines[k])) k++;
      if (k < lines.length && indentOf(lines[k]) > base) {
        j = k;
        continue;
      }
      break;
    }
    if (indentOf(lines[j]) <= base) break;
    j++;
  }
  return j;
}

export function prevSibling(lines: string[], i: number): number | null {
  const base = indentOf(lines[i]);
  for (let j = i - 1; j >= 0; j--) {
    const l = lines[j];
    if (isBlank(l)) continue;
    const ind = indentOf(l);
    if (ind > base) continue;
    return ind === base && parseItem(l) ? j : null;
  }
  return null;
}

export function nextSibling(lines: string[], i: number): number | null {
  const base = indentOf(lines[i]);
  let k = subtreeEnd(lines, i);
  while (k < lines.length && isBlank(lines[k])) k++;
  if (k >= lines.length) return null;
  return indentOf(lines[k]) === base && parseItem(lines[k]) ? k : null;
}

// Every item line keyed "block:i.j.k" — the nth list in the document, then
// sibling indices per depth. Fold state is stored under these keys, so they
// only have to be stable across edits that do not restructure the list.
export function outlinePaths(lines: string[]): Map<number, string> {
  const out = new Map<number, string>();
  let block = -1;
  let inList = false;
  const stack: { indent: number; count: number }[] = [];
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (isBlank(l)) continue;
    const ind = indentOf(l);
    if (!parseItem(l)) {
      if (inList && stack.length && ind > stack[stack.length - 1].indent) continue;
      inList = false;
      stack.length = 0;
      continue;
    }
    if (!inList) {
      inList = true;
      block++;
    }
    while (stack.length && ind < stack[stack.length - 1].indent) stack.pop();
    if (!stack.length || ind > stack[stack.length - 1].indent) stack.push({ indent: ind, count: 0 });
    stack[stack.length - 1].count++;
    out.set(i, `${block}:${stack.map((s) => s.count - 1).join(".")}`);
  }
  return out;
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd ui && pnpm test src/lib/outline.test.ts`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/lib/outline.ts ui/src/lib/outline.test.ts
git commit -F - <<'MSG'
feat(outline): a line model of markdown lists

Items, subtrees, siblings, and a path per item that fold state can be
keyed by.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 5: Indent, outdent, move, renumber

**Files:**
- Modify: `ui/src/lib/outline.ts`
- Modify: `ui/src/lib/outline.test.ts`

**Interfaces:**
- Produces:
  - `indentSubtree(lines, i, unit): string[]`
  - `outdentSubtree(lines, i, unit): string[] | null` — null at column 0
  - `moveSubtree(lines, i, dir: -1 | 1): { lines: string[]; line: number } | null`
  - `renumber(lines, at): string[]` — the list block containing line `at`

- [ ] **Step 1: Write the failing tests**

Extend the import in `ui/src/lib/outline.test.ts`:

```ts
import { indentOf, indentSubtree, moveSubtree, nextSibling, outdentSubtree, outlinePaths, parseItem, prevSibling, renumber, subtreeEnd } from "./outline";
```

Append:

```ts
describe("indent and outdent", () => {
  it("moves the item with its subtree and normalises tabs it touches to spaces", () => {
    expect(indentSubtree(L("- a\n- b\n\t- c\n- d"), 1, 2)).toEqual(L("- a\n  - b\n      - c\n- d"));
    expect(outdentSubtree(L("- a\n  - b\n    - c\n- d"), 1, 2)).toEqual(L("- a\n- b\n  - c\n- d"));
    expect(outdentSubtree(L("- a"), 0, 2)).toBeNull();
  });

  it("outdents by what is there when the indent is short of a unit", () => {
    expect(outdentSubtree(L("- a\n - b"), 1, 2)).toEqual(L("- a\n- b"));
  });

  it("renumbers the ordered list it changed, each run from one", () => {
    expect(outdentSubtree(L("1. a\n  1. b\n  2. c\n2. d"), 1, 2)).toEqual(L("1. a\n2. b\n  1. c\n3. d"));
    expect(renumber(L("3. x\n\ntext\n\n1. a\n1. b"), 4)).toEqual(L("3. x\n\ntext\n\n1. a\n2. b"));
    expect(renumber(L("- a\n1. b\n1. c"), 1)).toEqual(L("- a\n1. b\n2. c"));
  });
});

describe("move", () => {
  it("swaps a subtree with its sibling and keeps the blank between them", () => {
    const l = L("- a\n  - a1\n\n- b\n- c");
    expect(moveSubtree(l, 3, -1)).toEqual({ lines: L("- b\n\n- a\n  - a1\n- c"), line: 0 });
    expect(moveSubtree(l, 0, 1)).toEqual({ lines: L("- b\n\n- a\n  - a1\n- c"), line: 2 });
    expect(moveSubtree(l, 4, 1)).toBeNull();
    expect(moveSubtree(l, 1, -1)).toBeNull();
  });

  it("renumbers after a move", () => {
    expect(moveSubtree(L("1. a\n2. b"), 1, -1)).toEqual({ lines: L("1. b\n2. a"), line: 0 });
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test src/lib/outline.test.ts`
Expected: FAIL — `indentSubtree` is not exported.

- [ ] **Step 3: Implement**

Append to `ui/src/lib/outline.ts`:

```ts
const ORDERED = /^(\s*)(\d+)([.)])(\s)/;

// Rewrites the leading whitespace outright, so a tab on a touched line becomes spaces.
function setIndent(line: string, n: number): string {
  return isBlank(line) ? line : " ".repeat(n) + line.replace(/^[ \t]+/, "");
}

export function indentSubtree(lines: string[], i: number, unit: number): string[] {
  const end = subtreeEnd(lines, i);
  const out = lines.slice();
  for (let j = i; j < end; j++) out[j] = setIndent(out[j], indentOf(out[j]) + unit);
  return renumber(out, i);
}

export function outdentSubtree(lines: string[], i: number, unit: number): string[] | null {
  const base = indentOf(lines[i]);
  if (base === 0) return null;
  const delta = Math.min(unit, base);
  const end = subtreeEnd(lines, i);
  const out = lines.slice();
  for (let j = i; j < end; j++) out[j] = setIndent(out[j], Math.max(0, indentOf(out[j]) - delta));
  return renumber(out, i);
}

// Swaps the subtree at `i` with its previous (-1) or next (+1) sibling's. Blank
// lines between the two stay between them.
export function moveSubtree(lines: string[], i: number, dir: -1 | 1): { lines: string[]; line: number } | null {
  if (dir < 0) {
    const p = prevSibling(lines, i);
    if (p === null) return null;
    const endA = subtreeEnd(lines, p);
    const endB = subtreeEnd(lines, i);
    const out = [...lines.slice(0, p), ...lines.slice(i, endB), ...lines.slice(endA, i), ...lines.slice(p, endA), ...lines.slice(endB)];
    return { lines: renumber(out, p), line: p };
  }
  const n = nextSibling(lines, i);
  if (n === null) return null;
  const endA = subtreeEnd(lines, i);
  const endB = subtreeEnd(lines, n);
  const out = [...lines.slice(0, i), ...lines.slice(n, endB), ...lines.slice(endA, n), ...lines.slice(i, endA), ...lines.slice(endB)];
  const line = i + (endB - endA);
  return { lines: renumber(out, line), line };
}

// Ordered items in the list block containing `at` count from one per run of
// siblings. Only that block is touched, so a list elsewhere keeps its numbers.
export function renumber(lines: string[], at: number): string[] {
  const paths = outlinePaths(lines);
  const block = paths.get(at)?.split(":")[0];
  if (block === undefined) return lines;
  const out = lines.slice();
  for (const [i, p] of paths) {
    if (!p.startsWith(`${block}:`)) continue;
    const m = ORDERED.exec(out[i]);
    if (!m) continue;
    const prev = prevSibling(out, i);
    const pm = prev === null ? null : ORDERED.exec(out[prev]);
    const n = pm ? String(Number(pm[2]) + 1) : "1";
    if (n !== m[2]) out[i] = m[1] + n + m[3] + out[i].slice(m[0].length - 1);
  }
  return out;
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd ui && pnpm test src/lib/outline.test.ts`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/lib/outline.ts ui/src/lib/outline.test.ts
git commit -F - <<'MSG'
feat(outline): indent, outdent and move a subtree; renumber its list

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 6: The commands and the keymap with `Tab` precedence

**Files:**
- Modify: `ui/src/editor/outline.ts`
- Modify: `ui/src/editor/outline.test.ts`
- Modify: `ui/src/components/Editor.svelte`

**Interfaces:**
- Consumes: everything from `ui/src/lib/outline.ts`.
- Produces: `indentItem`, `outdentItem`, `moveItemUp`, `moveItemDown`,
  `enterInList` (`StateCommand`s) and `outlineKeymap: KeyBinding[]`.

- [ ] **Step 1: Write the failing tests**

Extend the import in `ui/src/editor/outline.test.ts`:

```ts
import { enterInList, indentItem, markdownBrackets, moveItemDown, moveItemUp, outdentItem } from "./outline";
```

Append:

```ts
describe("outline commands", () => {
  it("outdents an empty nested item on Enter and leaves the rest to the markdown keymap", () => {
    expect(run(enterInList, stateOf("- a\n  - ", 8))).toEqual({ ok: true, doc: "- a\n- ", head: 6 });
    expect(run(enterInList, stateOf("- a\n  - [ ] ", 12))).toEqual({ ok: true, doc: "- a\n- [ ] ", head: 10 });
    expect(run(enterInList, stateOf("- a\n- ", 6)).ok).toBe(false);
    expect(run(enterInList, stateOf("- a\n  - b", 9)).ok).toBe(false);
  });

  it("indents and outdents the subtree under the cursor, keeping the cursor on its text", () => {
    expect(run(indentItem, stateOf("- a\n- b\n  - c", 5))).toEqual({ ok: true, doc: "- a\n  - b\n    - c", head: 7 });
    expect(run(outdentItem, stateOf("- a\n  - b\n    - c", 7))).toEqual({ ok: true, doc: "- a\n- b\n  - c", head: 5 });
    expect(run(outdentItem, stateOf("- a", 1)).ok).toBe(true);
    expect(run(indentItem, stateOf("prose", 2)).ok).toBe(false);
  });

  it("moves a subtree among its siblings", () => {
    expect(run(moveItemDown, stateOf("- a\n  - a1\n- b", 1))).toEqual({ ok: true, doc: "- b\n- a\n  - a1", head: 5 });
    expect(run(moveItemUp, stateOf("- b\n- a\n  - a1", 5))).toEqual({ ok: true, doc: "- a\n  - a1\n- b", head: 1 });
    expect(run(moveItemUp, stateOf("- a\n- b", 1)).ok).toBe(true);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: FAIL — `enterInList` is not exported.

- [ ] **Step 3: Implement the commands**

Replace the top of `ui/src/editor/outline.ts` with these imports and append
the commands below the existing `markdownBrackets`:

```ts
import { EditorSelection, type EditorState, type Extension, type StateCommand } from "@codemirror/state";
import type { KeyBinding } from "@codemirror/view";
import { indentLess, indentMore } from "@codemirror/commands";
import { acceptCompletion } from "@codemirror/autocomplete";
import { getIndentUnit } from "@codemirror/language";
import { markdownLanguage } from "@codemirror/lang-markdown";
import * as O from "../lib/outline";
```

```ts
const lineArray = (state: EditorState) => state.doc.toString().split("\n");

function cursorLine(state: EditorState) {
  const l = state.doc.lineAt(state.selection.main.head);
  return { line: l.number - 1, col: state.selection.main.head - l.from };
}

// Replaces only the span of lines that differ and puts the cursor on `line`
// at `col`. Outline operations never change the line count, which is what
// lets the diff be a plain replacement.
function applyLines(state: EditorState, oldLines: string[], newLines: string[], line: number, col: number) {
  let a = 0;
  while (a < oldLines.length && oldLines[a] === newLines[a]) a++;
  let b = oldLines.length;
  while (b > a && oldLines[b - 1] === newLines[b - 1]) b--;
  const changes = b > a
    ? { from: state.doc.line(a + 1).from, to: state.doc.line(b).to, insert: newLines.slice(a, b).join("\n") }
    : [];
  const head = newLines.slice(0, line).reduce((n, l) => n + l.length + 1, 0) + Math.min(col, newLines[line].length);
  return state.update({ changes, selection: EditorSelection.cursor(head), scrollIntoView: true, userEvent: "input" });
}

// A command that runs only with a single cursor on a list item. `f` returning
// null means the operation does not apply here (top of the list, no sibling);
// the key is still consumed, as Obsidian does, so nothing else fires.
function onItem(f: (lines: string[], i: number, unit: number) => { lines: string[]; line: number } | null): StateCommand {
  return ({ state, dispatch }) => {
    if (!state.selection.main.empty) return false;
    const { line, col } = cursorLine(state);
    const lines = lineArray(state);
    if (!O.parseItem(lines[line])) return false;
    const r = f(lines, line, getIndentUnit(state));
    if (!r) return true;
    const shift = O.indentOf(r.lines[r.line]) - O.indentOf(lines[line]);
    dispatch(applyLines(state, lines, r.lines, r.line, Math.max(0, col + shift)));
    return true;
  };
}

export const indentItem = onItem((lines, i, unit) => ({ lines: O.indentSubtree(lines, i, unit), line: i }));
export const outdentItem = onItem((lines, i, unit) => {
  const out = O.outdentSubtree(lines, i, unit);
  return out && { lines: out, line: i };
});
export const moveItemUp = onItem((lines, i) => O.moveSubtree(lines, i, -1));
export const moveItemDown = onItem((lines, i) => O.moveSubtree(lines, i, 1));

// Enter on an empty nested item outdents it; the markdown keymap handles the
// rest, including removing the marker of an empty top-level item.
export const enterInList: StateCommand = ({ state, dispatch }) => {
  if (!state.selection.main.empty) return false;
  const { line } = cursorLine(state);
  const lines = lineArray(state);
  const item = O.parseItem(lines[line]);
  if (!item || item.text.trim() !== "" || item.indent === 0) return false;
  const out = O.outdentSubtree(lines, line, getIndentUnit(state))!;
  dispatch(applyLines(state, lines, out, line, out[line].length));
  return true;
};

// The spec's Tab precedence: completion, then the outline, then indentation.
export const outlineKeymap: KeyBinding[] = [
  { key: "Tab", run: acceptCompletion },
  { key: "Tab", run: indentItem },
  { key: "Tab", run: indentMore },
  { key: "Shift-Tab", run: outdentItem },
  { key: "Shift-Tab", run: indentLess },
  { key: "Alt-ArrowUp", run: moveItemUp },
  { key: "Alt-ArrowDown", run: moveItemDown },
  { key: "Enter", run: enterInList },
];
```

Remove the earlier `import type { Extension } from "@codemirror/state";` line
so the type is imported once.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: PASS, 6 tests.

- [ ] **Step 5: Bind the keymap**

In `ui/src/components/Editor.svelte`, change the commands import to drop
`indentWithTab`:

```ts
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
```

change the outline import to:

```ts
  import { markdownBrackets, outlineKeymap } from "../editor/outline";
```

and the keymap line to — order matters, ours must precede `defaultKeymap`'s
`Alt-ArrowUp` and `markdownKeymap`'s `Enter`:

```ts
          keymap.of([...closeBracketsKeymap, ...outlineKeymap, ...markdownKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap]),
```

- [ ] **Step 6: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 7: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/editor/outline.ts ui/src/editor/outline.test.ts ui/src/components/Editor.svelte
git commit -F - <<'MSG'
feat(editor): Tab, Shift+Tab and Alt+arrows work on the item's subtree

Tab goes to an open completion first, then the outline, then plain
indentation, as the spec orders it. Enter on an empty nested item
outdents instead of ending the list.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 7: Folding — list items and headings only, with a gutter chevron

**Files:**
- Modify: `ui/src/editor/outline.ts`
- Modify: `ui/src/editor/outline.test.ts`
- Modify: `ui/src/editor/theme.ts`
- Modify: `ui/src/components/Editor.svelte`
- Modify: `ui/src/app.css:122-123`

**Interfaces:**
- Produces: `listOnlyFolding` (a lang-markdown parser extension) and
  `outlineFolding(): Extension` (the fold state field and the gutter).

- [ ] **Step 1: Write the failing test**

In `ui/src/editor/outline.test.ts`, extend the two imports:

```ts
import { ensureSyntaxTree, indentUnit, codeFolding, foldable } from "@codemirror/language";
import { enterInList, indentItem, listOnlyFolding, markdownBrackets, moveItemDown, moveItemUp, outdentItem } from "./outline";
```

change `markdown({ base: markdownLanguage }),` in `stateOf` to:

```ts
      markdown({ base: markdownLanguage, extensions: [listOnlyFolding] }),
```

and append:

```ts
describe("folding", () => {
  it("folds list items and headings, never paragraphs", () => {
    const s = stateOf("- a\n  - b\n\npara one\npara two\n# H\ntext", 0);
    const l = (n: number) => s.doc.line(n);
    const item = foldable(s, l(1).from, l(1).to)!;
    expect(item.from).toBe(3);
    expect(s.doc.sliceString(0, item.to).trimEnd()).toBe("- a\n  - b");
    expect(foldable(s, l(4).from, l(4).to)).toBeNull();
    expect(foldable(s, l(6).from, l(6).to)).toEqual({ from: l(6).to, to: s.doc.length });
    expect(foldable(s, l(2).from, l(2).to)).toBeNull();
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: FAIL — `listOnlyFolding` is not exported.

- [ ] **Step 3: Implement the fold rule and the gutter**

Add to the imports of `ui/src/editor/outline.ts`:

```ts
import { codeFolding, foldGutter, foldNodeProp, getIndentUnit } from "@codemirror/language";
```

(merging with the existing `getIndentUnit` import), and append:

```ts
// lang-markdown makes every block foldable. Obsidian folds headings and list
// items only; a later prop source wins, so the rest are switched off here.
export const listOnlyFolding = {
  props: [
    foldNodeProp.add({
      Paragraph: () => null,
      Blockquote: () => null,
      FencedCode: () => null,
      CodeBlock: () => null,
      HTMLBlock: () => null,
      Table: () => null,
      LinkReference: () => null,
      CommentBlock: () => null,
      ProcessingInstructionBlock: () => null,
    }),
  ],
};

const CHEVRON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>';

export function outlineFolding(): Extension {
  return [
    codeFolding(),
    foldGutter({
      markerDOM(open) {
        const s = document.createElement("span");
        s.className = `cm-fold-marker ${open ? "open" : "closed"}`;
        s.innerHTML = CHEVRON;
        return s;
      },
    }),
  ];
}
```

- [ ] **Step 4: Run the test to see it pass**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: PASS, 7 tests. If the paragraph assertion fails because the
earlier prop source wins, replace the object form with a function source
that answers for every block type:
`foldNodeProp.add((type) => type.name === "ListItem" ? undefined : () => null)`
— `undefined` leaves the earlier rule in place for list items — and re-run.

- [ ] **Step 5: Show the gutter, and only the fold gutter**

In `ui/src/editor/theme.ts`, replace the `".cm-gutters": { display: "none" }`
line with:

```ts
  ".cm-gutters": { backgroundColor: "transparent", border: "none", color: "var(--fg-muted)" },
  ".cm-foldGutter .cm-gutterElement": { width: "24px", display: "flex", justifyContent: "center", paddingTop: "0.3em" },
  ".cm-fold-marker": { opacity: "0", width: "16px", height: "16px", cursor: "pointer", transition: "opacity 120ms" },
  ".cm-fold-marker svg": { display: "block" },
  ".cm-fold-marker.closed": { opacity: "1", transform: "rotate(-90deg)" },
  ".cm-gutters:hover .cm-fold-marker, .cm-activeLineGutter .cm-fold-marker": { opacity: "0.6" },
  ".cm-foldGutter .cm-gutterElement:hover .cm-fold-marker": { opacity: "1" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" },
  ".cm-foldPlaceholder": { background: "var(--bg-3)", border: "none", color: "var(--fg-muted)", padding: "0 6px", borderRadius: "3px" },
```

In `ui/src/app.css`, replace lines 122–123 (`.cm-editor { … }` and
`.cm-editor .cm-content { … }`) with:

```css
/* The column centres as a whole so the fold gutter sits in its left inset;
   24px gutter + 8px padding is the 32px the properties block uses. */
.cm-editor { height: auto; font-family: var(--font-text); font-size: 16px; max-width: var(--file-line-width); margin: 0 auto; }
.cm-editor .cm-content { padding: 24px 32px 24px 8px; }
```

- [ ] **Step 6: Wire folding into the editor**

In `ui/src/components/Editor.svelte`:

Change the view import to:

```ts
  import { EditorView, keymap, drawSelection, highlightActiveLine, highlightActiveLineGutter } from "@codemirror/view";
```

the outline import to:

```ts
  import { listOnlyFolding, markdownBrackets, outlineFolding, outlineKeymap } from "../editor/outline";
```

the language line to:

```ts
          yamlFrontmatter({ content: markdown({ base: markdownLanguage, codeLanguages: languages, extensions: [listOnlyFolding] }) }),
```

and add, after `highlightActiveLine(),`:

```ts
          highlightActiveLineGutter(),
          outlineFolding(),
```

- [ ] **Step 7: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/editor/outline.ts ui/src/editor/outline.test.ts ui/src/editor/theme.ts ui/src/components/Editor.svelte ui/src/app.css
git commit -F - <<'MSG'
feat(editor): fold list items and headings from a gutter chevron

The gutter's size and hover are not measured against Obsidian; this
machine has no copy to measure.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 8: Fold keys — naming what is folded, and restoring it

**Files:**
- Modify: `ui/src/editor/outline.ts`
- Modify: `ui/src/editor/outline.test.ts`

**Interfaces:**
- Produces: `foldKeys(state): string[]` and
  `foldTransaction(state, keys): TransactionSpec | null`. Keys are outline
  paths for list items and `h<n>` for the nth heading.

- [ ] **Step 1: Write the failing test**

In `ui/src/editor/outline.test.ts`, extend the imports:

```ts
import { ensureSyntaxTree, indentUnit, codeFolding, foldable, foldEffect } from "@codemirror/language";
import { enterInList, foldKeys, foldTransaction, indentItem, listOnlyFolding, markdownBrackets, moveItemDown, moveItemUp, outdentItem } from "./outline";
```

Append inside `describe("folding", …)`:

```ts
  it("names folded items by outline path and headings by ordinal, and restores them", () => {
    const s = stateOf("- a\n  - b\n- c\n  - d\n# H\ntext", 0);
    const range = foldable(s, s.doc.line(3).from, s.doc.line(3).to)!;
    const folded = s.update({ effects: foldEffect.of(range) }).state;
    expect(foldKeys(folded)).toEqual(["0:1"]);
    const restored = s.update(foldTransaction(s, ["0:1", "h1"])!).state;
    expect(foldKeys(restored)).toEqual(["0:1", "h1"]);
    expect(foldTransaction(s, ["9:9"])).toBeNull();
  });
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: FAIL — `foldKeys` is not exported.

- [ ] **Step 3: Implement**

Extend the imports of `ui/src/editor/outline.ts`:

```ts
import { EditorSelection, type EditorState, type Extension, type StateCommand, type TransactionSpec } from "@codemirror/state";
import { codeFolding, foldEffect, foldGutter, foldNodeProp, foldable, foldedRanges, getIndentUnit } from "@codemirror/language";
```

and append:

```ts
const HEADING = /^#{1,6}\s/;

// A list item's key is its outline path; a heading's is its ordinal. Both
// survive the edits that leave the structure alone, which is all that is asked.
function keyOfLine(lines: string[], paths: Map<number, string>, n: number): string | null {
  const p = paths.get(n);
  if (p) return p;
  if (!HEADING.test(lines[n])) return null;
  let h = 0;
  for (let i = 0; i <= n; i++) if (HEADING.test(lines[i])) h++;
  return `h${h}`;
}

export function foldKeys(state: EditorState): string[] {
  const lines = lineArray(state);
  const paths = O.outlinePaths(lines);
  const keys: string[] = [];
  foldedRanges(state).between(0, state.doc.length, (from) => {
    const k = keyOfLine(lines, paths, state.doc.lineAt(from).number - 1);
    if (k) keys.push(k);
  });
  return keys;
}

export function foldTransaction(state: EditorState, keys: string[]): TransactionSpec | null {
  const want = new Set(keys);
  const lines = lineArray(state);
  const paths = O.outlinePaths(lines);
  const effects = [];
  for (let n = 0; n < lines.length; n++) {
    const k = keyOfLine(lines, paths, n);
    if (!k || !want.has(k)) continue;
    const l = state.doc.line(n + 1);
    const r = foldable(state, l.from, l.to);
    if (r) effects.push(foldEffect.of(r));
  }
  return effects.length ? { effects } : null;
}
```

- [ ] **Step 4: Run the test to see it pass**

Run: `cd ui && pnpm test src/editor/outline.test.ts`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/editor/outline.ts ui/src/editor/outline.test.ts
git commit -F - <<'MSG'
feat(editor): fold state as keys a note can be reopened with

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 9: Fold rows in the index, moved on rename, two commands

**Files:**
- Modify: `core/src/index/schema.sql`
- Modify: `core/src/index/mod.rs:3-12` (module list, `SCHEMA_VERSION`)
- Create: `core/src/index/folds.rs`
- Modify: `core/src/rename.rs:106-113`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Produces on `Index`: `folds(&self, path: &str) -> Result<Vec<String>>`,
  `set_folds(&mut self, path: &str, keys: &[String]) -> Result<()>`,
  `move_folds(&mut self, from: &str, to: &str) -> Result<()>`.
- Produces in `api.ts`: `getFolds(path): Promise<string[]>`,
  `setFolds(path, keys): Promise<void>`.

- [ ] **Step 1: Write the failing Rust tests**

Create `core/src/index/folds.rs`:

```rust
//! Which items of a note are folded. Cosmetic and per machine: it lives in
//! the derived index and never in the file.

use super::Index;
use crate::Result;

impl Index {
    pub fn folds(&self, path: &str) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare("SELECT key FROM folds WHERE path=?1 ORDER BY key")?;
        let rows = st.query_map([path], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_folds(&mut self, path: &str, keys: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM folds WHERE path=?1", [path])?;
        for k in keys {
            tx.execute(
                "INSERT OR IGNORE INTO folds(path, key) VALUES (?1, ?2)",
                [path, k],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_folds(&mut self, from: &str, to: &str) -> Result<()> {
        self.conn
            .execute("UPDATE OR REPLACE folds SET path=?2 WHERE path=?1", [from, to])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::index::Index;

    #[test]
    fn folds_round_trip_replace_and_move() {
        let mut ix = Index::open_in_memory().unwrap();
        assert!(ix.folds("a.md").unwrap().is_empty());
        ix.set_folds("a.md", &["0:1".into(), "h2".into()]).unwrap();
        assert_eq!(ix.folds("a.md").unwrap(), vec!["0:1".to_string(), "h2".to_string()]);
        ix.set_folds("a.md", &["0:2".into()]).unwrap();
        assert_eq!(ix.folds("a.md").unwrap(), vec!["0:2".to_string()]);
        ix.move_folds("a.md", "b.md").unwrap();
        assert!(ix.folds("a.md").unwrap().is_empty());
        assert_eq!(ix.folds("b.md").unwrap(), vec!["0:2".to_string()]);
    }
}
```

Add to the tests in `core/src/rename.rs`, next to
`a_rename_carries_memory_to_the_new_path`, using that test's imports:

```rust
    #[test]
    fn a_rename_carries_folds_to_the_new_path() {
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        vault.write("a.md", "- one\n  - two\n").unwrap();
        let mut index = Index::open_in_memory().unwrap();
        index.rebuild(&vault).unwrap();
        index.set_folds("a.md", &["0:0".into()]).unwrap();
        let plan = plan_rename(&vault, &index, "a.md", "b.md").unwrap();
        apply_rename(&vault, &mut index, &plan).unwrap();
        assert!(index.folds("a.md").unwrap().is_empty());
        assert_eq!(index.folds("b.md").unwrap(), vec!["0:0".to_string()]);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --workspace folds`
Expected: compile error — `folds` is not a module of `index`.

- [ ] **Step 3: The table, the version, the module, the rename**

Append to `core/src/index/schema.sql`:

```sql
-- Fold state is view state, kept here so it is never written into a note.
-- No foreign key: it follows a rename through move_folds, like memory.
CREATE TABLE IF NOT EXISTS folds (
  path TEXT NOT NULL,
  key TEXT NOT NULL,
  PRIMARY KEY (path, key)
);
```

In `core/src/index/mod.rs`, add `pub mod folds;` to the module list (keep
the list alphabetical: `folds`, `fts`, `query`, `rebuild`, `resolve`) and
change:

```rust
pub const SCHEMA_VERSION: &str = "4";
```

In `core/src/rename.rs`, inside the `for m in &moves` loop of
`apply_rename`, after `index.move_memory(&m.from, &m.to)?;` add:

```rust
            index.move_folds(&m.from, &m.to)?;
```

- [ ] **Step 4: Run the Rust suite**

Run: `cargo fmt && cargo clippy --workspace -- -D warnings && cargo test --workspace`
Expected: clean, and every test passes including the two new ones and
`schema_version_mismatch_rebuilds_file`.

- [ ] **Step 5: The commands**

In `src-tauri/src/commands.rs`, after `set_workspace`, add:

```rust
#[tauri::command]
pub fn get_folds(state: State<AppState>, path: String) -> CmdResult<Vec<String>> {
    with_open(&state, |o| Ok(o.index.folds(&path)?))
}

#[tauri::command]
pub fn set_folds(state: State<AppState>, path: String, keys: Vec<String>) -> CmdResult<()> {
    with_open(&state, |o| Ok(o.index.set_folds(&path, &keys)?))
}
```

In `src-tauri/src/lib.rs`, after `commands::set_workspace,` add:

```rust
            commands::get_folds,
            commands::set_folds,
```

In `ui/src/lib/api.ts`, after the `setWorkspace` line add:

```ts
export const getFolds = (path: string) => invoke<string[]>("get_folds", { path });
export const setFolds = (path: string, keys: string[]) => invoke<void>("set_folds", { path, keys });
```

- [ ] **Step 6: Build the shell and check the frontend**

Run: `cargo clippy --workspace -- -D warnings && cargo test --workspace && cd ui && pnpm check`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
cd /home/user01/engram-notes && git add core/src/index/schema.sql core/src/index/mod.rs core/src/index/folds.rs core/src/rename.rs src-tauri/src/commands.rs src-tauri/src/lib.rs ui/src/lib/api.ts
git commit -F - <<'MSG'
feat(index): fold state per note, carried by a rename

Schema 3 -> 4, so the derived index rebuilds once.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

---

### Task 10: Folds restored on open and saved on change; docs; screenshots

**Files:**
- Modify: `ui/src/components/Editor.svelte`
- Modify: `ui/src/components/NoteView.svelte`
- Modify: `ui/scripts/shot.mjs` (the `switch (cmd)`)
- Create: `ui/scripts/fixtures/outline.json`, `ui/scripts/fixtures/outline-dark.json`
- Modify: `docs/smoke.md`, `docs/memory.md`

**Interfaces:**
- Consumes: `foldKeys`, `foldTransaction` (Task 8); `getFolds`, `setFolds`
  (Task 9).
- Produces: `Editor` props `folds: string[] | null` and
  `onFolds: (keys: string[]) => void`.

- [ ] **Step 1: The editor restores and reports folds**

In `ui/src/components/Editor.svelte`:

Change the language import to:

```ts
  import { ensureSyntaxTree, indentUnit } from "@codemirror/language";
```

add after it:

```ts
  import { foldEffect, unfoldEffect } from "@codemirror/language";
```

change the outline import to:

```ts
  import { foldKeys, foldTransaction, listOnlyFolding, markdownBrackets, outlineFolding, outlineKeymap } from "../editor/outline";
```

add to `Props` after `indent: number;`:

```ts
    // Fold keys from the index, or null while they load; and where to send changes.
    folds: string[] | null;
    onFolds: (keys: string[]) => void;
```

add `folds, onFolds` to the `$props()` destructuring.

In the `updateListener`, replace the body with:

```ts
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString());
            if (u.transactions.some((tr) => tr.effects.some((e) => e.is(foldEffect) || e.is(unfoldEffect)))) {
              onFolds(foldKeys(u.state));
            }
          }),
```

After the `$effect` that reconfigures `mode`, add:

```ts
  // Folds are applied once, when both the view and the keys exist. The tree
  // has to be parsed that far first or foldable() answers null.
  let foldsApplied = false;
  $effect(() => {
    const v = view;
    const keys = folds;
    if (!v || !keys || foldsApplied) return;
    foldsApplied = true;
    if (!keys.length) return;
    ensureSyntaxTree(v.state, v.state.doc.length, 2000);
    const tr = foldTransaction(v.state, keys);
    if (tr) v.dispatch(tr);
  });
```

- [ ] **Step 2: The note view loads and saves them**

In `ui/src/components/NoteView.svelte`:

Change the api import to include the two calls:

```ts
  import { resolveLink, createNote, anchorLine, tags as apiTags, typing, errorMessage, getFolds, setFolds } from "../lib/api";
```

After `let tagList = $state<string[]>([]);` add:

```ts
  let folds = $state<string[] | null>(null);
  let foldTimer: ReturnType<typeof setTimeout> | undefined;
  // Folding is frequent and cosmetic; one write per pause is enough.
  function onFolds(keys: string[]) {
    clearTimeout(foldTimer);
    foldTimer = setTimeout(() => void setFolds(path, keys).catch(() => {}), 500);
  }
```

Change `onMount` to:

```ts
  onMount(async () => {
    tagList = (await apiTags()).map((t) => t.tag);
    try {
      folds = await getFolds(path);
    } catch {
      folds = [];
    }
  });
```

Add to the `<Editor` element after the `indent=` line:

```svelte
        {folds}
        {onFolds}
```

- [ ] **Step 3: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 4: The screenshot fixture**

In `ui/scripts/shot.mjs`, inside `switch (cmd)`, after the `"get_workspace"`
case add:

```js
        case "get_folds": return (FX.folds ?? {})[args.path] ?? [];
        case "set_folds": return null;
```

Create `ui/scripts/fixtures/outline.json`:

```json
{
  "root": "/vault",
  "config": {
    "editor": { "default_mode": "live", "indent": 2 },
    "daily_notes": { "folder": "Daily", "template": null, "format": "%Y-%m-%d" },
    "hotkeys": {},
    "theme": "light"
  },
  "files": [
    { "path": "Outline.md", "text": "# Plan\n\n- Interview the treasurer\n  - Ask about the March statement\n  - Ask who signed\n- Pull the registry filings\n  - [ ] 2024\n  - [x] 2025\n\n1. Draft\n2. Review\n3. File\n\nA paragraph that is not a list, and long enough to wrap onto a second line so that its gutter stays empty.\n" }
  ],
  "folds": { "Outline.md": ["0:1"] },
  "workspace": {
    "layout": { "kind": "pane", "id": 1, "tabs": [{ "path": "Outline.md", "mode": "live" }], "active": 0 },
    "activePane": 1
  }
}
```

Create `ui/scripts/fixtures/outline-dark.json` as a copy with
`"theme": "dark"`.

- [ ] **Step 5: Take both screenshots and look at them**

Run, from `ui/`, with `pnpm dev` started in the background first:

```bash
cd /home/user01/engram-notes/ui && (pnpm dev > /tmp/claude-1000/-home-user01-engram-notes/4fb97236-3a2f-4ed4-965c-278a8d191681/scratchpad/dev.log 2>&1 &) && sleep 4
node scripts/shot.mjs scripts/fixtures/outline.json /tmp/claude-1000/-home-user01-engram-notes/4fb97236-3a2f-4ed4-965c-278a8d191681/scratchpad/outline.png
node scripts/shot.mjs scripts/fixtures/outline-dark.json /tmp/claude-1000/-home-user01-engram-notes/4fb97236-3a2f-4ed4-965c-278a8d191681/scratchpad/outline-dark.png
```

Open both PNGs with the Read tool. Expected: the list renders with bullets;
*Pull the registry filings* shows a closed chevron and a `…` placeholder
with its two tasks hidden; the paragraph's gutter shows nothing; the text's
left edge lines up with the heading's; the ordered list reads 1, 2, 3. Fix
anything that is off before going on.

- [ ] **Step 6: Docs**

Append to `docs/smoke.md`:

```markdown
49. Select a word and type `[[`: it becomes `[[word]]` with the word still
    selected; `*`, `` ` ``, `(` and `"` wrap the same way.
50. Enter at the end of a list item continues the list; Enter on an empty
    nested item outdents it; on an empty top-level item it removes the bullet.
51. Tab and Shift+Tab on an item with children move the whole subtree;
    Alt+Up/Down swap it with its sibling, and an ordered list stays in
    sequence afterwards.
52. Hover an item with children: a chevron appears in the gutter; click it to
    fold; close and reopen the note and it is still folded; rename the note
    and it stays folded.
53. With the `[[` popup open, Tab accepts the completion; Tab in prose inserts
    the indent width from settings.
```

Append to `docs/memory.md`:

```markdown
## Not measured: the fold gutter

The list and heading fold gutter (0.2) was built without a copy of Obsidian
to measure: a 24px gutter, a 16px chevron shown on hover of the gutter or
the active line, rotated when closed. Every other Obsidian-shaped surface in
this application was measured from `obsidian.asar`; this one should be, on a
machine that has it.
```

- [ ] **Step 7: Full verification**

Run: `cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace && cd ui && pnpm check && pnpm test`
Expected: all clean.

- [ ] **Step 8: Commit**

```bash
cd /home/user01/engram-notes && git add ui/src/components/Editor.svelte ui/src/components/NoteView.svelte ui/scripts/shot.mjs ui/scripts/fixtures/outline.json ui/scripts/fixtures/outline-dark.json docs/smoke.md docs/memory.md
git commit -F - <<'MSG'
feat(editor): a note reopens with the folds it was closed with

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ErWx3H5CREppnPK5SxjkpL
MSG
```

- [ ] **Step 9: The real window**

Build and run against the demo vault, then ask the user for screenshots —
the headless check has missed every defect that mattered in this repository:

```bash
cd /home/user01/engram-notes/ui && pnpm tauri build --debug --no-bundle
/home/user01/engram-notes/target/debug/engram-notes ~/engram-demo-vault
```

Ask the user to run smoke lines 49–53, and to hold the fold gutter beside
Obsidian's on a machine that has it. Tell them plainly that the gutter is the
one unmeasured surface in this release.

---

## Self-review against the spec

**Coverage of *The editor's floor*:** wrapping (Task 2); list continuation
and the indent setting (Tasks 1, 3); no forced `- ` and indentation as spaces
(Tasks 4–5, `setIndent`); fold state never in the note (Tasks 8–10); no zoom
(nothing builds it); every gesture — `Tab`/`Shift+Tab` with subtree,
`Alt+Up/Down`, `Enter` on an empty item, `Backspace` outdent (lang-markdown's
`deleteMarkupBackward`, bound in Task 3), gutter chevron, renumbering, task
items inheriting (Tasks 5–7); `Tab` precedence (Task 6). Testing bullets from
the spec's *Testing* section: outline round-trips (Task 5 tests), state keys
degrade without corrupting (Task 8: a missing key folds nothing), wrap and
`Tab` precedence (Tasks 2, 6).

**Placeholders:** none. Two steps name a fallback (Task 3 step 2, Task 7 step
4) because they pin library behaviour that cannot be read from the source at
hand; each fallback is a concrete instruction, not "adjust as needed".

**Type consistency:** `Item`, `indentOf`, `parseItem`, `subtreeEnd`,
`prevSibling`, `nextSibling`, `outlinePaths`, `indentSubtree`,
`outdentSubtree`, `moveSubtree`, `renumber` are named identically in Tasks
4–8. `foldKeys` / `foldTransaction` match between Tasks 8 and 10. The Rust
`folds` / `set_folds` / `move_folds` match between Task 9's module, tests,
rename call and commands. The `Editor` props `indent`, `folds`, `onFolds`
match between Tasks 3 and 10 and their use in `NoteView`.
