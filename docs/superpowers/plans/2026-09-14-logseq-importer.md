# The Logseq importer (0.4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One command that turns a Logseq graph folder into Obsidian-shaped
markdown inside the vault, never touching the original, and writes a report
naming everything it could not map.

**Architecture:** A `core::import` module with a pure mapping
(`logseq::map_graph`: a list of source files in, a list of vault files and a
report out) and one I/O function (`logseq::run`) that reads the graph, writes
the vault through `Vault::create`, copies `assets/` and writes the report.
The mapping is two passes: the first plans each file's destination and
collects every `id::` into a uuid → (page, anchor) table; the second rewrites
each page against that table. One Tauri command wraps `run`; one palette
command asks for the two folders.

**Tech Stack:** Rust 2024 (`regex`, `serde_yaml_ng`, `chrono`, `walkdir`,
`tempfile` for tests), Tauri 2 with `tauri-plugin-dialog`, Svelte 5,
TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-09-14-writing-first-direction-design.md`,
section *The Logseq importer* and *Testing → import*. `ROADMAP.md` step 0.4
is the summary. The first design,
`docs/superpowers/specs/2026-09-12-engram-notes-design.md`, governs the vault,
link resolution and frontmatter this plan writes into.

**Branch:** `master` is clean at `c6f6eca`. Create `feat/logseq-importer`
from it in Task 1 and stay on it.

---

## Global Constraints

- Rust 2024 edition, stable toolchain, `cargo fmt` and `cargo clippy
  --workspace -- -D warnings` clean before every commit.
- `core` has no Tauri dependency; the frontend never touches the filesystem.
- "The original is never touched": nothing under the graph folder is written,
  moved or deleted.
- "Everything the importer cannot map is written to `import-report.md` in the
  destination with file and line, so nothing is lost silently."
- "The importer is a `core` module with no I/O in its mapping, tested on
  fixture graphs, and it is idempotent on its own output."
- Files already in the vault are never overwritten; a collision is reported
  and the source file is skipped. The report itself is the one file the
  importer overwrites, since it wrote it.
- Tests run without a model, a window or the network.
- Comments short, saying why. Conventional commit subjects under 72
  characters.
- `cargo test --workspace` and `cd ui && pnpm check && pnpm test` clean
  before every commit.
- Every commit message ends with these two lines:
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01WhUpiBA17z7iDWXX5FT9a7`.

## Decisions the spec leaves open

Recorded here so the executor does not re-decide them.

- **Journals go to the vault's daily-notes folder** (`daily_notes.folder`,
  named with `daily_notes.format`), not under the destination. A journal is
  a daily note, and a daily note lives where Ctrl+D looks. Everything else
  goes under the destination; the destination may be the vault root.
- **Link names in rewritten references** are the page's basename, or its
  vault path without `.md` when two imported pages share a basename. This is
  what Obsidian's own resolver prefers (shortest path wins).
- **Anchors** are the first eight characters of the uuid without dashes,
  extended to twelve, sixteen or thirty-two when a page already uses that
  anchor. Short enough to read past, derived so a re-import is stable.
- **Indentation** is read as tabs or as runs of `editor.indent` spaces, so
  the importer's own output parses back to the same outline.
- **Asset links are left as written.** `![x](../assets/pic.png)` resolves in
  this app by basename already. Only Logseq's `{:height H, :width W}` suffix
  is rewritten, to Obsidian's `![x|WxH](...)`.
- **Frontmatter typing:** `tags::` and `alias::` become lists (`alias` is
  written as Obsidian's `aliases`), with `[[ ]]` and `#` stripped from each
  entry; a value that reads as a bool or a number is written as one; every
  other value is a string as written. `title::` renames the file and is
  dropped from the frontmatter; a title that cannot be a file name stays a
  property and is reported.
- **Reported but kept as text:** block properties other than `id::` and
  `collapsed::`, priorities `[#A]`, `SCHEDULED:` / `DEADLINE:` lines,
  `:LOGBOOK:` blocks, `CANCELED` / `CANCELLED` markers, references to a uuid
  the graph does not define, files that are not `pages/**/*.md`,
  `journals/*.md` or `assets/**`. Dropped and reported: nothing. Dropped
  silently: `collapsed::` only, as the spec says.

---

## File structure

| file | responsibility |
| --- | --- |
| `core/src/import/mod.rs` (new) | The module: `pub mod logseq`, re-exports `Report` and `Unmapped`. |
| `core/src/import/report.rs` (new) | `Report`: a list of `(file, line, what)` and its markdown rendering. |
| `core/src/import/logseq/mod.rs` (new) | `SourceFile`, `Mapped`, `Options`, `Output`, `Summary`; `map_graph` (paths, link names, the id table, rendering) and `run` (I/O). |
| `core/src/import/logseq/page.rs` (new) | A Logseq page as page properties, preamble and blocks; `parse` and `render`. No knowledge of references. |
| `core/src/import/logseq/inline.rs` (new) | Line-level rewrites: `((uuid))`, `{{embed}}`, `#[[tag]]`, task markers, image size hints, and what each reports. |
| `core/src/lib.rs` | `pub mod import;` |
| `core/fixtures/logseq/` (new) | A small Logseq graph exercising every rule. |
| `core/fixtures/logseq-expected/` (new) | What the graph maps to, one file per output. |
| `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` | `import_logseq(source, dest)`. |
| `ui/src/lib/api.ts` | `ImportSummary`, `importLogseq`. |
| `ui/src/lib/commands.ts` | *Import: Logseq graph*: two folder dialogs, then the report opens. |
| `docs/import.md` (new) | The mapping and the report, for users. |
| `docs/smoke.md`, `ROADMAP.md`, `README.md` | Smoke lines 63–67; 0.4 to 🔶; one line in the feature list. |

---

### Task 1: The branch, the module and the report

**Files:**
- Create: `core/src/import/mod.rs`, `core/src/import/report.rs`
- Modify: `core/src/lib.rs`

**Interfaces:**
- Produces: `import::Report { entries: Vec<Unmapped> }` with
  `note(&mut self, file: &str, line: usize, what: impl Into<String>)`,
  `is_empty()`, `render(&self, summary: &str) -> String`;
  `import::Unmapped { file, line, what }` (`line` 1-based, `0` = whole file).

- [ ] **Step 1: Create the branch**

```bash
cd /home/user01/Projekte/engram-notes && git checkout -b feat/logseq-importer
```

- [ ] **Step 2: Write the failing tests**

`core/src/import/report.rs`:

```rust
//! What an import could not map, and the markdown that names it.

use std::fmt::Write as _;

/// One construct kept as written or left out. `line` is 1-based; `0` means
/// the whole file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unmapped {
    pub file: String,
    pub line: usize,
    pub what: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub entries: Vec<Unmapped>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_grouped_by_file_in_line_order() {
        let mut r = Report::default();
        r.note("pages/b.md", 7, "priority `[#A]` kept as text");
        r.note("pages/a.md", 0, "not imported");
        r.note("pages/b.md", 2, "block property `x:: 1` kept as text");
        let s = r.render("Imported 2 pages.");
        assert_eq!(
            s,
            "# Import report\n\nImported 2 pages.\n\n## Not mapped\n\n### `pages/a.md`\n\n- not imported\n\n### `pages/b.md`\n\n- line 2: block property `x:: 1` kept as text\n- line 7: priority `[#A]` kept as text\n"
        );
    }

    #[test]
    fn empty_report_says_so() {
        let s = Report::default().render("Imported 1 page.");
        assert_eq!(s, "# Import report\n\nImported 1 page.\n\nEverything was mapped.\n");
        assert!(Report::default().is_empty());
    }
}
```

`core/src/import/mod.rs`:

```rust
//! Bringing another tool's notes into the vault. A mapping is pure and
//! tested on a fixture graph; only its `run` touches the disk.

pub mod logseq;
mod report;

pub use report::{Report, Unmapped};
```

For this task, leave `pub mod logseq;` out until Task 2 creates it. Add
`pub mod import;` to `core/src/lib.rs` in alphabetical order (after `graph`).

- [ ] **Step 3: Run to see them fail**

Run: `cargo test -p engram-notes-core import::report`
Expected: compile error, `no method named note`.

- [ ] **Step 4: Implement**

Append to `report.rs` above the tests:

```rust
impl Report {
    pub fn note(&mut self, file: &str, line: usize, what: impl Into<String>) {
        self.entries.push(Unmapped {
            file: file.to_owned(),
            line,
            what: what.into(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Markdown: the summary, then every entry under its file in line order.
    pub fn render(&self, summary: &str) -> String {
        let mut out = format!("# Import report\n\n{summary}\n");
        if self.entries.is_empty() {
            out.push_str("\nEverything was mapped.\n");
            return out;
        }
        out.push_str("\n## Not mapped\n");
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        let mut current: Option<&str> = None;
        for e in &sorted {
            if current != Some(e.file.as_str()) {
                let _ = write!(out, "\n### `{}`\n\n", e.file);
                current = Some(&e.file);
            }
            if e.line == 0 {
                let _ = writeln!(out, "- {}", e.what);
            } else {
                let _ = writeln!(out, "- line {}: {}", e.line, e.what);
            }
        }
        out
    }
}
```

- [ ] **Step 5: Run, format, lint**

Run: `cargo test -p engram-notes-core import::report && cargo fmt && cargo clippy --workspace -- -D warnings`
Expected: 2 passed, no warnings.

- [ ] **Step 6: Commit**

```bash
git add core/src/import core/src/lib.rs
git commit -m "feat(import): report of what an import could not map"
```

---

### Task 2: A Logseq page as blocks

**Files:**
- Create: `core/src/import/logseq/mod.rs` (stub: `pub mod page;`), `core/src/import/logseq/page.rs`
- Modify: `core/src/import/mod.rs` (add `pub mod logseq;`)

**Interfaces:**
- Produces:
  ```rust
  pub struct Page { pub properties: Vec<(String, String)>, pub preamble: Vec<String>, pub blocks: Vec<Block> }
  pub struct Block { pub line: usize, pub depth: usize, pub head: String, pub body: Vec<Line> }
  pub enum Line { Text(String), Property { line: usize, key: String, value: String } }
  pub fn parse(text: &str, indent: usize) -> Page
  pub fn render(page: &Page, frontmatter: &str, indent: usize) -> String
  pub fn is_property(line: &str) -> Option<(String, String)>
  ```
  `render` takes the frontmatter block already rendered (empty string for
  none) because the YAML typing rules belong to the mapper, not the page.

- [ ] **Step 1: Write the failing tests**

`core/src/import/logseq/page.rs`, tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn text_lines(b: &Block) -> Vec<&str> {
        b.body
            .iter()
            .filter_map(|l| match l {
                Line::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn page_properties_then_blocks_with_tabs() {
        let src = "title:: My Page\ntags:: a, b\n\n- first\n\tid:: 64f1a2b3-0000-4000-8000-000000000001\n\t- child\n\t  continued\n\t\t- grandchild\n- second";
        let p = parse(src, 2);
        assert_eq!(p.properties, vec![("title".into(), "My Page".into()), ("tags".into(), "a, b".into())]);
        assert_eq!(p.preamble, vec![""]);
        let depths: Vec<_> = p.blocks.iter().map(|b| (b.depth, b.head.as_str(), b.line)).collect();
        assert_eq!(depths, vec![(0, "first", 4), (1, "child", 6), (2, "grandchild", 8), (0, "second", 9)]);
        assert_eq!(
            p.blocks[0].body,
            vec![Line::Property { line: 5, key: "id".into(), value: "64f1a2b3-0000-4000-8000-000000000001".into() }]
        );
        assert_eq!(text_lines(&p.blocks[1]), vec!["continued"]);
    }

    #[test]
    fn first_block_of_properties_is_the_page_properties() {
        let p = parse("- title:: Old Style\n  public:: true\n- body", 2);
        assert_eq!(p.properties, vec![("title".into(), "Old Style".into()), ("public".into(), "true".into())]);
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0].head, "body");
    }

    #[test]
    fn frontmatter_and_prose_are_preamble() {
        let p = parse("---\ntitle: X\n---\n\nprose\n- item", 2);
        assert!(p.properties.is_empty());
        assert_eq!(p.preamble, vec!["---", "title: X", "---", "", "prose"]);
        assert_eq!(p.blocks[0].head, "item");
    }

    #[test]
    fn spaces_count_as_levels_and_fences_do_not_start_blocks() {
        let p = parse("- a\n  - b\n    ```\n    - not a block\n    ```\n    - c", 2);
        let heads: Vec<_> = p.blocks.iter().map(|b| (b.depth, b.head.as_str())).collect();
        assert_eq!(heads, vec![(0, "a"), (1, "b"), (2, "c")]);
        assert_eq!(text_lines(&p.blocks[1]), vec!["```", "- not a block", "```"]);
    }

    #[test]
    fn render_writes_spaces_and_keeps_order() {
        let p = parse("- first\n\tid:: u\n\tfoo:: bar\n\t- child\n\t  continued\n\t\t- grandchild", 2);
        let out = render(&p, "", 2);
        assert_eq!(out, "- first\n  id:: u\n  foo:: bar\n  - child\n    continued\n    - grandchild\n");
        let again = parse(&out, 2);
        assert_eq!(render(&again, "", 2), out);
    }

    #[test]
    fn render_puts_frontmatter_first() {
        let p = parse("title:: T\n\n- a", 2);
        assert_eq!(render(&p, "---\ntitle: T\n---\n", 2), "---\ntitle: T\n---\n\n- a\n");
    }

    #[test]
    fn empty_item_and_crlf() {
        let p = parse("-\r\n- x\r\n", 2);
        assert_eq!(p.blocks[0].head, "");
        assert_eq!(p.blocks[1].head, "x");
        assert_eq!(render(&p, "", 2), "-\n- x\n");
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p engram-notes-core import::logseq::page`
Expected: compile errors for the missing items.

- [ ] **Step 3: Implement**

`core/src/import/logseq/page.rs`:

```rust
//! A Logseq page: `key:: value` lines at the top, then an outline of `- `
//! blocks indented with tabs, each block's extra lines indented two more.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Text(String),
    Property { line: usize, key: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// 1-based line of the bullet in the source.
    pub line: usize,
    pub depth: usize,
    /// The first line, after the bullet.
    pub head: String,
    pub body: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    pub properties: Vec<(String, String)>,
    /// Lines before the first block that are not page properties, verbatim.
    pub preamble: Vec<String>,
    pub blocks: Vec<Block>,
}

static PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_.\-]+)::\s?(.*)$").unwrap());

pub fn is_property(line: &str) -> Option<(String, String)> {
    let c = PROPERTY.captures(line)?;
    Some((c[1].to_owned(), c[2].trim_end().to_owned()))
}

/// Leading whitespace as outline levels: a tab is one, `indent` spaces are one.
fn level(ws: &str, indent: usize) -> usize {
    let mut depth = 0;
    let mut spaces = 0;
    for ch in ws.chars() {
        if ch == '\t' {
            depth += 1;
            spaces = 0;
        } else {
            spaces += 1;
            if spaces == indent {
                depth += 1;
                spaces = 0;
            }
        }
    }
    depth
}

fn bullet(line: &str) -> Option<(&str, &str)> {
    let ws_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[ws_len..];
    if rest == "-" {
        Some((&line[..ws_len], ""))
    } else {
        rest.strip_prefix("- ").map(|h| (&line[..ws_len], h))
    }
}

/// A continuation line without its block's indentation and the two spaces
/// Logseq adds under a bullet. What is left is the line as the writer saw it.
fn continuation(line: &str, depth: usize, indent: usize) -> &str {
    let mut rest = line;
    let mut levels = 0;
    while levels < depth {
        if let Some(r) = rest.strip_prefix('\t') {
            rest = r;
        } else if rest.starts_with(&" ".repeat(indent)) {
            rest = &rest[indent..];
        } else {
            break;
        }
        levels += 1;
    }
    rest.strip_prefix("  ").unwrap_or(rest)
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

pub fn parse(text: &str, indent: usize) -> Page {
    let mut page = Page::default();
    let mut in_fence = false;
    let mut in_frontmatter = false;
    for (i, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        let n = i + 1;
        if page.blocks.is_empty() {
            if i == 0 && line == "---" {
                in_frontmatter = true;
            } else if in_frontmatter && line == "---" {
                in_frontmatter = false;
            }
            if in_frontmatter {
                page.preamble.push(line.to_owned());
                continue;
            }
        }
        let current = page.blocks.last_mut();
        if !in_fence
            && let Some((ws, head)) = bullet(line)
        {
            page.blocks.push(Block {
                line: n,
                depth: level(ws, indent),
                head: head.to_owned(),
                body: Vec::new(),
            });
            in_fence = is_fence(head);
            continue;
        }
        match current {
            Some(b) => {
                let c = continuation(line, b.depth, indent);
                if is_fence(c) {
                    in_fence = !in_fence;
                }
                b.body.push(match is_property(c) {
                    Some((key, value)) if !in_fence => Line::Property { line: n, key, value },
                    _ => Line::Text(c.to_owned()),
                });
            }
            None => match is_property(line) {
                Some(kv) if page.preamble.is_empty() => page.properties.push(kv),
                _ => page.preamble.push(line.to_owned()),
            },
        }
    }
    // A trailing newline is not an empty last line.
    if let Some(b) = page.blocks.last_mut()
        && b.body.last() == Some(&Line::Text(String::new()))
    {
        b.body.pop();
    } else if page.blocks.is_empty() && page.preamble.last().is_some_and(|l| l.is_empty()) {
        page.preamble.pop();
    }
    lift_property_block(&mut page);
    page
}

/// Older graphs wrote the page properties as the first block; they are the
/// page's when nothing else is in that block.
fn lift_property_block(page: &mut Page) {
    if !page.properties.is_empty() || !page.preamble.iter().all(|l| l.is_empty()) {
        return;
    }
    let Some(first) = page.blocks.first() else { return };
    let Some(head) = is_property(&first.head) else { return };
    if first.depth != 0
        || !first.body.iter().all(|l| matches!(l, Line::Property { .. }))
    {
        return;
    }
    let first = page.blocks.remove(0);
    page.properties.push(head);
    for l in first.body {
        if let Line::Property { key, value, .. } = l {
            page.properties.push((key, value));
        }
    }
}

pub fn render(page: &Page, frontmatter: &str, indent: usize) -> String {
    let mut out = String::from(frontmatter);
    for l in &page.preamble {
        out.push_str(l);
        out.push('\n');
    }
    for b in &page.blocks {
        let pad = " ".repeat(b.depth * indent);
        out.push_str(&pad);
        if b.head.is_empty() {
            out.push('-');
        } else {
            out.push_str("- ");
            out.push_str(&b.head);
        }
        out.push('\n');
        for l in &b.body {
            let text = match l {
                Line::Text(t) => t.clone(),
                Line::Property { key, value, .. } => format!("{key}:: {value}"),
            };
            if !text.is_empty() {
                out.push_str(&pad);
                out.push_str("  ");
            }
            out.push_str(&text);
            out.push('\n');
        }
    }
    out
}
```

Note the `is_fence(head)` after a bullet: a block whose head opens a fence
(`- ```rust`) puts the following lines inside it.

- [ ] **Step 4: Run, format, lint**

Run: `cargo test -p engram-notes-core import::logseq::page && cargo fmt && cargo clippy --workspace -- -D warnings`
Expected: 7 passed. If `let` chains are rejected, the toolchain is older than
1.88; split them into nested `if`s.

- [ ] **Step 5: Commit**

```bash
git add core/src/import
git commit -m "feat(import): parse and render a Logseq page as blocks"
```

---

### Task 3: Inline rewrites

**Files:**
- Create: `core/src/import/logseq/inline.rs`
- Modify: `core/src/import/logseq/mod.rs` (add `pub mod inline;`)

**Interfaces:**
- Consumes: `import::Report`.
- Produces:
  ```rust
  pub type Refs = std::collections::HashMap<String, (String, String)>; // uuid -> (link name, anchor)
  pub fn rewrite(text: &str, refs: &Refs, file: &str, line: usize, report: &mut Report) -> String
  pub fn task(head: &str, file: &str, line: usize, report: &mut Report) -> String
  pub fn note_kept(text: &str, file: &str, line: usize, report: &mut Report)
  ```

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> Refs {
        let mut r = Refs::new();
        r.insert("64f1a2b3-0000-4000-8000-000000000001".into(), ("Alpha".into(), "64f1a2b3".into()));
        r
    }

    #[test]
    fn references_and_embeds() {
        let mut rep = Report::default();
        let s = rewrite(
            "see ((64f1a2b3-0000-4000-8000-000000000001)) and {{embed ((64f1a2b3-0000-4000-8000-000000000001))}} and {{embed [[Other Page]]}}",
            &refs(), "pages/x.md", 3, &mut rep,
        );
        assert_eq!(s, "see [[Alpha#^64f1a2b3]] and ![[Alpha#^64f1a2b3]] and ![[Other Page]]");
        assert!(rep.is_empty());
    }

    #[test]
    fn unknown_reference_is_kept_and_reported() {
        let mut rep = Report::default();
        let s = rewrite("((deadbeef-0000-4000-8000-000000000009))", &refs(), "pages/x.md", 3, &mut rep);
        assert_eq!(s, "((deadbeef-0000-4000-8000-000000000009))");
        assert_eq!(rep.entries[0].what, "reference `((deadbeef-0000-4000-8000-000000000009))` has no block in this graph, kept as text");
        assert_eq!((rep.entries[0].file.as_str(), rep.entries[0].line), ("pages/x.md", 3));
    }

    #[test]
    fn multi_word_tag_becomes_a_link_and_single_word_stays() {
        let mut rep = Report::default();
        let s = rewrite("#[[multi word]] #single", &refs(), "f", 1, &mut rep);
        assert_eq!(s, "[[multi word]] #single");
    }

    #[test]
    fn image_size_hint_becomes_obsidians() {
        let mut rep = Report::default();
        let s = rewrite("![pic](../assets/pic.png){:height 200, :width 300}", &refs(), "f", 1, &mut rep);
        assert_eq!(s, "![pic|300x200](../assets/pic.png)");
        let s = rewrite("![pic](../assets/pic.png){:width 300, :height 200}", &refs(), "f", 1, &mut rep);
        assert_eq!(s, "![pic|300x200](../assets/pic.png)");
    }

    #[test]
    fn task_markers() {
        let mut rep = Report::default();
        assert_eq!(task("TODO buy milk", "f", 1, &mut rep), "[ ] buy milk");
        assert_eq!(task("DONE buy milk", "f", 1, &mut rep), "[x] buy milk");
        assert_eq!(task("DOING buy milk", "f", 1, &mut rep), "[ ] DOING buy milk");
        assert_eq!(task("LATER", "f", 1, &mut rep), "[ ] LATER");
        assert_eq!(task("TODOS are plural", "f", 1, &mut rep), "TODOS are plural");
        assert_eq!(task("[ ] already", "f", 1, &mut rep), "[ ] already");
        assert!(rep.is_empty());
        assert_eq!(task("CANCELED x", "f", 4, &mut rep), "CANCELED x");
        assert_eq!(rep.entries[0].what, "task marker `CANCELED` has no checkbox form, kept as text");
        assert_eq!(task("TODO [#A] urgent", "f", 5, &mut rep), "[ ] [#A] urgent");
        assert_eq!(rep.entries[1].what, "priority `[#A]` kept as text");
    }

    #[test]
    fn scheduling_and_logbook_are_noted() {
        let mut rep = Report::default();
        note_kept("SCHEDULED: <2026-09-20 Sun>", "f", 2, &mut rep);
        note_kept("DEADLINE: <2026-09-21 Mon>", "f", 3, &mut rep);
        note_kept(":LOGBOOK:", "f", 4, &mut rep);
        note_kept("CLOCK: [2026-09-14 Sun 10:00:00]", "f", 5, &mut rep);
        let whats: Vec<_> = rep.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(whats, vec!["`SCHEDULED:` kept as text", "`DEADLINE:` kept as text", "`:LOGBOOK:` kept as text"]);
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p engram-notes-core import::logseq::inline`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! Logseq's inline syntax into Obsidian's, one line at a time. Anything with
//! no Obsidian form stays as written and goes into the report.

use crate::import::Report;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::LazyLock;

/// uuid → (page link name, anchor without `^`).
pub type Refs = HashMap<String, (String, String)>;

const UUID: &str = r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

static EMBED_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\{{\{{embed \(\(({UUID})\)\)\}}\}}")).unwrap());
static EMBED_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{embed \[\[([^\]]+)\]\]\}\}").unwrap());
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"\(\(({UUID})\)\)")).unwrap());
static TAG_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[\[([^\]]+)\]\]").unwrap());
static IMAGE_SIZE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!\[([^\]]*)\]\(([^)]*)\)\{:(height|width) (\d+), :(height|width) (\d+)\}").unwrap()
});
static TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(TODO|DONE|DOING|LATER|NOW|WAITING|CANCELED|CANCELLED)(?: |$)").unwrap());
static PRIORITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[#[A-C]\]").unwrap());

fn link(refs: &Refs, uuid: &str, embed: bool) -> Option<String> {
    let (page, anchor) = refs.get(&uuid.to_lowercase())?;
    let bang = if embed { "!" } else { "" };
    Some(format!("{bang}[[{page}#^{anchor}]]"))
}

pub fn rewrite(text: &str, refs: &Refs, file: &str, line: usize, report: &mut Report) -> String {
    let mut unknown = Vec::new();
    let mut resolve = |c: &Captures, embed: bool| match link(refs, &c[1], embed) {
        Some(l) => l,
        None => {
            unknown.push(c[0].to_owned());
            c[0].to_owned()
        }
    };
    let s = EMBED_REF.replace_all(text, |c: &Captures| resolve(c, true));
    let s = REF.replace_all(&s, |c: &Captures| resolve(c, false));
    for u in unknown {
        report.note(file, line, format!("reference `{u}` has no block in this graph, kept as text"));
    }
    let s = EMBED_PAGE.replace_all(&s, "![[$1]]");
    let s = TAG_LINK.replace_all(&s, "[[$1]]");
    let s = IMAGE_SIZE.replace_all(&s, |c: &Captures| {
        let (w, h) = if &c[3] == "width" { (&c[4], &c[6]) } else { (&c[6], &c[4]) };
        format!("![{}|{w}x{h}]({})", &c[1], &c[2])
    });
    s.into_owned()
}

/// A block's first line: Logseq's marker becomes Obsidian's checkbox.
pub fn task(head: &str, file: &str, line: usize, report: &mut Report) -> String {
    let out = match TASK.captures(head) {
        Some(c) => {
            let marker = &c[1];
            let rest = &head[c[0].len()..];
            match marker {
                "TODO" => format!("[ ] {rest}"),
                "DONE" => format!("[x] {rest}"),
                "CANCELED" | "CANCELLED" => {
                    report.note(file, line, format!("task marker `{marker}` has no checkbox form, kept as text"));
                    head.to_owned()
                }
                _ if rest.is_empty() => format!("[ ] {marker}"),
                _ => format!("[ ] {marker} {rest}"),
            }
        }
        None => head.to_owned(),
    };
    if let Some(p) = PRIORITY.find(&out) {
        report.note(file, line, format!("priority `{}` kept as text", p.as_str()));
    }
    out.trim_end().to_owned()
}

/// Lines Obsidian has no reading of. `CLOCK:` lines are inside the logbook
/// and are covered by its entry.
pub fn note_kept(text: &str, file: &str, line: usize, report: &mut Report) {
    for prefix in ["SCHEDULED:", "DEADLINE:", ":LOGBOOK:"] {
        if text.starts_with(prefix) {
            report.note(file, line, format!("`{prefix}` kept as text"));
        }
    }
}
```

- [ ] **Step 4: Run, format, lint**

Run: `cargo test -p engram-notes-core import::logseq::inline && cargo fmt && cargo clippy --workspace -- -D warnings`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add core/src/import
git commit -m "feat(import): Logseq references, embeds, tags and tasks"
```

---

### Task 4: The mapping, on a fixture graph

**Files:**
- Modify: `core/src/import/logseq/mod.rs`
- Create: `core/fixtures/logseq/**`, `core/fixtures/logseq-expected/**`

**Interfaces:**
- Consumes: `page::{parse, render, Page, Block, Line}`, `inline::{rewrite, task, note_kept, Refs}`, `config::daily_note_path`, `Report`.
- Produces:
  ```rust
  pub struct SourceFile { pub path: String, pub text: String }   // graph-relative, '/'
  pub struct Mapped { pub path: String, pub text: String }        // vault-relative, '/'
  pub struct Options<'a> { pub dest: &'a str, pub config: &'a AppConfig }
  pub struct Output { pub pages: Vec<Mapped>, pub journals: usize, pub report: Report }
  pub fn map_graph(files: &[SourceFile], opts: &Options) -> Output
  ```

- [ ] **Step 1: Write the fixture graph**

`core/fixtures/logseq/logseq/config.edn`:

```
{:meta/version 1}
```

`core/fixtures/logseq/pages/Project___Alpha.md`:

```
title:: Project/Alpha
tags:: [[work]], #important
alias:: Alpha, The Alpha project
public:: true
rating:: 5

- The plan
  id:: 64f1a2b3-0000-4000-8000-000000000001
  collapsed:: true
	- TODO Write the spec
	  SCHEDULED: <2026-09-20 Sun>
	- DONE Pick a name [#A]
	- DOING Draft the outline #[[needs review]] #draft
	  :LOGBOOK:
	  CLOCK: [2026-09-14 Sun 10:00:00]--[2026-09-14 Sun 10:30:00] =>  00:30:00
	  :END:
- Notes
  id:: 64f1a2b3-0000-4000-8000-000000000002
  logseq.order-list-type:: number
	- ```rust
	  let x = ((64f1a2b3-0000-4000-8000-000000000001));
	  ```
	- ![diagram](../assets/diagram.png){:height 200, :width 300}
- CANCELED Old idea
```

`core/fixtures/logseq/pages/notes.md`:

```
- See ((64f1a2b3-0000-4000-8000-000000000001)) and ((64f1a2b3-0000-4000-8000-00000000dead)).
- {{embed ((64f1a2b3-0000-4000-8000-000000000002))}}
- {{embed [[Project/Alpha]]}}
```

`core/fixtures/logseq/pages/renamed.md`:

```
title:: My Renamed Page

- body
```

`core/fixtures/logseq/pages/odd title.md`:

```
title:: what: is this?

- body
```

`core/fixtures/logseq/pages/legacy.md`:

```
- title:: Legacy
  public:: false
- first
```

`core/fixtures/logseq/pages/plain.md`:

```
Just prose, no bullets.

- and one item
```

`core/fixtures/logseq/journals/2026_09_14.md`:

```
- Started the import
  id:: 64f1a2b3-0000-4000-8000-000000000003
	- LATER Read ((64f1a2b3-0000-4000-8000-000000000003))
```

`core/fixtures/logseq/journals/notadate.md`:

```
- stray
```

`core/fixtures/logseq/assets/diagram.png`: one byte, `printf 'x' > ...`.

`core/fixtures/logseq/whiteboards/board.edn`:

```
{}
```

- [ ] **Step 2: Write the expected output**

`core/fixtures/logseq-expected/Project/Alpha.md`:

```
---
tags:
- work
- important
aliases:
- Alpha
- The Alpha project
public: true
rating: 5
---

- The plan ^64f1a2b3
  - [ ] Write the spec
    SCHEDULED: <2026-09-20 Sun>
  - [x] Pick a name [#A]
  - [ ] DOING Draft the outline [[needs review]] #draft
    :LOGBOOK:
    CLOCK: [2026-09-14 Sun 10:00:00]--[2026-09-14 Sun 10:30:00] =>  00:30:00
    :END:
- Notes ^64f1a2b30000
  logseq.order-list-type:: number
  - ```rust
    let x = ((64f1a2b3-0000-4000-8000-000000000001));
    ```
  - ![diagram|300x200](../assets/diagram.png)
- CANCELED Old idea
```

Wait — the second anchor. Both uuids start with `64f1a2b3`, so the second
gets twelve characters: `64f1a2b30000`. The expected file above is right.
Same for the journal's block, which is in another page and so keeps eight.

`core/fixtures/logseq-expected/notes.md`:

```
- See [[Alpha#^64f1a2b3]] and ((64f1a2b3-0000-4000-8000-00000000dead)).
- ![[Alpha#^64f1a2b30000]]
- ![[Project/Alpha]]
```

`core/fixtures/logseq-expected/My Renamed Page.md`:

```
- body
```

`core/fixtures/logseq-expected/odd title.md`:

```
---
title: 'what: is this?'
---

- body
```

`core/fixtures/logseq-expected/Legacy.md`:

```
---
public: false
---
- first
```

`core/fixtures/logseq-expected/plain.md`:

```
Just prose, no bullets.

- and one item
```

`core/fixtures/logseq-expected/Daily/2026-09-14.md`:

```
- Started the import ^64f1a2b3
  - [ ] LATER Read [[2026-09-14#^64f1a2b3]]
```

`core/fixtures/logseq-expected/journals/notadate.md`:

```
- stray
```

Check the YAML serde_yaml_ng produces for the quoted title when the test
runs; if it chooses double quotes, correct the fixture, not the code.

- [ ] **Step 3: Write the failing tests**

In `core/src/import/logseq/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn read_dir(root: &Path) -> Vec<SourceFile> {
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(root) {
            let e = e.unwrap();
            if !e.file_type().is_file() {
                continue;
            }
            let path = e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                out.push(SourceFile { path, text });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn fixture() -> (Vec<SourceFile>, Vec<SourceFile>) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        (read_dir(&root.join("logseq")), read_dir(&root.join("logseq-expected")))
    }

    fn map(files: &[SourceFile]) -> Output {
        let cfg = AppConfig::default();
        map_graph(files, &Options { dest: "", config: &cfg })
    }

    #[test]
    fn fixture_graph_maps_to_expected_files() {
        let (src, expected) = fixture();
        let out = map(&src);
        let mut got: Vec<_> = out.pages.iter().map(|m| (m.path.as_str(), m.text.as_str())).collect();
        got.sort();
        let want: Vec<_> = expected.iter().map(|m| (m.path.as_str(), m.text.as_str())).collect();
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.0, w.0);
            assert_eq!(g.1, w.1, "in {}", g.0);
        }
        assert_eq!(got.len(), want.len());
        assert_eq!(out.journals, 1);
    }

    #[test]
    fn report_names_every_unmapped_construct() {
        let (src, _) = fixture();
        let out = map(&src);
        let mut whats: Vec<_> = out.report.entries.iter().map(|e| format!("{}:{} {}", e.file, e.line, e.what)).collect();
        whats.sort();
        let expected = [
            "journals/notadate.md:0 journal name is not a date, kept under `journals/`",
            "pages/Project___Alpha.md:10 `:LOGBOOK:` kept as text",
            "pages/Project___Alpha.md:15 block property `logseq.order-list-type:: number` kept as text",
            "pages/Project___Alpha.md:20 task marker `CANCELED` has no checkbox form, kept as text",
            "pages/Project___Alpha.md:8 `SCHEDULED:` kept as text",
            "pages/Project___Alpha.md:9 priority `[#A]` kept as text",
            "pages/notes.md:1 reference `((64f1a2b3-0000-4000-8000-00000000dead))` has no block in this graph, kept as text",
            "pages/odd title.md:0 title `what: is this?` cannot be a file name, kept as a property",
            "whiteboards/board.edn:0 not imported",
        ];
        assert_eq!(whats, expected);
    }

    #[test]
    fn importing_the_output_changes_nothing() {
        let (src, _) = fixture();
        let first = map(&src);
        let again: Vec<SourceFile> = first.pages.iter().map(|m| SourceFile { path: m.path.clone(), text: m.text.clone() }).collect();
        let second = map(&again);
        assert_eq!(second.pages, first.pages);
    }

    #[test]
    fn destination_and_duplicate_basenames() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile { path: "pages/a___x.md".into(), text: "- one\n  id:: 64f1a2b3-0000-4000-8000-000000000001\n".into() },
            SourceFile { path: "pages/b___x.md".into(), text: "- ((64f1a2b3-0000-4000-8000-000000000001))\n".into() },
            SourceFile { path: "journals/2026_01_02.md".into(), text: "- j\n".into() },
        ];
        let out = map_graph(&files, &Options { dest: "Logseq/", config: &cfg });
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["Logseq/a/x.md", "Logseq/b/x.md", "Daily/2026-01-02.md"]);
        assert_eq!(out.pages[1].text, "- [[Logseq/a/x#^64f1a2b3]]\n");
    }

    #[test]
    fn title_renames_and_case_is_the_titles() {
        let cfg = AppConfig::default();
        let files = vec![SourceFile { path: "pages/my page.md".into(), text: "title:: My Page\n\n- x\n".into() }];
        let out = map_graph(&files, &Options { dest: "", config: &cfg });
        assert_eq!(out.pages[0].path, "My Page.md");
        assert_eq!(out.pages[0].text, "\n- x\n");
    }
}
```

The title test: `title:: My Page` followed by an empty line and the block;
the property lines go, the empty preamble line stays, so the text starts
with a newline. That is faithful and idempotent, and it is what the fixture
for `renamed.md` shows too — correct the expected `My Renamed Page.md` to
begin with an empty line: `\n- body\n`. Likewise `Legacy.md` has no blank
line because the legacy form had none.

- [ ] **Step 4: Run to see them fail**

Run: `cargo test -p engram-notes-core import::logseq::tests`
Expected: compile errors.

- [ ] **Step 5: Implement `map_graph`**

`core/src/import/logseq/mod.rs`:

```rust
//! A Logseq graph (`pages/`, `journals/`, `assets/`) into a folder of the
//! vault in Obsidian's shape. `map_graph` is pure; `run` reads and writes.

pub mod inline;
pub mod page;

use crate::config::{self, AppConfig};
use crate::import::Report;
use inline::Refs;
use page::{Line, Page};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Relative to the graph root, `/`-separated.
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// Relative to the vault root.
    pub path: String,
    pub text: String,
}

pub struct Options<'a> {
    /// Folder inside the vault, `""` for the root.
    pub dest: &'a str,
    pub config: &'a AppConfig,
}

#[derive(Debug, Default)]
pub struct Output {
    pub pages: Vec<Mapped>,
    pub journals: usize,
    pub report: Report,
}

struct Planned<'a> {
    src: &'a SourceFile,
    path: String,
    journal: bool,
    page: Page,
    /// Frontmatter keys and values, `title` already decided.
    properties: Vec<(String, String)>,
}

fn join(dest: &str, rest: &str) -> String {
    let dest = dest.trim_matches('/');
    if dest.is_empty() {
        rest.to_owned()
    } else {
        format!("{dest}/{rest}")
    }
}

/// A title Obsidian would accept as a file name, with `/` for folders.
fn usable_name(title: &str) -> bool {
    !title.is_empty()
        && !title.contains(['\\', ':', '*', '?', '"', '<', '>', '|', '#', '^', '['])
        && !title.starts_with('/')
        && !title.ends_with('/')
        && title
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != ".." && !seg.ends_with('.') && seg.trim() == seg)
}

fn page_name(rel: &str) -> String {
    let stem = rel.strip_suffix(".md").unwrap_or(rel);
    stem.replace("___", "/").replace("%2F", "/")
}

fn plan<'a>(f: &'a SourceFile, opts: &Options, report: &mut Report) -> Planned<'a> {
    let indent = opts.config.editor.indent;
    let mut page = page::parse(&f.text, indent);
    let mut properties = std::mem::take(&mut page.properties);
    if let Some(name) = f.path.strip_prefix("journals/") {
        let stem = name.strip_suffix(".md").unwrap_or(name);
        let path = match chrono::NaiveDate::parse_from_str(stem, "%Y_%m_%d") {
            Ok(date) => config::daily_note_path(opts.config, date),
            Err(_) => {
                report.note(&f.path, 0, "journal name is not a date, kept under `journals/`");
                join(opts.dest, &format!("journals/{name}"))
            }
        };
        return Planned { src: f, path, journal: true, page, properties };
    }
    let rel = f.path.strip_prefix("pages/").unwrap_or(&f.path);
    let mut name = page_name(rel);
    if let Some(i) = properties.iter().position(|(k, _)| k == "title") {
        let title = properties[i].1.clone();
        if usable_name(&title) {
            properties.remove(i);
            name = title;
        } else {
            report.note(&f.path, 0, format!("title `{title}` cannot be a file name, kept as a property"));
        }
    }
    Planned { src: f, path: join(opts.dest, &format!("{name}.md")), journal: false, page, properties }
}

fn anchor_for(uuid: &str, used: &mut HashSet<String>) -> String {
    let flat: String = uuid.chars().filter(|c| *c != '-').collect::<String>().to_lowercase();
    for len in [8, 12, 16, 32] {
        let a = flat[..len.min(flat.len())].to_owned();
        if used.insert(a.clone()) {
            return a;
        }
    }
    flat
}

/// Collects `id::` blocks into the table and stamps their anchors onto the
/// blocks, so pass two can render `^anchor` and rewrite `((uuid))`.
fn collect_ids(planned: &mut [Planned], refs: &mut Refs) {
    let mut stems: HashMap<String, usize> = HashMap::new();
    for p in planned.iter() {
        let stem = p.path.rsplit('/').next().unwrap_or(&p.path).trim_end_matches(".md").to_lowercase();
        *stems.entry(stem).or_default() += 1;
    }
    for p in planned.iter_mut() {
        let stem = p.path.rsplit('/').next().unwrap_or(&p.path).trim_end_matches(".md");
        let link = if stems[&stem.to_lowercase()] > 1 {
            p.path.trim_end_matches(".md").to_owned()
        } else {
            stem.to_owned()
        };
        let mut used = HashSet::new();
        for b in &mut p.page.blocks {
            let id = b.body.iter().find_map(|l| match l {
                Line::Property { key, value, .. } if key == "id" => Some(value.clone()),
                _ => None,
            });
            if let Some(uuid) = id {
                let anchor = anchor_for(&uuid, &mut used);
                refs.insert(uuid.to_lowercase(), (link.clone(), anchor.clone()));
                if b.head.trim_start().starts_with("```") {
                    // An anchor after a fence opener would be part of the info string.
                    continue;
                }
                b.head = format!("{} ^{anchor}", b.head).trim_start().to_owned();
            }
        }
    }
}

fn yaml_value(key: &str, value: &str) -> serde_json::Value {
    use serde_json::Value;
    if key == "tags" || key == "alias" || key == "aliases" {
        let items: Vec<Value> = value
            .split(',')
            .map(|s| s.trim().trim_start_matches('#').trim_start_matches("[[").trim_end_matches("]]").trim())
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_owned()))
            .collect();
        return Value::Array(items);
    }
    if let Ok(b) = value.parse::<bool>() {
        return Value::Bool(b);
    }
    if let Ok(n) = value.parse::<i64>() {
        return Value::Number(n.into());
    }
    if let Ok(f) = value.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return Value::Number(n);
    }
    Value::String(value.to_owned())
}

fn frontmatter(properties: &[(String, String)]) -> String {
    if properties.is_empty() {
        return String::new();
    }
    let mut map = serde_json::Map::new();
    for (k, v) in properties {
        let key = if k == "alias" { "aliases" } else { k.as_str() };
        map.insert(key.to_owned(), yaml_value(k, v));
    }
    let yaml = serde_yaml_ng::to_string(&map).unwrap_or_default();
    format!("---\n{yaml}---\n")
}

fn render(p: &Planned, refs: &Refs, indent: usize, report: &mut Report) -> String {
    let file = &p.src.path;
    let mut page = p.page.clone();
    for b in &mut page.blocks {
        let mut in_fence = b.head.trim_start().starts_with("```");
        if !in_fence {
            b.head = inline::task(&b.head, file, b.line, report);
            b.head = inline::rewrite(&b.head, refs, file, b.line, report);
        }
        let mut line = b.line;
        b.body.retain(|l| !matches!(l, Line::Property { key, .. } if key == "id" || key == "collapsed"));
        for l in &mut b.body {
            line += 1;
            match l {
                Line::Text(t) => {
                    if t.trim_start().starts_with("```") {
                        in_fence = !in_fence;
                    } else if !in_fence {
                        inline::note_kept(t, file, line, report);
                        *t = inline::rewrite(t, refs, file, line, report);
                    }
                }
                Line::Property { line: n, key, value } => {
                    report.note(file, *n, format!("block property `{key}:: {value}` kept as text"));
                }
            }
        }
    }
    page::render(&page, &frontmatter(&p.properties), indent)
}

pub fn map_graph(files: &[SourceFile], opts: &Options) -> Output {
    let mut out = Output::default();
    let mut planned = Vec::new();
    for f in files {
        let is_md = f.path.to_lowercase().ends_with(".md");
        let in_scope = f.path.starts_with("pages/") || f.path.starts_with("journals/");
        if !is_md || !in_scope {
            out.report.note(&f.path, 0, "not imported");
            continue;
        }
        planned.push(plan(f, opts, &mut out.report));
    }
    let mut refs = Refs::new();
    collect_ids(&mut planned, &mut refs);
    let indent = opts.config.editor.indent;
    for p in &planned {
        let text = render(p, &refs, indent, &mut out.report);
        out.journals += p.journal as usize;
        out.pages.push(Mapped { path: p.path.clone(), text });
    }
    out
}
```

Two things to watch while making the tests pass:

- The body-line numbering in `render` counts removed `id::` and
  `collapsed::` lines wrongly, since `retain` runs first. Number the lines
  before removing: iterate `b.body` with its original index for reports,
  then drop the two keys. The simplest correct shape is to keep the
  `Line::Property { line, .. }` numbers (they carry their own) and, for
  `Line::Text`, compute the number from the position in the original body.
  Make `Line::Text` carry no number and instead walk the original body with
  `enumerate` before the `retain`, collecting the rewritten lines into a new
  `Vec<Line>`. Rewrite the loop that way rather than patching.
- On the second pass over its own output (the idempotence test), input paths
  are not under `pages/`; `map_graph` must treat any `.md` outside
  `journals/` as a page. Change `in_scope` to
  `!f.path.starts_with("assets/") && !f.path.starts_with("logseq/")` and let
  non-`.md` files be the "not imported" ones. `whiteboards/board.edn` still
  reports.
- `logseq.order-list-type:: number` is a `Line::Property` and stays in the
  body verbatim; the expected fixture shows it. `SCHEDULED:` is a
  `Line::Text` (no `::`).

- [ ] **Step 6: Run, fix the fixtures where serde_yaml_ng's quoting differs, format, lint**

Run: `cargo test -p engram-notes-core import:: && cargo fmt && cargo clippy --workspace -- -D warnings`
Expected: all import tests pass.

- [ ] **Step 7: Commit**

```bash
git add core/src/import core/fixtures
git commit -m "feat(import): map a Logseq graph to Obsidian-shaped files"
```

---

### Task 5: `run`: read the graph, write the vault, copy assets, write the report

**Files:**
- Modify: `core/src/import/logseq/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(serde::Serialize)] pub struct Summary { pub pages: usize, pub journals: usize, pub assets: usize, pub skipped: usize, pub unmapped: usize, pub report: String /* vault path */ }
  pub fn run(vault: &Vault, cfg: &AppConfig, graph: &Path, dest: &str) -> Result<Summary>
  ```

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn run_writes_pages_assets_and_report_and_never_overwrites() {
        let graph = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/logseq");
        let d = tempfile::tempdir().unwrap();
        let vault = crate::vault::Vault::open(d.path()).unwrap();
        vault.create("notes.md", "mine").unwrap();
        let cfg = AppConfig::default();
        let s = run(&vault, &cfg, &graph, "In/").unwrap();
        assert_eq!((s.pages, s.journals, s.assets, s.skipped), (6, 1, 1, 0));
        assert_eq!(s.report, "In/import-report.md");
        assert!(d.path().join("In/Project/Alpha.md").is_file());
        assert!(d.path().join("Daily/2026-09-14.md").is_file());
        assert!(d.path().join("In/assets/diagram.png").is_file());
        assert_eq!(vault.read("notes.md").unwrap(), "mine");
        let report = vault.read("In/import-report.md").unwrap();
        assert!(report.contains("### `whiteboards/board.edn`"), "{report}");
        assert!(!report.contains("logseq/config.edn"), "{report}");
        // Running again writes nothing and says why.
        let s = run(&vault, &cfg, &graph, "In/").unwrap();
        assert_eq!((s.pages, s.journals, s.assets), (0, 0, 0));
        assert_eq!(s.skipped, 8);
        let report = vault.read("In/import-report.md").unwrap();
        assert!(report.contains("already in the vault, not written"), "{report}");
        assert!(std::fs::read_dir(&graph).is_ok());
    }

    #[test]
    fn run_refuses_a_folder_that_is_not_a_graph() {
        let d = tempfile::tempdir().unwrap();
        let vault = crate::vault::Vault::open(d.path()).unwrap();
        let err = run(&vault, &AppConfig::default(), d.path(), "").unwrap_err();
        assert!(err.to_string().contains("not a Logseq graph"), "{err}");
    }
```

`skipped` on the second run: 7 pages/journals (`notes.md` is under `In/`
now, so it is a collision with the first run's copy) plus the asset = 8.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p engram-notes-core import::logseq::tests::run_`
Expected: compile error, `run` not found.

- [ ] **Step 3: Implement**

```rust
use crate::vault::Vault;
use crate::{Error, Result};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    pub pages: usize,
    pub journals: usize,
    pub assets: usize,
    /// Files left alone because the vault already had them.
    pub skipped: usize,
    pub unmapped: usize,
    pub report: String,
}

const IGNORED: [&str; 3] = ["logseq", "bak", "version-files"];

pub fn run(vault: &Vault, cfg: &AppConfig, graph: &Path, dest: &str) -> Result<Summary> {
    if !graph.join("pages").is_dir() && !graph.join("journals").is_dir() {
        return Err(Error::NotFound(format!(
            "{}: not a Logseq graph (no pages/ or journals/)",
            graph.display()
        )));
    }
    let mut files = Vec::new();
    let mut assets = Vec::new();
    let walker = walkdir::WalkDir::new(graph).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        e.depth() == 0 || (!name.starts_with('.') && !(e.depth() == 1 && IGNORED.contains(&name.as_ref())))
    });
    for entry in walker {
        let entry = entry.map_err(|e| Error::io(graph, e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(graph).unwrap().to_string_lossy().replace('\\', "/");
        if rel.starts_with("assets/") {
            assets.push((entry.path().to_path_buf(), rel));
        } else {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            files.push(SourceFile { path: rel, text });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = map_graph(&files, &Options { dest, config: cfg });
    let mut s = Summary { report: join(dest, "import-report.md"), ..Default::default() };
    for (m, p) in out.pages.iter().zip(planned_kinds(&files, dest, cfg)) {
        match vault.create(&m.path, &m.text) {
            Ok(()) => {
                if p { s.journals += 1 } else { s.pages += 1 }
            }
            Err(Error::Exists(_)) => {
                s.skipped += 1;
                out.report.note(&m.path, 0, "already in the vault, not written");
            }
            Err(e) => return Err(e),
        }
    }
    for (src, rel) in assets {
        let dst = vault.abs(&join(dest, &rel));
        if dst.exists() {
            s.skipped += 1;
            out.report.note(&rel, 0, "already in the vault, not written");
            continue;
        }
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| Error::io(&dst, e))?;
        s.assets += 1;
    }
    s.unmapped = out.report.entries.len();
    let summary = format!(
        "Imported {} pages, {} journals and {} assets from `{}` on {}. {} files were already in the vault and were left alone.",
        s.pages, s.journals, s.assets, graph.display(), chrono::Local::now().date_naive(), s.skipped
    );
    vault.write(&s.report, &out.report.render(&summary))?;
    Ok(s)
}
```

`planned_kinds` is a smell: `Output.pages` should say whether each entry was
a journal. Do that instead: add `pub journal: bool` to `Mapped`, set it in
`map_graph`, and count from it; drop `Output.journals` and read
`out.pages.iter().filter(|m| m.journal).count()` in the Task 4 test. Derive
`Default` on `Summary`.

- [ ] **Step 4: Run, format, lint**

Run: `cargo test -p engram-notes-core import:: && cargo fmt && cargo clippy --workspace -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add core/src/import
git commit -m "feat(import): run the Logseq import against a vault"
```

---

### Task 6: The command, in Tauri and the palette

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/commands.ts`, `ui/src/lib/commands.test.ts`

**Interfaces:**
- Consumes: `engram_core::import::logseq::{run, Summary}`.
- Produces: Tauri command `import_logseq(source: String, dest: String) -> Summary`;
  `api.importLogseq(source, dest): Promise<ImportSummary>`; palette command
  `import-logseq` named *Import: Logseq graph*.

- [ ] **Step 1: The Rust command**

In `src-tauri/src/commands.rs`:

```rust
use engram_core::import::logseq::{self as logseq_import, Summary as ImportSummary};

/// `dest` is an absolute folder from the dialog; it has to be inside the vault.
fn inside_vault(vault: &Vault, dir: &str) -> CmdResult<String> {
    let abs = std::path::Path::new(dir).canonicalize().map_err(|_| CommandError {
        code: "not_found",
        message: format!("{dir}: no such folder"),
    })?;
    let rel = abs.strip_prefix(vault.root()).map_err(|_| CommandError {
        code: "config",
        message: "the destination must be a folder inside the vault".into(),
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
pub fn import_logseq(state: State<AppState>, source: String, dest: String) -> CmdResult<ImportSummary> {
    with_open(&state, |o| {
        let rel = inside_vault(&o.vault, &dest)?;
        let summary = logseq_import::run(&o.vault, &o.config, std::path::Path::new(&source), &rel)?;
        o.index.rebuild(&o.vault)?;
        Ok(summary)
    })
}
```

Register `commands::import_logseq` in `src-tauri/src/lib.rs` after
`commands::snippets`. Add a serialisation smoke test beside the others in
`commands.rs`:

```rust
    #[test]
    fn import_summary() {
        let s = ImportSummary { pages: 2, journals: 1, assets: 0, skipped: 0, unmapped: 3, report: "import-report.md".into() };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["report"], "import-report.md");
        assert_eq!(v["unmapped"], 3);
    }

    #[test]
    fn destination_outside_the_vault_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path().join("v").tap(|p| std::fs::create_dir_all(p).unwrap())).unwrap();
        ...
    }
```

There is no `tap`; write it plainly: `let root = d.path().join("v");
std::fs::create_dir_all(&root).unwrap(); let v = Vault::open(&root).unwrap();`
then `assert!(inside_vault(&v, d.path().to_str().unwrap()).is_err());` and
`assert_eq!(inside_vault(&v, root.to_str().unwrap()).unwrap(), "");`.

- [ ] **Step 2: Run the Rust side**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: green.

- [ ] **Step 3: The frontend**

`ui/src/lib/api.ts`, after `snippets`:

```ts
export interface ImportSummary { pages: number; journals: number; assets: number; skipped: number; unmapped: number; report: string }
export const importLogseq = (source: string, dest: string) => invoke<ImportSummary>("import_logseq", { source, dest });
```

`ui/src/lib/commands.ts`: import `importLogseq as importLogseqCmd,
errorMessage` from `./api` and add

```ts
/** Two folders, then the report opens. The daily folder is the vault's, so journals land where Ctrl+D looks. */
export async function importLogseq() {
  const source = await open({ directory: true, title: "Choose the Logseq graph folder" });
  if (typeof source !== "string") return;
  const dest = await open({ directory: true, defaultPath: app.root ?? undefined, title: "Choose the folder in this vault to import into" });
  if (typeof dest !== "string") return;
  try {
    const s = await importLogseqCmd(source, dest);
    await app.refresh();
    await app.openNote(s.report);
    const rest = s.unmapped ? `${s.unmapped} things to look at in the report.` : "Everything was mapped.";
    app.say(`Imported ${s.pages} pages and ${s.journals} journals. ${rest}`);
  } catch (e) {
    app.say(errorMessage(e));
  }
}
```

and the entry, after `insert-template`:

```ts
  { id: "import-logseq", name: "Import: Logseq graph", hotkey: "", run: () => importLogseq() },
```

Check `ui/src/lib/commands.test.ts` for a list of command ids or names and
add the new one where the test enumerates them; if it does not, add one
assertion that `defaults.some((c) => c.id === "import-logseq")`.

- [ ] **Step 4: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri ui/src
git commit -m "feat(import): Import: Logseq graph command"
```

---

### Task 7: Docs, smoke lines, roadmap

**Files:**
- Create: `docs/import.md`
- Modify: `docs/smoke.md`, `ROADMAP.md`, `README.md`

- [ ] **Step 1: `docs/import.md`**

In the shape of `docs/bases.md`: what the command does (two folders, original
untouched, report), the mapping as a table (Logseq → engram-notes, one row per
rule from the spec plus the decisions above), where journals go and why, what
the report lists, and the two things a user should do afterwards (read the
report; run the import once — a second run writes nothing).

- [ ] **Step 2: Smoke lines 63–67 in `docs/smoke.md`**

```
63. *Import: Logseq graph* on a real graph: pick the graph, pick the vault
    root; the report opens, and the explorer shows the pages and the journals
    in the daily folder.
64. A page that had `id::` blocks: the block ends in `^…`, and a page that
    referenced it shows a link that opens the block.
65. `TODO` and `DONE` items show as checkboxes; a `DOING` item is an unchecked
    box with the word.
66. A namespaced page `a/b` is a folder `a` with `b.md`; `[[a/b]]` links to it.
67. Run the import a second time into the same folder: nothing is written,
    and the report lists every file as already in the vault.
```

- [ ] **Step 3: `ROADMAP.md`**

Change `## 0.4 — The Logseq importer ⬜` to 🔶, keep the paragraph, and add
"In review; what remains is smoke lines 63–67 in the real window." Add the
three bullets:

```
- 🔶 *Import: Logseq graph* picks the graph and a folder in the vault; the
  original is never written; `import-report.md` names every file and line
  that was kept as text or left out.
- 🔶 Journals to the daily folder, `a___b.md` to `a/b.md`, page properties
  to frontmatter, `id::` to `^anchor` and `((uuid))` to `[[Page#^anchor]]`,
  `{{embed}}` to `![[…]]`, `collapsed::` dropped, task markers to
  checkboxes, `#[[multi word]]` to a link, tabs to `editor.indent`,
  `assets/` copied beside the notes (`docs/import.md`).
- 🔶 Idempotent: a second run over the vault writes nothing.
```

- [ ] **Step 4: README**

Find the feature list in `README.md` and add one line naming the importer
and `docs/import.md`.

- [ ] **Step 5: Commit**

```bash
git add docs ROADMAP.md README.md
git commit -m "docs: the Logseq importer, smoke lines 63-67, roadmap 0.4"
```

---

### Task 8: Verify and hand over

- [ ] **Step 1: Full verification**

```bash
cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace
cd ui && pnpm check && pnpm test
```

- [ ] **Step 2: Build the real window if the toolchain is present**

`cd ui && pnpm tauri build --debug --no-bundle` — if this machine cannot,
say so in the handover rather than guessing.

- [ ] **Step 3: Push and open the pull request**

```bash
git push -u origin feat/logseq-importer
gh pr create --title "feat: the Logseq importer, roadmap step 0.4" --body "..."
```

Body: what it does, the decisions list from this plan, and that smoke lines
63–67 have not been run in the real window.

---

## Self-review

- **Spec coverage.** Journals → daily (Task 4 `plan`); `a___b` → `a/b`
  (`page_name`); page properties → frontmatter, `title::` renames (`plan`,
  `frontmatter`); `id::` → `^anchor` and the table (`collect_ids`);
  `((uuid))`, `{{embed ((uuid))}}`, `{{embed [[page]]}}` (Task 3);
  `collapsed::` dropped, other block properties kept and reported (Task 4
  `render`); task markers, priorities, scheduling kept (Task 3);
  `#[[multi word]]` (Task 3); tabs → indent, bullets kept (Task 2); `assets/`
  copied, `logseq/` ignored (Task 5); no I/O in the mapping (Task 4);
  fixture graphs (Task 4); idempotent (Task 4 test); the report with file and
  line (Task 1); one command, original untouched (Tasks 5, 6).
- **Type consistency.** `Mapped` gains `journal: bool` in Task 5 and
  `Output.journals` goes; the Task 4 test must read
  `out.pages.iter().filter(|m| m.journal).count()`. `inline::rewrite` takes
  `&Refs` everywhere. `Report::note` takes `&str, usize, impl Into<String>`.
