# Graph and Bases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the graph view and table bases from the spec, and close the four explorer and editor items the handoff carried over: drag to move, folder rename and delete, inline rename, and image embeds inside notes.

**Architecture:** Core gains folder and attachment moves with link rewriting, a graph builder over the index, and a `bases` module (expression parser, evaluator, `.base` file model, table runner). The Tauri shell exposes each as a thin command. The frontend gets pure, tested helpers (`tree.ts`, `graph.ts`, additions to `files.ts` and `layout.ts`) and components for the explorer, a canvas graph driven by `d3-force`, and a base table. Graph and base views open as tabs; a graph tab has the pseudo-path `graph:global` or `graph:local`, which no file can have because Obsidian forbids `:` in names.

**Tech Stack:** Rust 2024, rusqlite 0.40, serde_yaml_ng 0.10, chrono 0.4, Tauri 2, Svelte 5, CodeMirror 6, markdown-it 15, d3-force 3, vitest 5.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`, sections *The graph* and *Bases*, plus *The vault* (rename and move) and *The editor and UI* (explorer). Carried-over items come from `docs/superpowers/plans/2026-09-12-handoff.md`.

## Global Constraints

- `AGENTS.md` applies: short comments that say why, `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean, `core` has no Tauri dependency, conventional commits under 72 characters, errors typed in `core::Error`.
- Files are the truth. Nothing in this plan changes the index schema.
- Obsidian is the reference, with these facts checked against Obsidian's help pages on 2026-09-13:
  - A `.base` file is YAML with `filters`, `formulas`, `properties`, `summaries`, `views`. A filter is an expression string or a mapping with one of `and`, `or`, `not` holding a list of filters. Global and view filters combine with AND.
  - A view has `type`, `name`, `limit`, `filters`, `order` (column ids), `sort` (a list of `property` and `direction: ASC|DESC`), `groupBy`, `summaries`. `properties.<id>.displayName` names a column.
  - Property ids are `note.<name>` (or the bare name), `file.<field>`, `formula.<name>`.
  - `file.tags` values carry `#`. `file.hasTag("a")` also matches nested tags like `a/b`. `file.inFolder("x")` includes sub-folders. `file.name` has the extension, `file.basename` has not.
  - Operators: `+ - * / %`, `== != > < >= <=`, `! && ||`, parentheses. A date plus or minus a duration string shifts it; units `y year years`, `M month months`, `d day days`, `w week weeks`, `h hour hours`, `m minute minutes`, `s second seconds`. `date("2024-12-01") + "1M" + "4h" + "3m"` is `2025-01-01 04:03:00`.
  - `if(condition, trueResult, falseResult?)`; `now()`; `today()` is midnight; `date(string)`.
  - Graph settings use Obsidian's `graph.json` keys and defaults: `search ""`, `showTags false`, `showAttachments false`, `hideUnresolved false`, `showOrphans true`, `showArrow false`, `textFadeMultiplier 0`, `nodeSizeMultiplier 1`, `lineSizeMultiplier 1`, `centerStrength 0.518713248970312`, `repelStrength 10`, `linkStrength 1`, `linkDistance 250`. Open graph view is `Ctrl+G`. Hover highlights neighbours, click opens, wheel zooms, drag pans.
  - `![[pic.png|300]]` sets the width, `![[pic.png|300x200]]` width and height.
- Anything in a base outside the supported subset is listed in the view as unsupported and a filter that fails yields no rows, never a guess.
- Tests run without a window, a model or the network. Every UI change is verified before its task is called done: the real window with `spectacle -b -n -f -o <file>` if that works, otherwise `ui/scripts/shot.mjs` in headless Chromium.
- TypeScript stays on 5.x. After adding an npm package, restart Vite with `pnpm dev --force`.

## Out of scope

- *Semantic edges* in the graph need `assoc` and embeddings, which the semantic-search-and-memory plan builds. That plan adds the toggle.
- Card view, embedded bases, `groupBy` and `summaries` (the spec defers them; the view reports them as unsupported).

## File structure

```
core/src/error.rs               + Error::Base
core/src/vault.rs               + folders()
core/src/rename.rs              moves of a file, an attachment or a folder; Move; rewrite_links over moves
core/src/index/rebuild.rs       remove_file drops everything below a folder
core/src/graph.rs               Graph {nodes, edges} from the index and the file list
core/src/config.rs              + load_graph, save_graph (graph.json, opaque JSON)
core/src/bases/mod.rs           BaseFile model, notes(index), run() -> Table, set_sort()
core/src/bases/expr.rs          lexer and parser for the supported expression subset
core/src/bases/eval.rs          Value, NoteData, Ctx::eval, dates and durations
core/src/lib.rs                 + pub mod bases, graph
src-tauri/src/commands.rs       list_folders, graph, get/set_graph_config, run_base, set_base_sort
src-tauri/src/lib.rs            registers them
src-tauri/src/error.rs          code "base"
src-tauri/tauri.conf.json       dragDropEnabled false, so HTML drag and drop reaches the webview
ui/src/lib/api.ts               types and wrappers for the new commands
ui/src/lib/files.ts             + base and graph kinds, resolveFile, imageSize, tabTitle
ui/src/lib/layout.ts            renamePath and withoutPath cover folders
ui/src/lib/tree.ts              explorer tree with empty folders, drop and rename targets
ui/src/lib/graph.ts             settings, search parsing, filtering, local neighbourhood, sizes
ui/src/lib/render.ts            image embeds and markdown images through a resolver
ui/src/editor/livePreview.ts    ImageWidget for embeds and markdown images
ui/src/lib/state.svelte.ts      folders, lastNote, folder-aware renamed/forget
ui/src/lib/commands.ts          graph, local graph, new base
ui/src/components/Explorer.svelte   inline rename, folder menu, drag to move
ui/src/components/GraphView.svelte  canvas renderer, d3-force, controls
ui/src/components/BaseView.svelte   views, table, sort, cell editing, source mode
ui/src/components/YamlEditor.svelte CodeMirror with YAML, for a base's source
ui/src/components/PropertyValue.svelte one typed property editor, shared by Properties and BaseView
ui/src/components/Pane.svelte       picks note, file, graph or base view
ui/scripts/shot.mjs             mocks the new commands from the fixture
docs/bases.md                   the supported subset
docs/smoke.md                   lines for the new features
```

---
### Task 1: Folder and attachment moves in core

The explorer will rename and move folders and attachments, so a rename must move every file below a folder and rewrite links to notes and attachments alike. Empty folders must be listable.

**Files:**
- Modify: `core/src/vault.rs` (add `folders`)
- Modify: `core/src/index/rebuild.rs` (`remove_file`)
- Modify: `core/src/rename.rs` (whole file)
- Modify: `src-tauri/src/commands.rs` (`plan_rename` passes the vault)
- Test: in-file test modules of those files

**Interfaces:**
- Produces: `Vault::folders(&self) -> Result<Vec<String>>`; `Index::remove_file(rel)` also removes notes below a folder; `rename::Move { from: String, to: String, unique: bool }`; `rename::plan_rename(vault: &Vault, index: &Index, from: &str, to: &str) -> Result<RenamePlan>`; `rename::apply_rename(vault, index, plan)` unchanged in signature; `rename::rewrite_links(text: &str, moves: &[Move]) -> String`. `RenamePlan { from, to, affected }` keeps its shape; `affected` lists files outside the move whose links change.

- [ ] **Step 1: Write the failing tests**

In `core/src/vault.rs` tests:

```rust
    #[test]
    fn folders_include_empty_and_skip_hidden() {
        let (d, v) = tmp();
        fs::create_dir_all(d.path().join("a/b")).unwrap();
        fs::create_dir_all(d.path().join(".git/x")).unwrap();
        fs::create_dir_all(d.path().join("empty")).unwrap();
        assert_eq!(v.folders().unwrap(), vec!["a", "a/b", "empty"]);
    }
```

In `core/src/index/rebuild.rs` tests:

```rust
    #[test]
    fn remove_file_drops_a_whole_folder() {
        let (_d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        ix.remove_file("sub").unwrap();
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 1);
    }
```

In `core/src/rename.rs`, replace the three `rewrite_*` tests and add two:

```rust
    fn mv(from: &str, to: &str, unique: bool) -> Move {
        Move {
            from: from.into(),
            to: to.into(),
            unique,
        }
    }

    #[test]
    fn rewrite_keeps_alias_heading_and_other_text() {
        let text = "see [[Old|shown]] and [[Old#Sec]] and ![[Old]] and [[Older]] and [m](sub/Old.md) `[[Old]]`";
        let out = rewrite_links(text, &[mv("sub/Old.md", "New Name.md", true)]);
        assert_eq!(
            out,
            "see [[New Name|shown]] and [[New Name#Sec]] and ![[New Name]] and [[Older]] and [m](New%20Name.md) `[[Old]]`"
        );
    }

    #[test]
    fn rewrite_uses_full_path_when_stem_is_ambiguous() {
        let out = rewrite_links("[[Old]]", &[mv("Old.md", "a/Note.md", false)]);
        assert_eq!(out, "[[a/Note]]");
    }

    #[test]
    fn rewrite_keeps_block_fragment() {
        let out = rewrite_links("[[Old#^b1|see]]", &[mv("Old.md", "New.md", true)]);
        assert_eq!(out, "[[New#^b1|see]]");
    }

    fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, Vault, Index) {
        let d = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let p = d.path().join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, v, ix)
    }

    #[test]
    fn renaming_an_attachment_rewrites_its_embeds() {
        let (_d, v, mut ix) = indexed(&[("pic.png", "p"), ("N.md", "![[pic.png|300]]")]);
        let plan = plan_rename(&v, &ix, "pic.png", "img/photo.png").unwrap();
        assert_eq!(plan.affected, vec!["N.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("N.md").unwrap(), "![[photo.png|300]]");
        assert!(v.stat("img/photo.png").unwrap().is_some());
    }

    #[test]
    fn folder_move_takes_every_file_and_rewrites_path_links() {
        let (d, v, mut ix) = indexed(&[
            ("old/A.md", "[[old/deep/B]] ![[pic.png]]"),
            ("old/deep/B.md", "b"),
            ("old/pic.png", "p"),
            ("Ref.md", "[[old/A]] [[B]] ![[old/pic.png]]"),
            ("Other.md", "nothing"),
        ]);
        let plan = plan_rename(&v, &ix, "old", "new/place").unwrap();
        assert_eq!(plan.affected, vec!["Ref.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("Ref.md").unwrap(), "[[A]] [[B]] ![[pic.png]]");
        assert_eq!(v.read("new/place/A.md").unwrap(), "[[B]] ![[pic.png]]");
        assert!(v.stat("new/place/pic.png").unwrap().is_some());
        assert!(!d.path().join("old").exists());
        assert!(ix.note("old/A.md").unwrap().is_none());
        assert_eq!(ix.backlinks("new/place/deep/B.md").unwrap().len(), 2);
        assert!(ix.unresolved().unwrap().is_empty());
    }
```

Update the existing `plan_and_apply_move_the_file_and_fix_referrers` test to call `plan_rename(&v, &ix, "Old.md", "sub/New.md")`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core`
Expected: compile errors: no method `folders`, no struct `Move`, `plan_rename` takes 3 arguments.

- [ ] **Step 3: `Vault::folders`**

In `core/src/vault.rs`, after `walk`:

```rust
    /// Folders below the root, hidden ones skipped, so empty folders can be shown.
    pub fn folders(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !is_hidden(e.file_name()));
        for entry in walker {
            let entry = entry.map_err(|e| Error::io(&self.root, e.into()))?;
            if entry.file_type().is_dir() {
                let rel = entry.path().strip_prefix(&self.root).unwrap();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        out.sort();
        Ok(out)
    }
```

- [ ] **Step 4: `remove_file` covers folders**

In `core/src/index/rebuild.rs` replace `remove_file`:

```rust
    /// Drops `rel`, or every note below it when it names a folder.
    pub fn remove_file(&mut self, rel: &str) -> Result<()> {
        let dir = format!("{}/", rel.trim_end_matches('/'));
        self.conn.execute(
            "DELETE FROM notes WHERE path=?1 OR substr(path, 1, length(?2))=?2",
            params![rel, dir],
        )?;
        self.resolve_all()
    }
```

- [ ] **Step 5: Moves in `rename.rs`**

Replace everything above `#[cfg(test)]` in `core/src/rename.rs`:

```rust
//! Renaming or moving a file or folder means rewriting every link that
//! pointed at what moved.

use crate::Result;
use crate::index::Index;
use crate::parse::{LinkKind, parse};
use crate::vault::{Vault, normalize_rel};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenamePlan {
    pub from: String,
    pub to: String,
    /// Files outside the move whose links change, by their current path.
    pub affected: Vec<String>,
}

/// One file's old and new path, and whether its new name is unique in the
/// vault so a wikilink can stay a bare name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: String,
    pub to: String,
    pub unique: bool,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

fn is_note(path: &str) -> bool {
    path.to_lowercase().ends_with(".md")
}

fn clean(path: &str) -> String {
    normalize_rel(path).trim_end_matches('/').to_owned()
}

// A folder moves every file below it.
fn moves(vault: &Vault, from: &str, to: &str) -> Result<Vec<Move>> {
    let files = vault.walk()?;
    let pairs: Vec<(String, String)> = if vault.abs(from).is_dir() {
        let prefix = format!("{from}/");
        files
            .iter()
            .filter_map(|e| {
                let rest = e.path.strip_prefix(&prefix)?;
                Some((e.path.clone(), format!("{to}/{rest}")))
            })
            .collect()
    } else {
        vec![(from.to_owned(), to.to_owned())]
    };
    Ok(pairs
        .into_iter()
        .map(|(from, to)| {
            let s = stem(&to).to_lowercase();
            let unique = !files
                .iter()
                .any(|e| e.path != from && stem(&e.path).to_lowercase() == s);
            Move { from, to, unique }
        })
        .collect())
}

// Attachments never resolve in the index, so links to them match by name.
fn referrers(index: &Index, path: &str) -> Result<Vec<String>> {
    if is_note(path) {
        return Ok(index
            .backlinks(path)?
            .into_iter()
            .map(|l| l.src_path)
            .collect());
    }
    let mut stmt = index
        .conn()
        .prepare("SELECT src_path, target_raw FROM links WHERE target_path IS NULL")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (src, raw) = row?;
        if names(&raw, path) {
            out.push(src);
        }
    }
    Ok(out)
}

pub fn plan_rename(vault: &Vault, index: &Index, from: &str, to: &str) -> Result<RenamePlan> {
    let (from, to) = (clean(from), clean(to));
    let moved: HashSet<String> = moves(vault, &from, &to)?
        .into_iter()
        .map(|m| m.from)
        .collect();
    let mut affected = Vec::new();
    for path in &moved {
        affected.extend(referrers(index, path)?);
    }
    affected.retain(|p| !moved.contains(p));
    affected.sort();
    affected.dedup();
    Ok(RenamePlan { from, to, affected })
}

pub fn apply_rename(vault: &Vault, index: &mut Index, plan: &RenamePlan) -> Result<()> {
    let moves = moves(vault, &plan.from, &plan.to)?;
    vault.rename(&plan.from, &plan.to)?;
    // Moved notes are rewritten too: they may name each other or themselves.
    let mut files: Vec<String> = moves
        .iter()
        .filter(|m| is_note(&m.to))
        .map(|m| m.to.clone())
        .collect();
    files.extend(plan.affected.iter().cloned());
    files.sort();
    files.dedup();
    for path in &files {
        let text = vault.read(path)?;
        let out = rewrite_links(&text, &moves);
        if out != text {
            vault.write(path, &out)?;
        }
    }
    // One rebuild re-parses what moved or changed and resolves links once.
    index.rebuild(vault)?;
    Ok(())
}

/// Does `target`, as written in a link, name `old_path`? The resolver's rule
/// applied to one known file: exact path or bare name, case-insensitive.
fn names(target: &str, old_path: &str) -> bool {
    let t = normalize_rel(target).to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t);
    let old = old_path.to_lowercase();
    let old = old.strip_suffix(".md").unwrap_or(&old);
    t == old || t == stem(old)
}

pub fn rewrite_links(text: &str, moves: &[Move]) -> String {
    let note = parse(text);
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for l in &note.links {
        let Some(m) = moves.iter().find(|m| names(&l.target, &m.from)) else {
            continue;
        };
        out.push_str(&text[pos..l.start]);
        let original = &text[l.start..l.end];
        match l.kind {
            LinkKind::Wiki | LinkKind::Embed => {
                let name = if m.unique {
                    stem(&m.to)
                } else {
                    m.to.strip_suffix(".md").unwrap_or(&m.to)
                };
                let bang = if l.kind == LinkKind::Embed { "!" } else { "" };
                let fragment = match (&l.heading, &l.block) {
                    (Some(h), _) => format!("#{h}"),
                    (None, Some(b)) => format!("#^{b}"),
                    (None, None) => String::new(),
                };
                let alias = l
                    .alias
                    .as_ref()
                    .map(|a| format!("|{a}"))
                    .unwrap_or_default();
                out.push_str(&format!("{bang}[[{name}{fragment}{alias}]]"));
            }
            LinkKind::Markdown => {
                let open = original.rfind('(').unwrap_or(0);
                out.push_str(&original[..=open]);
                out.push_str(&m.to.replace(' ', "%20"));
                out.push(')');
            }
        }
        pos = l.end;
    }
    out.push_str(&text[pos..]);
    out
}
```

The two moved-note paths in `apply_rename` are new paths; `plan.affected` are outside the move, so their paths did not change.

- [ ] **Step 6: The Tauri command passes the vault**

In `src-tauri/src/commands.rs`, `plan_rename` becomes:

```rust
        Ok(engram_core::rename::plan_rename(&o.vault, &o.index, &from, &to)?)
```

- [ ] **Step 7: Run the tests**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all pass, including the four new tests.

- [ ] **Step 8: Commit**

```bash
git add core/src src-tauri/src/commands.rs
git commit -m "feat(core): move folders and attachments with their links"
```

### Task 2: Graph data and graph.json in core

The graph view needs every note, attachment and unresolved target as a node and each distinct link as an edge. Attachments do not resolve in the index, so the builder matches embeds against the file list by path or name, shortest path first, as notes resolve.

**Files:**
- Create: `core/src/graph.rs`
- Modify: `core/src/lib.rs` (`pub mod graph;`)
- Modify: `core/src/config.rs` (add `load_graph`, `save_graph`)
- Test: in-file test modules

**Interfaces:**
- Consumes: `Index::titles()`, `Index::conn()`, `vault::FileEntry`, `vault::normalize_rel`.
- Produces: `graph::build(index: &Index, files: &[FileEntry]) -> Result<Graph>`; `Graph { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> }`; `GraphNode { id: String, title: String, kind: NodeKind, tags: Vec<String> }` where `id` is the vault path or, for `NodeKind::Unresolved`, the raw target; `NodeKind` serialises as `"note" | "attachment" | "unresolved"`; `GraphEdge { source: String, target: String }`. `config::load_graph(vault) -> Result<serde_json::Value>` (`{}` when missing) and `config::save_graph(vault, &Value) -> Result<()>`.

- [ ] **Step 1: Write the failing tests**

`core/src/graph.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use std::fs;

    #[test]
    fn notes_attachments_unresolved_and_one_edge_per_pair() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::create_dir_all(d.path().join("img")).unwrap();
        fs::write(
            d.path().join("A.md"),
            "[[B]] [[B]] [[Ghost]] ![[pic.png]] #t",
        )
        .unwrap();
        fs::write(d.path().join("sub/B.md"), "[[A]] [[B]]").unwrap();
        fs::write(d.path().join("img/pic.png"), "p").unwrap();
        fs::write(d.path().join("other.pdf"), "p").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let g = build(&ix, &v.walk().unwrap()).unwrap();
        let nodes: Vec<_> = g
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.title.as_str(), n.kind))
            .collect();
        assert_eq!(
            nodes,
            vec![
                ("A.md", "A", NodeKind::Note),
                ("sub/B.md", "B", NodeKind::Note),
                ("Ghost", "Ghost", NodeKind::Unresolved),
                ("img/pic.png", "pic.png", NodeKind::Attachment),
                ("other.pdf", "other.pdf", NodeKind::Attachment),
            ]
        );
        assert_eq!(g.nodes[0].tags, vec!["t"]);
        let edges: Vec<_> = g
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        assert_eq!(
            edges,
            vec![
                ("A.md", "sub/B.md"),
                ("A.md", "Ghost"),
                ("A.md", "img/pic.png"),
                ("sub/B.md", "A.md"),
            ]
        );
    }
}
```

In `core/src/config.rs` tests:

```rust
    #[test]
    fn graph_settings_are_opaque_json() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert_eq!(load_graph(&v).unwrap(), serde_json::json!({}));
        save_graph(&v, &serde_json::json!({"showOrphans": false})).unwrap();
        assert_eq!(load_graph(&v).unwrap()["showOrphans"], false);
        assert!(d.path().join(".engram-notes/graph.json").is_file());
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core graph`
Expected: compile errors, `build` and `load_graph` not found.

- [ ] **Step 3: The builder**

`core/src/graph.rs` above the tests:

```rust
//! Nodes and edges for the graph view.

use crate::Result;
use crate::index::Index;
use crate::vault::{FileEntry, normalize_rel};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Note,
    Attachment,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphNode {
    /// The vault path, or the link text for an unresolved target.
    pub id: String,
    pub title: String,
    pub kind: NodeKind,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn build(index: &Index, files: &[FileEntry]) -> Result<Graph> {
    let conn = index.conn();
    let mut tags: HashMap<String, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT path, tag FROM tags ORDER BY path, tag")?;
    for row in stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (path, tag) = row?;
        tags.entry(path).or_default().push(tag);
    }
    let mut nodes: Vec<GraphNode> = index
        .titles()?
        .into_iter()
        .map(|(path, title)| GraphNode {
            tags: tags.remove(&path).unwrap_or_default(),
            id: path,
            title,
            kind: NodeKind::Note,
        })
        .collect();

    // Attachments resolve like notes: exact path, then name, shortest path first.
    let mut attachments: Vec<&FileEntry> = files.iter().filter(|f| !f.is_markdown).collect();
    attachments.sort_by_key(|f| (f.path.len(), f.path.as_str()));
    let mut by_name: HashMap<String, &str> = HashMap::new();
    for f in &attachments {
        by_name.entry(f.path.to_lowercase()).or_insert(&f.path);
    }
    for f in &attachments {
        by_name.entry(name(&f.path).to_lowercase()).or_insert(&f.path);
    }

    let mut unresolved = Vec::new();
    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    let mut pairs = HashSet::new();
    let mut stmt =
        conn.prepare("SELECT src_path, target_raw, target_path FROM links ORDER BY src_path, start")?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })? {
        let (source, raw, resolved) = row?;
        let target = match resolved {
            Some(t) => t,
            None => match by_name.get(&normalize_rel(&raw).to_lowercase()) {
                Some(p) => (*p).to_owned(),
                None => {
                    if seen.insert(raw.clone()) {
                        unresolved.push(GraphNode {
                            id: raw.clone(),
                            title: raw.clone(),
                            kind: NodeKind::Unresolved,
                            tags: vec![],
                        });
                    }
                    raw
                }
            },
        };
        if source != target && pairs.insert((source.clone(), target.clone())) {
            edges.push(GraphEdge { source, target });
        }
    }
    nodes.extend(unresolved);
    nodes.extend(
        files
            .iter()
            .filter(|f| !f.is_markdown)
            .map(|f| GraphNode {
                id: f.path.clone(),
                title: name(&f.path).to_owned(),
                kind: NodeKind::Attachment,
                tags: vec![],
            }),
    );
    Ok(Graph { nodes, edges })
}
```

Add `pub mod graph;` to `core/src/lib.rs` in alphabetical order.

- [ ] **Step 4: graph.json**

In `core/src/config.rs`, after `save_workspace`:

```rust
pub fn load_graph(vault: &Vault) -> Result<serde_json::Value> {
    Ok(read_json(&vault.config_dir().join("graph.json"))?
        .unwrap_or_else(|| serde_json::json!({})))
}

pub fn save_graph(vault: &Vault, graph: &serde_json::Value) -> Result<()> {
    write_json(&vault.config_dir().join("graph.json"), graph)
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p engram-notes-core && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add core/src
git commit -m "feat(core): graph nodes and edges, graph.json"
```

### Task 3: The bases expression parser

Obsidian's expression syntax is JavaScript-like. This parser accepts the subset in the spec plus arithmetic, `if()`, `today()`, and the file fields the index has. Anything else is an error that names what is unsupported, so the view can report it.

**Files:**
- Create: `core/src/bases/mod.rs` (for now only `pub mod expr;`)
- Create: `core/src/bases/expr.rs`
- Modify: `core/src/lib.rs` (`pub mod bases;`)
- Modify: `core/src/error.rs` (add `Base(String)`)
- Modify: `src-tauri/src/error.rs` (map `Error::Base` to code `"base"`)
- Test: in-file test module of `expr.rs`

**Interfaces:**
- Produces: `bases::expr::parse(src: &str) -> Result<Expr, String>`; `Expr` with variants `Null`, `Bool(bool)`, `Num(f64)`, `Str(String)`, `Field(Scope, String)`, `Call(String, Vec<Expr>)` (globals as `"date"`, file methods as `"file.hasTag"`), `Method(Box<Expr>, String, Vec<Expr>)`, `Unary(char, Box<Expr>)` with `'!'` or `'-'`, `Binary(Box<Expr>, Op, Box<Expr>)`; `Scope { File, Note, Formula }`; `Op { Or, And, Eq, Ne, Lt, Le, Gt, Ge, Add, Sub, Mul, Div, Mod }`. `Error::Base(String)` displays as `base error: {0}`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::Expr::*;
    use super::*;

    fn b(l: Expr, op: Op, r: Expr) -> Expr {
        Binary(Box::new(l), op, Box::new(r))
    }

    #[test]
    fn precedence_and_fields() {
        assert_eq!(
            parse(r#"status != "done" && file.size > 1 + 2 * 3"#).unwrap(),
            b(
                b(Field(Scope::Note, "status".into()), Op::Ne, Str("done".into())),
                Op::And,
                b(
                    Field(Scope::File, "size".into()),
                    Op::Gt,
                    b(Num(1.0), Op::Add, b(Num(2.0), Op::Mul, Num(3.0)))
                )
            )
        );
        assert_eq!(
            parse("a || b && !c").unwrap(),
            b(
                Field(Scope::Note, "a".into()),
                Op::Or,
                b(
                    Field(Scope::Note, "b".into()),
                    Op::And,
                    Unary('!', Box::new(Field(Scope::Note, "c".into())))
                )
            )
        );
    }

    #[test]
    fn calls_methods_brackets_and_literals() {
        assert_eq!(
            parse(r#"file.hasTag("a", 'b')"#).unwrap(),
            Call("file.hasTag".into(), vec![Str("a".into()), Str("b".into())])
        );
        assert_eq!(
            parse(r#"note["due date"].isEmpty()"#).unwrap(),
            Method(
                Box::new(Field(Scope::Note, "due date".into())),
                "isEmpty".into(),
                vec![]
            )
        );
        assert_eq!(
            parse("formula.x == null").unwrap(),
            b(Field(Scope::Formula, "x".into()), Op::Eq, Null)
        );
        assert_eq!(parse("-2.5").unwrap(), Unary('-', Box::new(Num(2.5))));
        assert_eq!(
            parse("if(true, now(), 1)").unwrap(),
            Call("if".into(), vec![Bool(true), Call("now".into(), vec![]), Num(1.0)])
        );
        assert_eq!(parse(r#""a\"b""#).unwrap(), Str("a\"b".into()));
    }

    #[test]
    fn unsupported_is_named_and_bad_syntax_fails() {
        for (src, what) in [
            (r#"link("a")"#, "function `link()`"),
            ("name.lower()", "method `.lower()`"),
            ("name.length", "field `.length`"),
            ("file.ctime > now()", "field `file.ctime`"),
            ("file.asLink()", "method `file.asLink()`"),
            ("this.file", "`this`"),
            ("tags[0]", "index"),
        ] {
            let e = parse(src).unwrap_err();
            assert!(e.contains(what), "{src}: {e}");
            assert!(e.starts_with("unsupported"), "{src}: {e}");
        }
        for src in ["a ==", "\"open", "(a", "a b", "#"] {
            assert!(parse(src).is_err(), "{src}");
        }
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core bases`
Expected: compile errors, module `bases` not found.

- [ ] **Step 3: The error variant**

`core/src/error.rs`, after `Exists`:

```rust
    #[error("base error: {0}")]
    Base(String),
```

`src-tauri/src/error.rs`, in the match: `Error::Base(_) => "base",`.

- [ ] **Step 4: Lexer and parser**

`core/src/bases/mod.rs`:

```rust
//! Obsidian's `.base` files: filters and formulas over notes, shown as views.

pub mod expr;
```

Add `pub mod bases;` to `core/src/lib.rs`.

`core/src/bases/expr.rs` above the tests:

```rust
//! Obsidian's bases expression syntax, the subset `docs/bases.md` lists.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    /// `file.name`, `note.status` or a bare `status`, `formula.x`.
    Field(Scope, String),
    /// A global like `date`, or a file method as `file.hasTag`.
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    Unary(char, Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    File,
    Note,
    Formula,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

const GLOBALS: &[&str] = &["date", "now", "today", "if"];
const FILE_FIELDS: &[&str] = &[
    "name", "basename", "path", "folder", "ext", "size", "mtime", "tags", "links",
];
const FILE_METHODS: &[&str] = &["hasTag", "inFolder", "hasLink", "hasProperty"];
const METHODS: &[&str] = &["contains", "isEmpty"];
// Two-character operators first, so `<=` is not read as `<`.
const PUNCT: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "<", ">", "!", "+", "-", "*", "/", "%", "(", ")", ",",
    ".", "[", "]",
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    P(&'static str),
}

fn syntax(src: &str, what: &str) -> String {
    format!("{what} in `{src}`")
}

fn unsupported(src: &str, what: &str) -> String {
    format!("unsupported {what} in `{src}`")
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Tok::Num(
                text.parse().map_err(|_| syntax(src, "bad number"))?,
            ));
        } else if c == '"' || c == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(syntax(src, "unclosed string")),
                    Some(&q) if q == c => break,
                    Some('\\') => {
                        s.extend(chars.get(i + 1));
                        i += 2;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            i += 1;
            out.push(Tok::Str(s));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let p = PUNCT
                .iter()
                .find(|p| rest.starts_with(**p))
                .ok_or_else(|| syntax(src, &format!("unexpected `{c}`")))?;
            out.push(Tok::P(p));
            i += p.len();
        }
    }
    Ok(out)
}

pub fn parse(src: &str) -> Result<Expr, String> {
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
        src,
    };
    let e = p.or()?;
    if p.pos < p.toks.len() {
        return Err(syntax(src, "unexpected input after the expression"));
    }
    Ok(e)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    src: &'a str,
}

type Level<'a> = fn(&mut Parser<'a>) -> Result<Expr, String>;

impl<'a> Parser<'a> {
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn eat(&mut self, p: &str) -> bool {
        let hit = matches!(self.toks.get(self.pos), Some(Tok::P(q)) if *q == p);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn expect(&mut self, p: &str) -> Result<(), String> {
        if self.eat(p) {
            Ok(())
        } else {
            Err(syntax(self.src, &format!("expected `{p}`")))
        }
    }

    fn binary(&mut self, ops: &[(&str, Op)], next: Level<'a>) -> Result<Expr, String> {
        let mut left = next(self)?;
        'more: loop {
            for (s, op) in ops {
                if self.eat(s) {
                    left = Expr::Binary(Box::new(left), *op, Box::new(next(self)?));
                    continue 'more;
                }
            }
            return Ok(left);
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        self.binary(&[("||", Op::Or)], Self::and)
    }

    fn and(&mut self) -> Result<Expr, String> {
        self.binary(&[("&&", Op::And)], Self::equality)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        self.binary(&[("==", Op::Eq), ("!=", Op::Ne)], Self::relational)
    }

    fn relational(&mut self) -> Result<Expr, String> {
        let ops = [("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)];
        self.binary(&ops, Self::additive)
    }

    fn additive(&mut self) -> Result<Expr, String> {
        self.binary(&[("+", Op::Add), ("-", Op::Sub)], Self::multiplicative)
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        let ops = [("*", Op::Mul), ("/", Op::Div), ("%", Op::Mod)];
        self.binary(&ops, Self::unary)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        for op in ['!', '-'] {
            if self.eat(&op.to_string()) {
                return Ok(Expr::Unary(op, Box::new(self.unary()?)));
            }
        }
        self.postfix()
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        if self.eat(")") {
            return Ok(out);
        }
        loop {
            out.push(self.or()?);
            if self.eat(")") {
                return Ok(out);
            }
            self.expect(",")?;
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Tok::Ident(s)) => Ok(s),
            _ => Err(syntax(self.src, "expected a name")),
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat("[") {
                return Err(unsupported(self.src, "index `[...]`"));
            }
            if !self.eat(".") {
                return Ok(e);
            }
            let m = self.name()?;
            if !self.eat("(") {
                return Err(unsupported(self.src, &format!("field `.{m}`")));
            }
            if !METHODS.contains(&m.as_str()) {
                return Err(unsupported(self.src, &format!("method `.{m}()`")));
            }
            e = Expr::Method(Box::new(e), m, self.args()?);
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Num(n)),
            Some(Tok::Str(s)) => Ok(Expr::Str(s)),
            Some(Tok::P("(")) => {
                let e = self.or()?;
                self.expect(")")?;
                Ok(e)
            }
            Some(Tok::Ident(id)) => self.ident(id),
            Some(Tok::P(p)) => Err(syntax(self.src, &format!("unexpected `{p}`"))),
            None => Err(syntax(self.src, "unexpected end")),
        }
    }

    fn ident(&mut self, id: String) -> Result<Expr, String> {
        match id.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "null" => return Ok(Expr::Null),
            "this" => return Err(unsupported(self.src, "`this`")),
            _ => {}
        }
        if self.eat("(") {
            if !GLOBALS.contains(&id.as_str()) {
                return Err(unsupported(self.src, &format!("function `{id}()`")));
            }
            return Ok(Expr::Call(id, self.args()?));
        }
        let scope = match id.as_str() {
            "file" => Scope::File,
            "note" => Scope::Note,
            "formula" => Scope::Formula,
            _ => return Ok(Expr::Field(Scope::Note, id)),
        };
        let key = if self.eat("[") {
            let Some(Tok::Str(k)) = self.next() else {
                return Err(syntax(self.src, "expected a quoted name"));
            };
            self.expect("]")?;
            k
        } else {
            self.expect(".")?;
            self.name()?
        };
        if scope != Scope::File {
            return Ok(Expr::Field(scope, key));
        }
        if self.eat("(") {
            if !FILE_METHODS.contains(&key.as_str()) {
                return Err(unsupported(self.src, &format!("method `file.{key}()`")));
            }
            return Ok(Expr::Call(format!("file.{key}"), self.args()?));
        }
        if !FILE_FIELDS.contains(&key.as_str()) {
            return Err(unsupported(self.src, &format!("field `file.{key}`")));
        }
        Ok(Expr::Field(Scope::File, key))
    }
}
```

`a b` fails because `parse` rejects input left after the expression; `#` fails in the lexer.

- [ ] **Step 5: Run the tests**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add core/src src-tauri/src/error.rs
git commit -m "feat(core): parse the bases expression subset"
```

### Task 4: Evaluating expressions against a note

**Files:**
- Create: `core/src/bases/eval.rs`
- Modify: `core/src/bases/mod.rs` (`pub mod eval;`)
- Test: in-file test module of `eval.rs`

**Interfaces:**
- Consumes: `expr::{Expr, Op, Scope, parse}` from Task 3.
- Produces: `eval::Value { Null, Bool(bool), Num(f64), Str(String), Date(NaiveDateTime), List(Vec<Value>) }`; `eval::NoteData { path: String, mtime_ms: i64, size: i64, properties: serde_json::Map<String, serde_json::Value>, tags: Vec<String>, links: Vec<String> }` where `tags` are without `#` as the index stores them and `links` are resolved note paths; `eval::Ctx<'a> { note: &'a NoteData, formulas: &'a HashMap<String, Expr>, now: NaiveDateTime }` with `fn eval(&self, e: &Expr) -> Result<Value, String>`; `eval::truthy(&Value) -> bool`; `eval::compare(&Value, &Value) -> Option<Ordering>`; `eval::to_json(&Value) -> serde_json::Value`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::expr::parse;
    use serde_json::json;

    fn note() -> NoteData {
        let now = chrono::Local::now();
        NoteData {
            path: "Projects/Alpha.md".into(),
            mtime_ms: now.timestamp_millis() - 3 * 86_400_000,
            size: 120,
            properties: json!({
                "status": "open", "rating": 3, "done": false, "due": "2026-09-20",
                "tags": ["a", "b"], "title": "Alpha note"
            })
            .as_object()
            .unwrap()
            .clone(),
            tags: vec!["project".into(), "project/sub".into()],
            links: vec!["Other/Beta.md".into()],
        }
    }

    fn ev(src: &str) -> Result<Value, String> {
        let n = note();
        let formulas: HashMap<String, Expr> = [
            ("double", "rating * 2"),
            ("loop", "formula.loop"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), parse(v).unwrap()))
        .collect();
        let cx = Ctx {
            note: &n,
            formulas: &formulas,
            now: chrono::Local::now().naive_local(),
        };
        cx.eval(&parse(src).unwrap())
    }

    fn s(v: &str) -> Value {
        Value::Str(v.into())
    }

    #[test]
    fn file_fields_and_methods() {
        assert_eq!(ev("file.name"), Ok(s("Alpha.md")));
        assert_eq!(ev("file.basename"), Ok(s("Alpha")));
        assert_eq!(ev("file.folder"), Ok(s("Projects")));
        assert_eq!(ev("file.ext"), Ok(s("md")));
        assert_eq!(ev("file.size"), Ok(Value::Num(120.0)));
        assert_eq!(
            ev("file.tags"),
            Ok(Value::List(vec![s("#project"), s("#project/sub")]))
        );
        assert_eq!(ev(r#"file.hasTag("project")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r##"file.hasTag("#sub", "proj")"##), Ok(Value::Bool(false)));
        assert_eq!(ev(r#"file.inFolder("Projects")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.inFolder("Proj")"#), Ok(Value::Bool(false)));
        assert_eq!(ev(r#"file.hasLink("beta")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.hasProperty("due")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.mtime > now() - "1 week""#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.mtime > now() - "2d""#), Ok(Value::Bool(false)));
    }

    #[test]
    fn properties_comparisons_and_null() {
        assert_eq!(ev(r#"status == "open""#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"missing != "done""#), Ok(Value::Bool(true)));
        assert_eq!(ev("missing == null"), Ok(Value::Bool(true)));
        assert_eq!(ev("missing > 1"), Ok(Value::Bool(false)));
        assert_eq!(ev(r#"due < date("2026-10-01")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"note["rating"] >= 3 && !done"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"tags.contains("b")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"title.contains("lph")"#), Ok(Value::Bool(true)));
        assert_eq!(ev("missing.isEmpty()"), Ok(Value::Bool(true)));
        assert_eq!(ev("tags.isEmpty()"), Ok(Value::Bool(false)));
    }

    #[test]
    fn arithmetic_formulas_if_and_dates() {
        assert_eq!(ev("formula.double + 1"), Ok(Value::Num(7.0)));
        assert_eq!(ev("7 % 4"), Ok(Value::Num(3.0)));
        assert_eq!(ev(r#""a" + 1"#), Ok(s("a1")));
        assert_eq!(ev(r#"if(done, "yes", "no")"#), Ok(s("no")));
        assert_eq!(ev(r#"if(done, "yes")"#), Ok(Value::Null));
        let d = ev(r#"date("2024-12-01") + "1M" + "4h" + "3m""#).unwrap();
        assert_eq!(to_json(&d), json!("2025-01-01T04:03:00"));
        assert_eq!(to_json(&ev(r#"date("2024-12-01")"#).unwrap()), json!("2024-12-01"));
        assert!(ev("formula.loop").unwrap_err().contains("refers to itself"));
        assert!(ev("formula.nope").unwrap_err().contains("unknown formula"));
        assert!(ev(r#"date("2024-01-01") + "3 fortnights""#).is_err());
    }
}
```

`file.hasLink("beta")` matches `Other/Beta.md` by name, case-insensitive, as a link would resolve.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core eval`
Expected: compile errors.

- [ ] **Step 3: The evaluator**

Add `pub mod eval;` to `core/src/bases/mod.rs`. `core/src/bases/eval.rs` above the tests:

```rust
//! Evaluating an expression against one note.

use super::expr::{Expr, Op, Scope};
use chrono::{Months, NaiveDate, NaiveDateTime, TimeDelta};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Date(NaiveDateTime),
    List(Vec<Value>),
}

/// What an expression sees of one note.
#[derive(Debug, Clone, Default)]
pub struct NoteData {
    pub path: String,
    pub mtime_ms: i64,
    pub size: i64,
    pub properties: serde_json::Map<String, serde_json::Value>,
    /// Without `#`, as the index stores them.
    pub tags: Vec<String>,
    /// Resolved note paths.
    pub links: Vec<String>,
}

pub struct Ctx<'a> {
    pub note: &'a NoteData,
    pub formulas: &'a HashMap<String, Expr>,
    pub now: NaiveDateTime,
}

// Deeper than this, a formula is taken to refer to itself.
const MAX_DEPTH: usize = 32;

static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:-?\d+\s*[A-Za-z]+\s*)+$").unwrap());
static DURATION_PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(-?\d+)\s*([A-Za-z]+)").unwrap());

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Num(n) => *n != 0.0 && !n.is_nan(),
        Value::Str(s) => !s.is_empty(),
        Value::Date(_) => true,
        Value::List(l) => !l.is_empty(),
    }
}

pub fn from_json(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => n.as_f64().map_or(Value::Null, Value::Num),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(from_json).collect()),
        serde_json::Value::Null | serde_json::Value::Object(_) => Value::Null,
    }
}

fn parse_date(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0);
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(s, f).ok())
}

fn as_date(v: &Value) -> Option<NaiveDateTime> {
    match v {
        Value::Date(d) => Some(*d),
        Value::Str(s) => parse_date(s),
        _ => None,
    }
}

fn is_midnight(d: &NaiveDateTime) -> bool {
    d.time() == chrono::NaiveTime::MIN
}

pub fn display(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
        Value::Num(n) => n.to_string(),
        Value::Str(s) => s.clone(),
        Value::Date(d) if is_midnight(d) => d.format("%Y-%m-%d").to_string(),
        Value::Date(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::List(l) => l.iter().map(display).collect::<Vec<_>>().join(", "),
    }
}

/// A cell value: dates in the form the property editors read.
pub fn to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => (*b).into(),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => (*n as i64).into(),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, Into::into),
        Value::Str(s) => s.clone().into(),
        Value::Date(d) if is_midnight(d) => d.format("%Y-%m-%d").to_string().into(),
        Value::Date(d) => d.format("%Y-%m-%dT%H:%M:%S").to_string().into(),
        Value::List(l) => l.iter().map(to_json).collect::<Vec<_>>().into(),
    }
}

/// Loose equality: a date equals a string naming the same moment.
pub fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Date(_), Value::Str(_)) | (Value::Str(_), Value::Date(_)) => {
            as_date(a).is_some() && as_date(a) == as_date(b)
        }
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equal(p, q))
        }
        _ => a == b,
    }
}

pub fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Date(_), _) | (_, Value::Date(_)) => Some(as_date(a)?.cmp(&as_date(b)?)),
        (Value::Str(x), Value::Str(y)) => Some(x.to_lowercase().cmp(&y.to_lowercase())),
        _ => None,
    }
}

/// `date + "1M"`: Obsidian's duration units, several parts allowed.
fn shift(date: NaiveDateTime, text: &str, sign: i64) -> Result<NaiveDateTime, String> {
    let bad = || format!("`{text}` is not a duration");
    if !DURATION.is_match(text) {
        return Err(bad());
    }
    let mut out = date;
    for c in DURATION_PART.captures_iter(text) {
        let n: i64 = c[1].parse::<i64>().map_err(|_| bad())? * sign;
        let unit = &c[2];
        let months = match unit {
            "M" => Some(1),
            "y" => Some(12),
            _ => match unit.to_lowercase().as_str() {
                "year" | "years" => Some(12),
                "month" | "months" => Some(1),
                _ => None,
            },
        };
        if let Some(m) = months {
            let span = Months::new((n.unsigned_abs() * m) as u32);
            out = if n < 0 { out.checked_sub_months(span) } else { out.checked_add_months(span) }
                .ok_or_else(bad)?;
            continue;
        }
        let secs = match unit.to_lowercase().as_str() {
            "w" | "week" | "weeks" => 604_800,
            "d" | "day" | "days" => 86_400,
            "h" | "hour" | "hours" => 3_600,
            "m" | "minute" | "minutes" => 60,
            "s" | "second" | "seconds" => 1,
            _ => return Err(bad()),
        };
        out = out
            .checked_add_signed(TimeDelta::seconds(n * secs))
            .ok_or_else(bad)?;
    }
    Ok(out)
}

fn names_link(link_path: &str, target: &str) -> bool {
    let l = link_path.to_lowercase();
    let l = l.strip_suffix(".md").unwrap_or(&l);
    let t = target.trim().to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t);
    l == t || l.rsplit('/').next() == Some(t)
}

impl Ctx<'_> {
    pub fn eval(&self, e: &Expr) -> Result<Value, String> {
        self.at(e, 0)
    }

    fn at(&self, e: &Expr, depth: usize) -> Result<Value, String> {
        let ev = |x: &Expr| self.at(x, depth);
        Ok(match e {
            Expr::Null => Value::Null,
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Field(Scope::Note, k) => self
                .note
                .properties
                .get(k)
                .map_or(Value::Null, from_json),
            Expr::Field(Scope::Formula, k) => {
                let f = self
                    .formulas
                    .get(k)
                    .ok_or_else(|| format!("unknown formula `{k}`"))?;
                if depth >= MAX_DEPTH {
                    return Err(format!("formula `{k}` refers to itself"));
                }
                self.at(f, depth + 1)?
            }
            Expr::Field(Scope::File, k) => self.file_field(k),
            Expr::Unary('!', x) => Value::Bool(!truthy(&ev(x)?)),
            Expr::Unary(_, x) => match ev(x)? {
                Value::Num(n) => Value::Num(-n),
                _ => Value::Null,
            },
            Expr::Binary(l, op, r) => self.binary(ev(l)?, *op, r, depth)?,
            Expr::Call(name, args) => self.call(name, args, depth)?,
            Expr::Method(recv, name, args) => {
                let v = ev(recv)?;
                let args = args.iter().map(ev).collect::<Result<Vec<_>, _>>()?;
                match (name.as_str(), args.as_slice()) {
                    ("isEmpty", []) => Value::Bool(!truthy(&v) && !matches!(v, Value::Bool(_) | Value::Num(_))),
                    ("contains", [x]) => Value::Bool(match (&v, x) {
                        (Value::Str(s), Value::Str(n)) => s.contains(n.as_str()),
                        (Value::List(l), x) => l.iter().any(|i| equal(i, x)),
                        _ => false,
                    }),
                    _ => return Err(format!("`.{name}()` got {} arguments", args.len())),
                }
            }
        })
    }

    fn file_field(&self, k: &str) -> Value {
        let path = &self.note.path;
        let name = path.rsplit('/').next().unwrap_or(path);
        match k {
            "name" => Value::Str(name.into()),
            "basename" => Value::Str(name.rsplit_once('.').map_or(name, |(b, _)| b).into()),
            "path" => Value::Str(path.clone()),
            // Obsidian's root folder is `/`.
            "folder" => Value::Str(path.rsplit_once('/').map_or("/", |(f, _)| f).into()),
            "ext" => Value::Str(name.rsplit_once('.').map_or("", |(_, e)| e).into()),
            "size" => Value::Num(self.note.size as f64),
            "mtime" => chrono::DateTime::from_timestamp_millis(self.note.mtime_ms)
                .map_or(Value::Null, |d| Value::Date(d.with_timezone(&chrono::Local).naive_local())),
            "tags" => Value::List(self.note.tags.iter().map(|t| Value::Str(format!("#{t}"))).collect()),
            "links" => Value::List(self.note.links.iter().map(|l| Value::Str(l.clone())).collect()),
            _ => Value::Null,
        }
    }

    fn binary(&self, a: Value, op: Op, r: &Expr, depth: usize) -> Result<Value, String> {
        // `&&` and `||` evaluate their right side only when it matters.
        match op {
            Op::And if !truthy(&a) => return Ok(Value::Bool(false)),
            Op::Or if truthy(&a) => return Ok(Value::Bool(true)),
            _ => {}
        }
        let b = self.at(r, depth)?;
        let ord = || compare(&a, &b);
        Ok(match op {
            Op::And | Op::Or => Value::Bool(truthy(&b)),
            Op::Eq => Value::Bool(equal(&a, &b)),
            Op::Ne => Value::Bool(!equal(&a, &b)),
            Op::Lt => Value::Bool(ord() == Some(Ordering::Less)),
            Op::Le => Value::Bool(matches!(ord(), Some(Ordering::Less | Ordering::Equal))),
            Op::Gt => Value::Bool(ord() == Some(Ordering::Greater)),
            Op::Ge => Value::Bool(matches!(ord(), Some(Ordering::Greater | Ordering::Equal))),
            Op::Add | Op::Sub => match (&a, &b, op) {
                (Value::Num(x), Value::Num(y), Op::Add) => Value::Num(x + y),
                (Value::Num(x), Value::Num(y), _) => Value::Num(x - y),
                (Value::Date(d), Value::Str(s), _) => {
                    Value::Date(shift(*d, s, if op == Op::Add { 1 } else { -1 })?)
                }
                (Value::Date(x), Value::Date(y), Op::Sub) => {
                    Value::Num((*x - *y).num_milliseconds() as f64)
                }
                (Value::Str(_), _, Op::Add) | (_, Value::Str(_), Op::Add) => {
                    Value::Str(display(&a) + &display(&b))
                }
                _ => Value::Null,
            },
            Op::Mul | Op::Div | Op::Mod => match (&a, &b) {
                (Value::Num(x), Value::Num(y)) => Value::Num(match op {
                    Op::Mul => x * y,
                    Op::Div => x / y,
                    _ => x % y,
                }),
                _ => Value::Null,
            },
        })
    }

    fn call(&self, name: &str, args: &[Expr], depth: usize) -> Result<Value, String> {
        let ev = |x: &Expr| self.at(x, depth);
        let arity = |n: std::ops::RangeInclusive<usize>| {
            if n.contains(&args.len()) {
                Ok(())
            } else {
                Err(format!("`{name}()` got {} arguments", args.len()))
            }
        };
        let strings = || -> Result<Vec<String>, String> {
            args.iter().map(|a| ev(a).map(|v| display(&v))).collect()
        };
        Ok(match name {
            "now" => {
                arity(0..=0)?;
                Value::Date(self.now)
            }
            "today" => {
                arity(0..=0)?;
                Value::Date(self.now.date().and_time(chrono::NaiveTime::MIN))
            }
            "date" => {
                arity(1..=1)?;
                as_date(&ev(&args[0])?).map_or(Value::Null, Value::Date)
            }
            "if" => {
                arity(2..=3)?;
                if truthy(&ev(&args[0])?) {
                    ev(&args[1])?
                } else if let Some(f) = args.get(2) {
                    ev(f)?
                } else {
                    Value::Null
                }
            }
            "file.hasTag" => {
                arity(1..=usize::MAX)?;
                let tags = &self.note.tags;
                Value::Bool(strings()?.iter().any(|want| {
                    let want = want.trim_start_matches('#').to_lowercase();
                    tags.iter().any(|t| *t == want || t.starts_with(&format!("{want}/")))
                }))
            }
            "file.inFolder" => {
                arity(1..=1)?;
                let folder = strings()?.remove(0);
                let folder = folder.trim_matches('/');
                Value::Bool(folder.is_empty() || self.note.path.starts_with(&format!("{folder}/")))
            }
            "file.hasLink" => {
                arity(1..=1)?;
                let target = strings()?.remove(0);
                Value::Bool(self.note.links.iter().any(|l| names_link(l, &target)))
            }
            "file.hasProperty" => {
                arity(1..=1)?;
                Value::Bool(self.note.properties.contains_key(&strings()?.remove(0)))
            }
            _ => return Err(format!("unsupported function `{name}()`")),
        })
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: pass. If clippy flags a closure as redundant, apply its suggestion.

- [ ] **Step 5: Commit**

```bash
git add core/src/bases
git commit -m "feat(core): evaluate base expressions with dates and durations"
```

### Task 5: `.base` files and running a table view

**Files:**
- Modify: `core/src/bases/mod.rs`
- Test: in-file test module of `bases/mod.rs`

**Interfaces:**
- Consumes: `expr::parse`, `eval::{Ctx, NoteData, Value, truthy, compare, to_json}`.
- Produces:
  - `bases::notes(index: &Index) -> Result<Vec<NoteData>>`, every note ordered by path.
  - `bases::run(text: &str, notes: &[NoteData], view: usize, now: NaiveDateTime) -> Result<Table>`; a YAML error is `Error::Base`.
  - `Table { views: Vec<String>, view: usize, columns: Vec<Column>, rows: Vec<Row>, sort: Vec<SortKey>, errors: Vec<String> }`, `Column { id: String, label: String, editable: bool }`, `Row { path: String, cells: Vec<serde_json::Value> }`, `SortKey { property: String, direction: String }` (serde both ways).
  - `bases::set_sort(text: &str, view: usize, sort: &[SortKey]) -> Result<String>`.
  - `bases::NEW_BASE: &str`, the text a new base file starts with.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data() -> Vec<NoteData> {
        let props = |v: serde_json::Value| v.as_object().unwrap().clone();
        vec![
            NoteData {
                path: "Books/Dune.md".into(),
                properties: props(json!({"status": "reading", "rating": 5})),
                tags: vec!["book".into()],
                ..Default::default()
            },
            NoteData {
                path: "Books/Emma.md".into(),
                properties: props(json!({"status": "done", "rating": 3})),
                tags: vec!["book".into(), "classic".into()],
                ..Default::default()
            },
            NoteData {
                path: "Inbox.md".into(),
                ..Default::default()
            },
        ]
    }

    fn now() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 9, 13)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    const BOOKS: &str = r#"
filters:
  and:
    - file.hasTag("book")
formulas:
  double: "rating * 2"
properties:
  note.status:
    displayName: State
views:
  - type: table
    name: Reading
    filters:
      not:
        - 'status == "done"'
    order: [file.name, note.status, formula.double]
  - type: table
    name: All books
    order: [file.basename, rating]
    sort:
      - property: rating
        direction: ASC
    limit: 1
"#;

    #[test]
    fn filters_columns_formulas() {
        let t = run(BOOKS, &data(), 0, now()).unwrap();
        assert_eq!(t.views, vec!["Reading", "All books"]);
        let cols: Vec<_> = t
            .columns
            .iter()
            .map(|c| (c.id.as_str(), c.label.as_str(), c.editable))
            .collect();
        assert_eq!(
            cols,
            vec![
                ("file.name", "file name", false),
                ("note.status", "State", true),
                ("formula.double", "double", false)
            ]
        );
        assert_eq!(
            t.rows,
            vec![Row {
                path: "Books/Dune.md".into(),
                cells: vec![json!("Dune.md"), json!("reading"), json!(10)]
            }]
        );
        assert!(t.errors.is_empty(), "{:?}", t.errors);
    }

    #[test]
    fn sort_and_limit_per_view() {
        let t = run(BOOKS, &data(), 1, now()).unwrap();
        assert_eq!(t.view, 1);
        assert_eq!(
            t.rows,
            vec![Row {
                path: "Books/Emma.md".into(),
                cells: vec![json!("Emma"), json!(3)]
            }]
        );
        assert_eq!(t.columns[1].id, "note.rating");
        assert_eq!(t.sort[0].direction, "ASC");
    }

    #[test]
    fn empty_base_lists_every_note_by_name() {
        let t = run("", &data(), 0, now()).unwrap();
        assert_eq!(t.views, vec!["Table"]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.columns[0].id, "file.name");
    }

    #[test]
    fn unsupported_is_reported_not_guessed() {
        let text = "filters: 'file.ctime > now()'\nformulas:\n  x: 'link(\"a\")'\nviews:\n  - type: cards\n    groupBy:\n      property: note.status\n      direction: ASC\n";
        let t = run(text, &data(), 0, now()).unwrap();
        assert!(t.rows.is_empty());
        let all = t.errors.join("\n");
        for what in ["file.ctime", "link()", "cards", "groupBy"] {
            assert!(all.contains(what), "{what} missing from {all}");
        }
        assert!(run("views: [", &data(), 0, now()).is_err());
    }

    #[test]
    fn set_sort_keeps_the_rest() {
        let key = SortKey {
            property: "file.name".into(),
            direction: "DESC".into(),
        };
        let out = set_sort(BOOKS, 0, std::slice::from_ref(&key)).unwrap();
        let t = run(&out, &data(), 0, now()).unwrap();
        assert_eq!(t.sort, vec![key]);
        assert_eq!(t.views, vec!["Reading", "All books"]);
        assert_eq!(t.rows.len(), 1);
        let out = set_sort(&out, 0, &[]).unwrap();
        assert!(run(&out, &data(), 0, now()).unwrap().sort.is_empty());
        assert!(set_sort("", 0, &[]).is_ok());
    }

    #[test]
    fn notes_come_from_the_index() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "---\nstatus: open\n---\n[[B]] #x").unwrap();
        std::fs::write(d.path().join("B.md"), "").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let n = notes(&ix).unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].properties["status"], json!("open"));
        assert_eq!(n[0].tags, vec!["x"]);
        assert_eq!(n[0].links, vec!["B.md"]);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core bases::tests`
Expected: compile errors, `run` not found.

- [ ] **Step 3: The file model, runner and sort writer**

Replace `core/src/bases/mod.rs` above the tests:

```rust
//! Obsidian's `.base` files: filters and formulas over notes, shown as views.

pub mod eval;
pub mod expr;

use crate::index::Index;
use crate::{Error, Result};
use chrono::NaiveDateTime;
use eval::{Ctx, NoteData, Value};
use expr::{Expr, Scope};
use std::collections::{BTreeMap, HashMap};

/// What Obsidian writes for a new base.
pub const NEW_BASE: &str = "views:\n  - type: table\n    name: Table\n";

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct BaseFile {
    filters: Option<Filter>,
    formulas: BTreeMap<String, String>,
    properties: BTreeMap<String, PropertyConfig>,
    summaries: Option<serde_yaml_ng::Value>,
    views: Vec<View>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum Filter {
    Expr(String),
    And { and: Vec<Filter> },
    Or { or: Vec<Filter> },
    Not { not: Vec<Filter> },
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct PropertyConfig {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct View {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    limit: Option<usize>,
    filters: Option<Filter>,
    order: Vec<String>,
    sort: Vec<SortKey>,
    #[serde(rename = "groupBy")]
    group_by: Option<serde_yaml_ng::Value>,
    summaries: Option<serde_yaml_ng::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SortKey {
    pub property: String,
    #[serde(default = "ascending")]
    pub direction: String,
}

fn ascending() -> String {
    "ASC".into()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Column {
    pub id: String,
    pub label: String,
    /// Note properties edit the note's frontmatter; file fields and formulas do not.
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Row {
    pub path: String,
    pub cells: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Table {
    pub views: Vec<String>,
    pub view: usize,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub sort: Vec<SortKey>,
    /// What the base asks for outside the supported subset.
    pub errors: Vec<String>,
}

enum Cond {
    Expr(Expr),
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Vec<Cond>),
}

fn compile(f: &Filter) -> std::result::Result<Cond, String> {
    let all = |fs: &[Filter]| fs.iter().map(compile).collect::<std::result::Result<Vec<_>, _>>();
    Ok(match f {
        Filter::Expr(s) => Cond::Expr(expr::parse(s)?),
        Filter::And { and } => Cond::And(all(and)?),
        Filter::Or { or } => Cond::Or(all(or)?),
        Filter::Not { not } => Cond::Not(all(not)?),
    })
}

// `not` holds when none of its filters do.
fn holds(c: &Cond, cx: &Ctx) -> std::result::Result<bool, String> {
    Ok(match c {
        Cond::Expr(e) => eval::truthy(&cx.eval(e)?),
        Cond::And(cs) => cs.iter().try_fold(true, |ok, c| Ok::<_, String>(ok && holds(c, cx)?))?,
        Cond::Or(cs) => cs.iter().try_fold(false, |ok, c| Ok::<_, String>(ok || holds(c, cx)?))?,
        Cond::Not(cs) => !cs.iter().try_fold(false, |ok, c| Ok::<_, String>(ok || holds(c, cx)?))?,
    })
}

/// A column id as an expression; a bare name is a note property.
fn column(id: &str) -> std::result::Result<(String, Expr), String> {
    match id.split_once('.') {
        Some(("file", _)) => Ok((id.to_owned(), expr::parse(id)?)),
        Some(("note", k)) => Ok((id.to_owned(), Expr::Field(Scope::Note, k.into()))),
        Some(("formula", k)) => Ok((id.to_owned(), Expr::Field(Scope::Formula, k.into()))),
        _ => Ok((format!("note.{id}"), Expr::Field(Scope::Note, id.into()))),
    }
}

fn yaml_err(e: serde_yaml_ng::Error) -> Error {
    Error::Base(e.to_string())
}

fn parse_base(text: &str) -> Result<BaseFile> {
    if text.trim().is_empty() {
        return Ok(BaseFile::default());
    }
    serde_yaml_ng::from_str(text).map_err(yaml_err)
}

fn push_once(errors: &mut Vec<String>, e: String) {
    if !errors.contains(&e) {
        errors.push(e);
    }
}

pub fn run(text: &str, notes: &[NoteData], view: usize, now: NaiveDateTime) -> Result<Table> {
    let base = parse_base(text)?;
    let mut views = base.views.clone();
    if views.is_empty() {
        views.push(View {
            kind: "table".into(),
            name: "Table".into(),
            ..Default::default()
        });
    }
    let view = if view < views.len() { view } else { 0 };
    let v = &views[view];
    let mut errors = Vec::new();
    if !v.kind.is_empty() && v.kind != "table" {
        errors.push(format!("unsupported view type `{}`, shown as a table", v.kind));
    }
    if v.group_by.is_some() {
        errors.push("unsupported `groupBy`, rows are not grouped".into());
    }
    if v.summaries.is_some() || base.summaries.is_some() {
        errors.push("unsupported `summaries`".into());
    }

    let mut formulas = HashMap::new();
    for (name, src) in &base.formulas {
        match expr::parse(src) {
            Ok(e) => {
                formulas.insert(name.clone(), e);
            }
            Err(e) => errors.push(format!("formula `{name}`: {e}")),
        }
    }
    let mut conds = Vec::new();
    let mut filter_failed = false;
    for f in [&base.filters, &v.filters].into_iter().flatten() {
        match compile(f) {
            Ok(c) => conds.push(c),
            Err(e) => {
                errors.push(format!("filter: {e}"));
                filter_failed = true;
            }
        }
    }

    let order = if v.order.is_empty() { vec!["file.name".to_owned()] } else { v.order.clone() };
    let mut columns = Vec::new();
    let mut exprs = Vec::new();
    for id in &order {
        let (id, e) = match column(id) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("column `{id}`: {e}"));
                (id.clone(), Expr::Null)
            }
        };
        let (scope, key) = id.split_once('.').unwrap_or(("note", &id));
        let label = base
            .properties
            .get(&id)
            .or_else(|| base.properties.get(key).filter(|_| scope == "note"))
            .and_then(|p| p.display_name.clone())
            .unwrap_or_else(|| if scope == "file" { format!("file {key}") } else { key.to_owned() });
        columns.push(Column { editable: scope == "note", label, id: id.clone() });
        exprs.push(e);
    }
    let sort_exprs: Vec<Option<Expr>> = v.sort.iter().map(|k| column(&k.property).ok().map(|c| c.1)).collect();

    let mut rows: Vec<(Vec<Value>, Row)> = Vec::new();
    if !filter_failed {
        for note in notes {
            let cx = Ctx { note, formulas: &formulas, now };
            let keep = conds.iter().try_fold(true, |ok, c| Ok::<_, String>(ok && holds(c, &cx)?));
            match keep {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    push_once(&mut errors, e);
                    continue;
                }
            }
            let mut cells = Vec::new();
            for (c, e) in columns.iter().zip(&exprs) {
                // Note properties pass through as written, so editors see the file's value.
                let cell = match (c.editable, c.id.split_once('.')) {
                    (true, Some((_, key))) => note.properties.get(key).cloned().unwrap_or_default(),
                    _ => cx.eval(e).map(|v| eval::to_json(&v)).unwrap_or_else(|err| {
                        push_once(&mut errors, err);
                        serde_json::Value::Null
                    }),
                };
                cells.push(cell);
            }
            let keys = sort_exprs
                .iter()
                .map(|e| e.as_ref().and_then(|e| cx.eval(e).ok()).unwrap_or(Value::Null))
                .collect();
            rows.push((keys, Row { path: note.path.clone(), cells }));
        }
    }
    rows.sort_by(|(a, _), (b, _)| {
        for (i, k) in v.sort.iter().enumerate() {
            let (x, y) = (&a[i], &b[i]);
            // Empty values sort last in either direction.
            let ord = match (x == &Value::Null, y == &Value::Null) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => {
                    let o = eval::compare(x, y)
                        .unwrap_or_else(|| eval::display(x).cmp(&eval::display(y)));
                    if k.direction.eq_ignore_ascii_case("desc") { o.reverse() } else { o }
                }
            };
            if ord.is_ne() {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    let limit = v.limit.unwrap_or(usize::MAX);
    Ok(Table {
        views: views
            .iter()
            .enumerate()
            .map(|(i, v)| if v.name.is_empty() { format!("View {}", i + 1) } else { v.name.clone() })
            .collect(),
        view,
        columns,
        rows: rows.into_iter().take(limit).map(|(_, r)| r).collect(),
        sort: v.sort.clone(),
        errors,
    })
}

/// The base text with one view's sort replaced; Obsidian rewrites the file the same way.
pub fn set_sort(text: &str, view: usize, sort: &[SortKey]) -> Result<String> {
    use serde_yaml_ng::{Mapping, Value as Yaml};
    let mut doc = if text.trim().is_empty() {
        Yaml::Mapping(Mapping::new())
    } else {
        serde_yaml_ng::from_str(text).map_err(yaml_err)?
    };
    let not_a_base = || Error::Base("a base file must be a mapping with a list of views".into());
    let map = doc.as_mapping_mut().ok_or_else(not_a_base)?;
    if !map.contains_key("views") {
        map.insert("views".into(), Yaml::Sequence(vec![]));
    }
    let views = map.get_mut("views").and_then(Yaml::as_sequence_mut).ok_or_else(not_a_base)?;
    if views.is_empty() {
        let mut m = Mapping::new();
        m.insert("type".into(), "table".into());
        m.insert("name".into(), "Table".into());
        views.push(Yaml::Mapping(m));
    }
    let v = views
        .get_mut(view)
        .and_then(Yaml::as_mapping_mut)
        .ok_or_else(|| Error::Base(format!("no view {view}")))?;
    if sort.is_empty() {
        v.remove("sort");
    } else {
        v.insert("sort".into(), serde_yaml_ng::to_value(sort).map_err(yaml_err)?);
    }
    serde_yaml_ng::to_string(&doc).map_err(yaml_err)
}

pub fn notes(index: &Index) -> Result<Vec<NoteData>> {
    let conn = index.conn();
    let pairs = |sql: &str| -> Result<HashMap<String, Vec<String>>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        let mut stmt = conn.prepare(sql)?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = row?;
            out.entry(k).or_default().push(v);
        }
        Ok(out)
    };
    let mut tags = pairs("SELECT path, tag FROM tags ORDER BY tag")?;
    let mut links = pairs(
        "SELECT src_path, target_path FROM links WHERE target_path IS NOT NULL ORDER BY src_path, start",
    )?;
    let mut stmt = conn.prepare("SELECT path, mtime_ms, size, frontmatter FROM notes ORDER BY path")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (path, mtime_ms, size, fm) = row?;
        out.push(NoteData {
            properties: serde_json::from_str(&fm).unwrap_or_default(),
            tags: tags.remove(&path).unwrap_or_default(),
            links: links.remove(&path).unwrap_or_default(),
            path,
            mtime_ms,
            size,
        });
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add core/src/bases
git commit -m "feat(core): run base table views and write their sort"
```

### Task 6: Tauri commands for folders, the graph and bases

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)
- Modify: `src-tauri/tauri.conf.json` (`dragDropEnabled: false`)
- Test: `src-tauri/src/commands.rs` test module

**Interfaces:**
- Consumes: `Vault::folders`, `graph::build`, `config::{load_graph, save_graph}`, `bases::{notes, run, set_sort, SortKey, Table}`.
- Produces (frontend-facing, argument names as the UI passes them):
  - `list_folders() -> Vec<String>`
  - `graph() -> Graph` (`{nodes: [{id, title, kind, tags}], edges: [{source, target}]}`)
  - `get_graph_config() -> Value`, `set_graph_config(config: Value)`
  - `run_base(path: String, view: usize) -> Table`
  - `set_base_sort(path: String, view: usize, sort: Vec<SortKey>)`
  - `delete_file(path)` already trashes folders; `remove_file` now drops the notes below them.

- [ ] **Step 1: Write the failing serialisation test**

In the `commands.rs` test module:

```rust
    #[test]
    fn graph_and_base_table() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "---\ns: 1\n---\n[[Ghost]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let g = engram_core::graph::build(&ix, &v.walk().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            json!({
                "nodes": [
                    {"id": "A.md", "title": "A", "kind": "note", "tags": []},
                    {"id": "Ghost", "title": "Ghost", "kind": "unresolved", "tags": []}
                ],
                "edges": [{"source": "A.md", "target": "Ghost"}]
            })
        );
        let notes = engram_core::bases::notes(&ix).unwrap();
        let now = chrono::Local::now().naive_local();
        let t = engram_core::bases::run("views:\n  - type: table\n    order: [file.basename, s]\n", &notes, 0, now).unwrap();
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            json!({
                "views": ["View 1"], "view": 0,
                "columns": [
                    {"id": "file.basename", "label": "file basename", "editable": false},
                    {"id": "note.s", "label": "s", "editable": true}
                ],
                "rows": [{"path": "A.md", "cells": ["A", 1]}],
                "sort": [], "errors": []
            })
        );
        let k: engram_core::bases::SortKey =
            serde_json::from_value(json!({"property": "s", "direction": "DESC"})).unwrap();
        assert_eq!(k.direction, "DESC");
    }
```

Add `tempfile = "3"` under `[dev-dependencies]` in `src-tauri/Cargo.toml` if it is not there.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p engram-notes graph_and_base_table`
Expected: fails to compile until `tempfile` is a dev-dependency, or passes already, since it only uses core. Either way the commands below are still missing; the test guards the JSON shapes the UI relies on.

- [ ] **Step 3: The commands**

In `src-tauri/src/commands.rs`, after `create_folder`:

```rust
#[tauri::command]
pub fn list_folders(state: State<AppState>) -> CmdResult<Vec<String>> {
    with_open(&state, |o| Ok(o.vault.folders()?))
}

#[tauri::command]
pub fn graph(state: State<AppState>) -> CmdResult<Graph> {
    with_open(&state, |o| {
        Ok(engram_core::graph::build(&o.index, &o.vault.walk()?)?)
    })
}

#[tauri::command]
pub fn get_graph_config(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_open(&state, |o| Ok(config::load_graph(&o.vault)?))
}

#[tauri::command]
pub fn set_graph_config(state: State<AppState>, config: serde_json::Value) -> CmdResult<()> {
    with_open(&state, |o| Ok(config::save_graph(&o.vault, &config)?))
}

#[tauri::command]
pub fn run_base(state: State<AppState>, path: String, view: usize) -> CmdResult<Table> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let notes = engram_core::bases::notes(&o.index)?;
        let now = chrono::Local::now().naive_local();
        Ok(engram_core::bases::run(&text, &notes, view, now)?)
    })
}

#[tauri::command]
pub fn set_base_sort(
    state: State<AppState>,
    path: String,
    view: usize,
    sort: Vec<SortKey>,
) -> CmdResult<()> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        o.vault
            .write(&path, &engram_core::bases::set_sort(&text, view, &sort)?)?;
        Ok(())
    })
}
```

with imports `use engram_core::bases::{SortKey, Table};` and `use engram_core::graph::Graph;`. Register `list_folders`, `graph`, `get_graph_config`, `set_graph_config`, `run_base`, `set_base_sort` in `src-tauri/src/lib.rs`.

In `src-tauri/tauri.conf.json` the window gains `"dragDropEnabled": false`: Tauri's own file-drop handler otherwise swallows HTML drag and drop events, which the explorer needs.

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(app): commands for folders, the graph and bases"
```

### Task 7: Frontend helpers for folders, files and the new commands

Pure functions the explorer, image embeds and tabs need, with tests, plus the API wrappers and the screenshot mock for everything this plan adds.

**Files:**
- Create: `ui/src/lib/tree.ts`, `ui/src/lib/tree.test.ts`
- Modify: `ui/src/lib/files.ts`, `ui/src/lib/files.test.ts`
- Modify: `ui/src/lib/layout.ts`, `ui/src/lib/layout.test.ts`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/state.svelte.ts`
- Modify: `ui/src/lib/commands.ts` (`newNote` uses `freeName`; add `newBase`)
- Modify: `ui/src/components/Tabs.svelte` (titles through `tabTitle`)
- Modify: `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: the Task 6 commands.
- Produces:
  - `files.ts`: `FileKind` adds `"base" | "graph"`; `resolveFile(files: FileEntry[], target: string): string | null`; `imageSize(alias?: string): { width?: number; height?: number }`; `tabTitle(path: string): string`.
  - `layout.ts`: `under(path: string, dir: string): boolean`; `renamePath` and `withoutPath` act on everything under a folder.
  - `tree.ts`: `TreeDir { name, path, dirs: TreeDir[], files: TreeFile[] }`, `TreeFile { name, path }`, `buildTree(files, folders): TreeDir`, `basename(p)`, `parent(p)`, `join(dir, name)`, `dropTarget(src, dir): string | null`, `renameTarget(path, name, isNote): string | null` (throws on a forbidden character), `freeName(taken: string[], dir: string, base: string, ext?: string): string`.
  - `api.ts`: `GraphNode`, `GraphEdge`, `Graph`, `SortKey`, `Column`, `BaseRow`, `BaseTable`; `listFolders`, `graph`, `getGraphConfig`, `setGraphConfig`, `runBase`, `setBaseSort`.
  - `state.svelte.ts`: `app.folders: string[]`, `app.lastNote: string | null`; `renamed` and `forget` cover folders.
  - `commands.ts`: `newBase(dir?: string)`, `NEW_BASE`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/tree.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildTree, dropTarget, freeName, renameTarget } from "./tree";

const f = (path: string) => ({ path, mtime_ms: 0, size: 0, is_markdown: path.endsWith(".md") });

describe("explorer tree", () => {
  it("shows empty folders, folders first, notes without .md", () => {
    const t = buildTree([f("b/n.md"), f("a.md"), f("pic.png")], ["b", "b/empty", "c"]);
    expect(t.dirs.map((d) => d.path)).toEqual(["b", "c"]);
    expect(t.dirs[0].dirs.map((d) => d.path)).toEqual(["b/empty"]);
    expect(t.dirs[0].files).toEqual([{ name: "n", path: "b/n.md" }]);
    expect(t.files.map((x) => x.name)).toEqual(["a", "pic.png"]);
  });

  it("drops into another folder, never into itself or where it already is", () => {
    expect(dropTarget("a/n.md", "b")).toBe("b/n.md");
    expect(dropTarget("a/n.md", "")).toBe("n.md");
    expect(dropTarget("a/n.md", "a")).toBeNull();
    expect(dropTarget("a", "a/sub")).toBeNull();
    expect(dropTarget("a", "a")).toBeNull();
    expect(dropTarget("ab", "a")).toBe("a/ab");
  });

  it("renames in place, implying .md for notes", () => {
    expect(renameTarget("d/Old.md", "New", true)).toBe("d/New.md");
    expect(renameTarget("pic.png", "photo.png", false)).toBe("photo.png");
    expect(renameTarget("d/sub", "other", false)).toBe("d/other");
    expect(renameTarget("d/Old.md", " Old ", true)).toBeNull();
    expect(renameTarget("d/Old.md", "", true)).toBeNull();
    expect(() => renameTarget("d/Old.md", "a/b", true)).toThrow();
  });

  it("finds a free Untitled name", () => {
    expect(freeName(["Untitled.md", "d/Untitled.md"], "", "Untitled", ".md")).toBe("Untitled 1.md");
    expect(freeName(["d/Untitled"], "d", "Untitled")).toBe("d/Untitled 1");
  });
});
```

Append to `ui/src/lib/files.test.ts` (import the new names too):

```ts
describe("file resolution and titles", () => {
  const files = ["deep/er/pic.png", "img/pic.png", "Notes/a.md"].map((path) => ({ path, mtime_ms: 0, size: 0, is_markdown: path.endsWith(".md") }));
  it("resolves exact paths, then the shortest path with that name", () => {
    expect(resolveFile(files, "deep/er/pic.png")).toBe("deep/er/pic.png");
    expect(resolveFile(files, "PIC.png")).toBe("img/pic.png");
    expect(resolveFile(files, "gone.png")).toBeNull();
  });
  it("reads Obsidian's image sizes", () => {
    expect(imageSize("300")).toEqual({ width: 300 });
    expect(imageSize("300x200")).toEqual({ width: 300, height: 200 });
    expect(imageSize("a caption")).toEqual({});
    expect(imageSize()).toEqual({});
  });
  it("knows bases and graph tabs", () => {
    expect(fileKind("x/Books.base")).toBe("base");
    expect(fileKind("graph:local")).toBe("graph");
    expect(tabTitle("x/Books.base")).toBe("Books");
    expect(tabTitle("graph:global")).toBe("Graph view");
    expect(tabTitle("graph:local")).toBe("Local graph");
    expect(tabTitle("a/Note.md")).toBe("Note");
  });
});
```

Append to `ui/src/lib/layout.test.ts`:

```ts
describe("folders in the layout", () => {
  it("renames and closes everything under a folder", () => {
    const root: Node = {
      kind: "pane", id: 1, active: 2,
      tabs: [{ path: "d/a.md", mode: "live" }, { path: "d.md", mode: "live" }, { path: "d/e/b.md", mode: "live" }],
    };
    expect(panes(renamePath(root, "d", "x/d"))[0].tabs.map((t) => t.path)).toEqual(["x/d/a.md", "d.md", "x/d/e/b.md"]);
    expect(panes(withoutPath(root, "d"))[0].tabs.map((t) => t.path)).toEqual(["d.md"]);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test`
Expected: FAIL, `./tree` not found and `resolveFile` not exported.

- [ ] **Step 3: `tree.ts`**

```ts
import type { FileEntry } from "./api";
import { under } from "./layout";

export interface TreeFile { name: string; path: string }
export interface TreeDir { name: string; path: string; dirs: TreeDir[]; files: TreeFile[] }

export const basename = (p: string) => p.slice(p.lastIndexOf("/") + 1);
export const parent = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "");
export const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

/** The explorer's tree: every folder, even empty ones, and notes shown without `.md`. */
export function buildTree(files: FileEntry[], folders: string[]): TreeDir {
  const root: TreeDir = { name: "", path: "", dirs: [], files: [] };
  const dirAt = (path: string): TreeDir => {
    let node = root;
    if (!path) return node;
    const parts = path.split("/");
    parts.forEach((name, i) => {
      let d = node.dirs.find((x) => x.name === name);
      if (!d) {
        d = { name, path: parts.slice(0, i + 1).join("/"), dirs: [], files: [] };
        node.dirs.push(d);
      }
      node = d;
    });
    return node;
  };
  for (const dir of folders) dirAt(dir);
  for (const f of files) {
    const name = basename(f.path);
    dirAt(parent(f.path)).files.push({ name: f.is_markdown ? name.replace(/\.md$/i, "") : name, path: f.path });
  }
  const sort = (n: TreeDir) => {
    n.dirs.sort((a, b) => a.name.localeCompare(b.name));
    n.files.sort((a, b) => a.name.localeCompare(b.name));
    n.dirs.forEach(sort);
  };
  sort(root);
  return root;
}

/** Where `src` lands when dropped on folder `dir`, or null when the drop would change nothing or nest a folder in itself. */
export function dropTarget(src: string, dir: string): string | null {
  if (parent(src) === dir || under(dir, src)) return null;
  return join(dir, basename(src));
}

// The characters Obsidian refuses in a file name.
const FORBIDDEN = /[*"\\/<>:|?]/;

/** The path an inline rename moves to, or null when nothing changes. The explorer hides a note's `.md`, so it is implied. */
export function renameTarget(path: string, name: string, isNote: boolean): string | null {
  const clean = name.trim();
  if (!clean) return null;
  if (FORBIDDEN.test(clean)) throw new Error('A name cannot contain * " \\ / < > : | ?');
  const to = join(parent(path), isNote ? `${clean}.md` : clean);
  return to === path ? null : to;
}

export function freeName(taken: string[], dir: string, base: string, ext = ""): string {
  let n = join(dir, `${base}${ext}`);
  for (let i = 1; taken.includes(n); i++) n = join(dir, `${base} ${i}${ext}`);
  return n;
}
```

- [ ] **Step 4: `files.ts` and `layout.ts`**

`ui/src/lib/files.ts` becomes:

```ts
import type { FileEntry } from "./api";

export type FileKind = "note" | "image" | "pdf" | "base" | "graph" | "other";

// The image formats Obsidian previews.
const IMAGE = /\.(png|jpe?g|gif|bmp|svg|webp|avif)$/i;

// Graph tabs have pseudo-paths; Obsidian forbids `:` in file names, so none collides.
export function fileKind(path: string): FileKind {
  if (path.startsWith("graph:")) return "graph";
  if (/\.md$/i.test(path)) return "note";
  if (/\.base$/i.test(path)) return "base";
  if (IMAGE.test(path)) return "image";
  if (/\.pdf$/i.test(path)) return "pdf";
  return "other";
}

/** A link target as a vault file: exact path, then the shortest path with that name; case-insensitive. */
export function resolveFile(files: FileEntry[], target: string): string | null {
  const t = target.replace(/\\/g, "/").replace(/^\.?\//, "").toLowerCase();
  const exact = files.find((f) => f.path.toLowerCase() === t);
  if (exact) return exact.path;
  const name = t.slice(t.lastIndexOf("/") + 1);
  const hits = files
    .filter((f) => f.path.slice(f.path.lastIndexOf("/") + 1).toLowerCase() === name)
    .sort((a, b) => a.path.length - b.path.length || a.path.localeCompare(b.path));
  return hits[0]?.path ?? null;
}

/** Obsidian's `![[pic.png|300]]` and `|300x200`; any other alias is not a size. */
export function imageSize(alias?: string): { width?: number; height?: number } {
  const m = /^(\d+)(?:x(\d+))?$/.exec(alias?.trim() ?? "");
  if (!m) return {};
  return m[2] ? { width: Number(m[1]), height: Number(m[2]) } : { width: Number(m[1]) };
}

export function tabTitle(path: string): string {
  if (path === "graph:global") return "Graph view";
  if (path === "graph:local") return "Local graph";
  return path.slice(path.lastIndexOf("/") + 1).replace(/\.(md|base)$/i, "");
}
```

In `ui/src/lib/layout.ts` add, and use it in `withoutPath` (both `t.path === path` comparisons become `under(t.path, path)`) and `renamePath`:

```ts
/** `path` is `dir` itself or lies below it. */
export function under(path: string, dir: string): boolean {
  return path === dir || path.startsWith(`${dir}/`);
}

export function renamePath(root: Node, from: string, to: string): Node {
  return mapPanes(root, (p) => ({
    ...p,
    tabs: p.tabs.map((t) => (under(t.path, from) ? { ...t, path: to + t.path.slice(from.length) } : t)),
  }));
}
```

- [ ] **Step 5: API wrappers**

Append to the interfaces in `ui/src/lib/api.ts`:

```ts
export interface GraphNode { id: string; title: string; kind: "note" | "attachment" | "unresolved"; tags: string[] }
export interface GraphEdge { source: string; target: string }
export interface Graph { nodes: GraphNode[]; edges: GraphEdge[] }
export interface SortKey { property: string; direction: "ASC" | "DESC" }
export interface Column { id: string; label: string; editable: boolean }
export interface BaseRow { path: string; cells: unknown[] }
export interface BaseTable { views: string[]; view: number; columns: Column[]; rows: BaseRow[]; sort: SortKey[]; errors: string[] }
```

and to the wrappers:

```ts
export const listFolders = () => invoke<string[]>("list_folders");
export const graph = () => invoke<Graph>("graph");
export const getGraphConfig = () => invoke<Record<string, unknown>>("get_graph_config");
export const setGraphConfig = (config: Record<string, unknown>) => invoke<void>("set_graph_config", { config });
export const runBase = (path: string, view: number) => invoke<BaseTable>("run_base", { path, view });
export const setBaseSort = (path: string, view: number, sort: SortKey[]) => invoke<void>("set_base_sort", { path, view, sort });
```

- [ ] **Step 6: State and commands**

In `ui/src/lib/state.svelte.ts`:

```ts
  folders = $state<string[]>([]);
  // The note a local graph centres on: the last one active in any pane.
  lastNote = $state<string | null>(null);
```

`refresh` loads folders too:

```ts
  async refresh() {
    this.files = await api.listFiles();
    this.folders = await api.listFolders();
    this.titles = await api.titles();
  }
```

`renamed` and `forget` cover folders:

```ts
  renamed(from: string, to: string) {
    for (const path of Object.keys(this.docs)) {
      if (!L.under(path, from)) continue;
      const d = this.docs[path];
      delete this.docs[path];
      d.path = to + path.slice(from.length);
      this.docs[d.path] = d;
    }
    this.layout = L.renamePath(this.layout, from, to);
    this.persist();
  }

  forget(path: string) {
    this.layout = L.withoutPath(this.layout, path);
    for (const p of Object.keys(this.docs)) if (L.under(p, path)) delete this.docs[p];
    this.afterLayoutChange();
  }
```

In `ui/src/App.svelte`'s script, keep `lastNote` current:

```ts
  import { fileKind } from "./lib/files";
  $effect(() => {
    const p = app.activeTab?.path;
    if (p && fileKind(p) === "note") app.lastNote = p;
  });
```

In `ui/src/lib/commands.ts`:

```ts
import { freeName } from "./tree";

// What Obsidian writes into a new base.
export const NEW_BASE = "views:\n  - type: table\n    name: Table\n";

export async function newNote(dir = "") {
  const n = freeName(app.files.map((f) => f.path), dir, "Untitled", ".md");
  await createNote(n);
  await app.refresh();
  await app.openNote(n);
}

export async function newBase(dir = "") {
  const n = freeName(app.files.map((f) => f.path), dir, "Untitled", ".base");
  await createNote(n, NEW_BASE);
  await app.refresh();
  await app.openNote(n);
}
```

and a palette entry `{ id: "new-base", name: "Create new base", hotkey: "", run: () => newBase() }` after `new-note`.

In `ui/src/components/Tabs.svelte` replace the local `title` with `import { tabTitle } from "../lib/files";` and use `tabTitle(t.path)`.

- [ ] **Step 7: Screenshot mock**

In `ui/scripts/shot.mjs`, add cases to the `invoke` switch:

```js
        case "list_folders": return FX.folders ?? [...new Set(FX.files.flatMap((f) => f.path.split("/").slice(0, -1).map((_, i, a) => a.slice(0, i + 1).join("/"))))];
        case "graph": return FX.graph ?? { nodes: [], edges: [] };
        case "get_graph_config": return FX.graphConfig ?? {};
        case "run_base": { const t = FX.bases?.[args.path]; if (!t) throw { code: "base", message: "no fixture for " + args.path }; return { ...t, view: args.view }; }
        case "plan_rename": return { from: args.from, to: args.to, affected: FX.affected ?? [] };
        case "search": return FX.search ?? [];
```

- [ ] **Step 8: Run the checks**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass.

- [ ] **Step 9: Commit**

```bash
git add ui/src ui/scripts
git commit -m "feat(ui): helpers for folders, file resolution and new commands"
```

### Task 8: Explorer: inline rename, folder actions, drag to move

Obsidian's explorer renames in place, offers rename and delete on folders, creates a folder named *Untitled* ready for renaming, and moves files and folders by dragging them onto a folder. `window.prompt` goes away.

**Files:**
- Modify: `ui/src/components/Explorer.svelte` (whole file)
- Modify: `ui/src/app.css` (tree drop and rename styles)

**Interfaces:**
- Consumes: `buildTree`, `dropTarget`, `renameTarget`, `freeName`, `parent` from `tree.ts`; `newNote`, `newBase` from `commands.ts`; `app.folders`, `app.renamed`, `app.forget`, `app.save`; `planRename`, `applyRename`, `createFolder`, `deleteFile`.

- [ ] **Step 1: Explorer**

`ui/src/components/Explorer.svelte`:

```svelte
<script lang="ts">
  import { ask, confirm } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/state.svelte";
  import { newBase, newNote } from "../lib/commands";
  import { applyRename, createFolder, deleteFile, errorMessage, planRename } from "../lib/api";
  import { buildTree, dropTarget, freeName, parent, renameTarget, type TreeDir } from "../lib/tree";

  interface Menu { x: number; y: number; path: string; isDir: boolean }

  let collapsed = $state<Record<string, boolean>>({});
  let menu = $state<Menu | null>(null);
  let editing = $state<string | null>(null);
  let dragging = $state<string | null>(null);
  let dropDir = $state<string | null>(null);

  const tree = $derived(buildTree(app.files, app.folders));

  function openMenu(e: MouseEvent, path: string, isDir: boolean) {
    e.preventDefault();
    e.stopPropagation();
    menu = { x: e.clientX, y: e.clientY, path, isDir };
  }

  async function guarded(f: () => Promise<void>) {
    try {
      await f();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  // Rename and drag end here; the dialog lists the files whose links change.
  async function move(from: string, to: string) {
    const plan = await planRename(from, to);
    if (plan.affected.length > 0) {
      const list = plan.affected.join("\n");
      if (!(await ask(`Update links in ${plan.affected.length} file(s)?\n\n${list}`, { title: "Move", kind: "info" }))) return;
    }
    // A pending autosave would otherwise write the old text over the rewrite.
    for (const d of Object.values(app.docs)) await app.save(d);
    await applyRename(plan);
    app.renamed(from, plan.to);
    await app.refresh();
  }

  function commitRename(path: string, isDir: boolean, name: string) {
    editing = null;
    return guarded(async () => {
      const to = renameTarget(path, name, !isDir && /\.md$/i.test(path));
      if (to) await move(path, to);
    });
  }

  function remove(path: string, isDir: boolean) {
    return guarded(async () => {
      const what = isDir ? `the folder "${path}" and everything in it` : `"${path}"`;
      if (!(await confirm(`Move ${what} to the trash?`, { title: "Delete", kind: "warning" }))) return;
      await deleteFile(path);
      app.forget(path);
      await app.refresh();
    });
  }

  // As in Obsidian, a new folder is called Untitled and opens for renaming.
  function newFolderIn(dir: string) {
    return guarded(async () => {
      const path = freeName(app.folders, dir, "Untitled");
      await createFolder(path);
      await app.refresh();
      collapsed[dir] = false;
      editing = path;
    });
  }

  function start(e: DragEvent, path: string) {
    // WebKitGTK starts a drag only when it carries data.
    e.dataTransfer?.setData("text/plain", path);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
    dragging = path;
  }

  function over(e: DragEvent, dir: string) {
    if (!dragging || dropTarget(dragging, dir) === null) return;
    e.preventDefault();
    e.stopPropagation();
    dropDir = dir;
  }

  function drop(e: DragEvent, dir: string) {
    e.preventDefault();
    e.stopPropagation();
    const src = dragging;
    dragging = dropDir = null;
    const to = src ? dropTarget(src, dir) : null;
    if (src && to) void guarded(() => move(src, to));
  }

  function end() {
    dragging = dropDir = null;
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }
</script>

<svelte:window onclick={() => (menu = null)} />

{#snippet renameBox(path: string, isDir: boolean, name: string, indent: number)}
  <input
    class="rename"
    style="margin-left:{indent}px;width:calc(100% - {indent + 8}px)"
    value={name}
    use:focusSelect
    onkeydown={(e) => {
      e.stopPropagation();
      if (e.key === "Enter") e.currentTarget.blur();
      if (e.key === "Escape") editing = null;
    }}
    onblur={(e) => {
      if (editing === path) void commitRename(path, isDir, e.currentTarget.value);
    }}
  />
{/snippet}

{#snippet dir(node: TreeDir, depth: number)}
  {#each node.dirs as d (d.path)}
    {#if editing === d.path}
      {@render renameBox(d.path, true, d.name, 8 + depth * 12)}
    {:else}
      <button
        class="folder"
        class:drop={dropDir === d.path}
        style="padding-left:{8 + depth * 12}px"
        draggable="true"
        ondragstart={(e) => start(e, d.path)}
        ondragend={end}
        ondragover={(e) => over(e, d.path)}
        ondrop={(e) => drop(e, d.path)}
        onclick={() => (collapsed[d.path] = !collapsed[d.path])}
        oncontextmenu={(e) => openMenu(e, d.path, true)}
      >
        {collapsed[d.path] ? "▸" : "▾"} {d.name}
      </button>
    {/if}
    {#if !collapsed[d.path]}{@render dir(d, depth + 1)}{/if}
  {/each}
  {#each node.files as f (f.path)}
    {#if editing === f.path}
      {@render renameBox(f.path, false, f.name, 20 + depth * 12)}
    {:else}
      <button
        style="padding-left:{20 + depth * 12}px"
        class:active={app.activeTab?.path === f.path}
        draggable="true"
        ondragstart={(e) => start(e, f.path)}
        ondragend={end}
        ondragover={(e) => over(e, parent(f.path))}
        ondrop={(e) => drop(e, parent(f.path))}
        onclick={() => guarded(() => app.openNote(f.path))}
        oncontextmenu={(e) => openMenu(e, f.path, false)}
      >
        {f.name}
      </button>
    {/if}
  {/each}
{/snippet}

<div class="pane-title explorer-head">
  <span>Files</span>
  <span>
    <button title="New note" onclick={() => guarded(() => newNote())}>＋</button>
    <button title="New folder" aria-label="New folder" onclick={() => newFolderIn("")}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3"><path d="M1.5 4.5v8h13v-7H8L6.5 4H1.5z" /><path d="M8 7.5v3M6.5 9h3" /></svg>
    </button>
  </span>
</div>
<div
  class="tree"
  class:drop={dropDir === ""}
  role="tree"
  tabindex="-1"
  oncontextmenu={(e) => openMenu(e, "", true)}
  ondragover={(e) => over(e, "")}
  ondrop={(e) => drop(e, "")}
>
  {@render dir(tree, 0)}
</div>

{#if menu}
  {@const m = menu}
  <div class="menu" style="left:{m.x}px;top:{m.y}px" role="menu" tabindex="-1">
    {#if m.isDir}
      <button onclick={() => guarded(() => newNote(m.path))}>New note</button>
      <button onclick={() => newFolderIn(m.path)}>New folder</button>
      <button onclick={() => guarded(() => newBase(m.path))}>New base</button>
    {/if}
    {#if m.path}
      <button onclick={() => (editing = m.path)}>Rename…</button>
      <button onclick={() => remove(m.path, m.isDir)}>Delete</button>
    {/if}
  </div>
{/if}
```

- [ ] **Step 2: Styles**

Append to `ui/src/app.css`:

```css
.explorer-head { display: flex; justify-content: space-between; align-items: center; }
.explorer-head button { color: var(--fg-muted); padding: 0 4px; vertical-align: middle; }
.explorer-head button:hover { color: var(--fg); }
.tree { min-height: calc(100% - 72px); padding-bottom: 24px; }
.tree.drop, .tree button.drop { background: var(--accent-bg); }
.tree input.rename { display: block; margin: 1px 0; padding: 1px 6px; font-size: 14px; }
```

- [ ] **Step 3: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, tests pass.

- [ ] **Step 4: Verify in the window**

Build and run the real app on a scratch vault holding `A.md` (`[[B]]`), `folder/B.md`, `folder/pic.png`, an empty folder `empty/`, and `Ref.md` (`[[folder/B]]`):

```bash
cd ui && pnpm tauri build --debug --no-bundle && ../target/debug/engram-notes "$VAULT" &
spectacle -b -n -f -o "$SCRATCH/explorer.png"
```

If `spectacle` hangs or saves nothing, use headless Chromium with a fixture holding the same files (`folders` included) and these `EVAL` expressions, one screenshot each:

1. Right-click a folder: `document.querySelectorAll('.tree button.folder')[0].dispatchEvent(new MouseEvent('contextmenu', {bubbles: true, clientX: 80, clientY: 90}))`. The menu shows New note, New folder, New base, Rename…, Delete.
2. Then click Rename…: the row becomes an input with the name selected.
3. Type and blur: set the input's value to `renamed`, dispatch `blur`; `window.__calls` holds `plan_rename` with `to: "renamed"` and `apply_rename`.
4. Drag `A` onto the folder: dispatch `dragstart` on the file row with `new DragEvent('dragstart', {bubbles: true, dataTransfer: new DataTransfer()})`, `dragover` and `drop` on the folder row; `window.__calls` holds `plan_rename` from `A.md` to `folder/A.md`.
5. The empty folder shows in the tree.

Ask the user to try drag and drop in the real window: headless Chromium cannot show whether WebKitGTK delivers drop events.

- [ ] **Step 5: Commit**

```bash
git add ui/src
git commit -m "feat(ui): explorer renames in place, moves by drag, handles folders"
```

### Task 9: Image embeds inside notes

`![[pic.png]]` renders as a link today. Obsidian shows the image, in live preview when the cursor is off the line and always in reading view, sized by `|300` or `|300x200`. Markdown images `![alt](path)` load vault files the same way, and a trailing `|300` in the alt text sizes them.

**Files:**
- Modify: `ui/src/lib/files.ts`, `ui/src/lib/files.test.ts` (`altAndSize`, `imageUrl`)
- Modify: `ui/src/lib/render.ts`, `ui/src/lib/render.test.ts`
- Modify: `ui/src/editor/livePreview.ts`, `ui/src/editor/livePreview.test.ts`
- Modify: `ui/src/components/Editor.svelte`, `Reading.svelte`, `NoteView.svelte`

**Interfaces:**
- Consumes: `resolveFile`, `imageSize`, `fileKind` (Task 7).
- Produces: `type ImageResolver = (target: string) => string | null` exported from `files.ts`; `altAndSize(text): { alt: string; width?: number; height?: number }`; `imageUrl(url: string, image?: ImageResolver): string | null`; `renderMarkdown(text: string, opts?: { image?: ImageResolver }): string`; `buildDecorations(state, ranges, focused = true, image?: ImageResolver)`; `livePreview({ onFollow, image })`.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/files.test.ts`:

```ts
describe("image urls", () => {
  it("reads a size off markdown alt text", () => {
    expect(altAndSize("cap|120")).toEqual({ alt: "cap", width: 120 });
    expect(altAndSize("a|b")).toEqual({ alt: "a|b" });
  });
  it("passes remote urls and resolves vault paths", () => {
    const image = (t: string) => (t === "my x.png" ? "asset://ok" : null);
    expect(imageUrl("https://e.com/a.png", image)).toBe("https://e.com/a.png");
    expect(imageUrl("my%20x.png", image)).toBe("asset://ok");
    expect(imageUrl("gone.png", image)).toBeNull();
    expect(imageUrl("bad%zz.png", () => "asset://raw")).toBe("asset://raw");
  });
});
```

In `ui/src/lib/render.test.ts`, replace the first test and add one:

```ts
  it("renders wikilinks as anchors with targets", () => {
    const h = renderMarkdown("see [[Note#Sec|shown]] and ![[Other]]");
    expect(h).toContain('<a class="wikilink" data-target="Note#Sec">shown</a>');
    expect(h).toContain('<a class="wikilink" data-target="Other">Other</a>');
  });
  it("embeds images through the resolver with Obsidian's sizes", () => {
    const image = (t: string) => (t.startsWith("gone") ? null : `asset://${t}`);
    const h = renderMarkdown(
      "![[pic.png|300]] ![[wide.png|300x200]] ![cap|120](img/my%20x.png) ![[gone.png]] ![r](https://e.com/a.png)",
      { image },
    );
    expect(h).toContain('<img class="embed" src="asset://pic.png" alt="pic.png" width="300">');
    expect(h).toContain('alt="wide.png" width="300" height="200"');
    expect(h).toContain('<img class="embed" src="asset://img/my x.png" alt="cap" width="120">');
    expect(h).toContain('<span class="embed-missing">gone.png</span>');
    expect(h).toContain('src="https://e.com/a.png"');
  });
```

In `ui/src/editor/livePreview.test.ts`, give `decos` a resolver parameter and add a test:

```ts
function decos(doc: string, cursor = doc.length, focused = true, image?: (t: string) => string | null) {
  const out: { text: string; kind: string }[] = [];
  buildDecorations(stateOf(doc, cursor), [{ from: 0, to: doc.length }], focused, image).between(0, doc.length, (from, to, d) => {
    const spec = d.spec;
    const kind = spec.widget ? spec.widget.constructor.name : (spec.class ?? "hide");
    out.push({ text: doc.slice(from, to), kind });
  });
  return out;
}
```

```ts
  it("draws image embeds and markdown images off the cursor line", () => {
    const image = (t: string) => `asset://${t}`;
    const doc = "![[pic.png|300]]\n\n![cap](img/a.png)\n\n![[Note]]\n\nz";
    const d = decos(doc, doc.length, true, image);
    expect(d.filter((x) => x.kind === "ImageWidget").map((x) => x.text)).toEqual(["![[pic.png|300]]", "![cap](img/a.png)"]);
    expect(d).toContainEqual({ text: "![[Note]]", kind: "WikiWidget" });
    expect(decos(doc, 0, true, image)[0]).toEqual({ text: "![[pic.png|300]]", kind: "cm-wikilink-src" });
  });
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test`
Expected: FAIL: `altAndSize` not exported; the embed renders as an `img` without `src`; no `ImageWidget`.

- [ ] **Step 3: Helpers in `files.ts`**

```ts
export type ImageResolver = (target: string) => string | null;

/** `![caption|300](url)`: Obsidian reads a trailing size off the alt text. */
export function altAndSize(text: string): { alt: string; width?: number; height?: number } {
  const i = text.lastIndexOf("|");
  const size = i >= 0 ? imageSize(text.slice(i + 1)) : {};
  return size.width ? { alt: text.slice(0, i), ...size } : { alt: text };
}

/** A markdown image's URL for the webview: remote URLs pass, vault paths go through `image`. */
export function imageUrl(url: string, image?: ImageResolver): string | null {
  if (/^[a-z][a-z0-9+.-]*:/i.test(url)) return url;
  let path = url;
  try {
    path = decodeURIComponent(url);
  } catch {
    // a stray % is part of the name
  }
  return image?.(path) ?? null;
}
```

- [ ] **Step 4: Reading view**

In `ui/src/lib/render.ts`:

```ts
import { altAndSize, fileKind, imageSize, imageUrl, type ImageResolver } from "./files";

// A type alias, so it fits markdown-it's indexable `env`.
export type RenderOptions = { image?: ImageResolver };

function imageHtml(alt: string, src: string | null, size: { width?: number; height?: number }): string {
  if (!src) return `<span class="embed-missing">${escapeHtml(alt)}</span>`;
  const w = size.width ? ` width="${size.width}"` : "";
  const h = size.height ? ` height="${size.height}"` : "";
  return `<img class="embed" src="${escapeAttr(src)}" alt="${escapeAttr(alt)}"${w}${h}>`;
}

function wikilinkHtml(raw: string, opts: RenderOptions): string {
  const l = findWikilinks(raw)[0];
  if (!l) return escapeHtml(raw);
  if (l.embed && fileKind(l.target) === "image") {
    return imageHtml(l.target, opts.image?.(l.target) ?? null, imageSize(l.alias));
  }
  return `<a class="wikilink" data-target="${escapeAttr(linkTarget(l))}">${escapeHtml(displayText(l))}</a>`;
}
```

The inline rule passes the environment: `tok.content = wikilinkHtml(src.slice(state.pos, end + 2), state.env as RenderOptions);`.

After the rules, markdown images:

```ts
// Vault paths in markdown images load through the same resolver as embeds.
md.renderer.rules.image = (tokens, idx, options, env) => {
  const tok = tokens[idx];
  const url = String(tok.attrGet("src") ?? "");
  const text = md.renderer.renderInlineAsText(tok.children ?? [], options, env);
  const { alt, ...size } = altAndSize(text);
  return imageHtml(alt || url, imageUrl(url, (env as RenderOptions | undefined)?.image), size);
};

export function renderMarkdown(text: string, opts: RenderOptions = {}): string {
  return md.render(blankFrontmatter(text), opts);
}
```

- [ ] **Step 5: Live preview**

In `ui/src/editor/livePreview.ts` import `altAndSize, fileKind, imageSize, imageUrl, type ImageResolver` from `../lib/files` and add:

```ts
class ImageWidget extends WidgetType {
  constructor(readonly src: string | null, readonly alt: string, readonly width?: number, readonly height?: number) { super(); }
  eq(o: ImageWidget) { return o.src === this.src && o.alt === this.alt && o.width === this.width && o.height === this.height; }
  toDOM() {
    if (!this.src) {
      const s = document.createElement("span");
      s.className = "cm-embed-missing";
      s.textContent = this.alt;
      return s;
    }
    const img = document.createElement("img");
    img.className = "cm-embed-image";
    img.src = this.src;
    img.alt = this.alt;
    if (this.width) img.width = this.width;
    if (this.height) img.height = this.height;
    return img;
  }
}
```

`buildDecorations` gains a fourth parameter `image?: ImageResolver`. Inside `enter`, right after the `Link`/`Image` without URL check:

```ts
        if (name === "Image" && !active.has(line)) {
          const url = node.node.getChild("URL")!;
          const target = state.sliceDoc(url.from, url.to);
          const text = /^!\[([^\]]*)\]/.exec(state.sliceDoc(node.from, node.to))?.[1] ?? "";
          const { alt, width, height } = altAndSize(text);
          push(node.from, node.to, Decoration.replace({ widget: new ImageWidget(imageUrl(target, image), alt || target, width, height) }));
          return false;
        }
```

In the wikilink loop, the replacement off the cursor line picks an image for image embeds:

```ts
      const size = imageSize(l.alias);
      const widget = l.embed && fileKind(l.target) === "image"
        ? new ImageWidget(image?.(l.target) ?? null, l.target, size.width, size.height)
        : new WikiWidget(displayText(l), target);
      push(a, b, active.has(line) ? Decoration.mark({ class: "cm-wikilink-src" }) : Decoration.replace({ widget }));
```

`livePreview(opts: { onFollow: (target: string) => void; image?: ImageResolver })` passes `opts.image` in both `buildDecorations` calls. Add to its `baseTheme`:

```ts
    ".cm-embed-image": { maxWidth: "100%", verticalAlign: "top", borderRadius: "4px" },
    ".cm-embed-missing": { color: "var(--fg-muted)", border: "1px dashed var(--border)", borderRadius: "4px", padding: "0 6px" },
```

- [ ] **Step 6: Wire the resolver**

`NoteView.svelte`:

```ts
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { resolveFile } from "../lib/files";

  // The asset protocol is scoped to the open vault.
  const image = (target: string) => {
    const p = resolveFile(app.files, target);
    return p && app.root ? convertFileSrc(`${app.root}/${p}`) : null;
  };
```

Pass `{image}` to `<Editor>` and `<Reading>`. `Editor.svelte` gains the prop `image: (target: string) => string | null` and `forMode` becomes `(m === "live" ? livePreview({ onFollow, image }) : [])`. `Reading.svelte` gains the same prop and `const html = $derived(renderMarkdown(text, { image }));`, plus styles:

```css
  .reading :global(img.embed) { max-width: 100%; border-radius: 4px; }
  .reading :global(.embed-missing) { color: var(--fg-muted); border: 1px dashed var(--border); border-radius: 4px; padding: 0 6px; }
```

- [ ] **Step 7: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: pass.

- [ ] **Step 8: Verify in the window**

A note `Pics.md` holding `![[pic.png|200]]`, a paragraph, `![caption](img/pic.png)` and `![[missing.png]]`, with `pic.png` and `img/pic.png` in the vault. Screenshot it in live mode with the cursor on the last line, and in reading mode: both images show at their sizes and the missing one is a dashed name. With the real window, use real PNGs; in headless Chromium the mock's `convertFileSrc` draws a labelled rectangle.

- [ ] **Step 9: Commit**

```bash
git add ui/src
git commit -m "feat(ui): show image embeds and markdown images in notes"
```

### Task 10: Graph settings, search and filtering

Everything the graph view decides without a canvas: settings over Obsidian's `graph.json` defaults, the search operators, which nodes and edges survive the filters, the local neighbourhood, and sizes.

**Files:**
- Create: `ui/src/lib/graph.ts`, `ui/src/lib/graph.test.ts`

**Interfaces:**
- Consumes: `Graph`, `GraphNode`, `GraphEdge` from `api.ts`.
- Produces: `GraphSettings` (Obsidian's keys plus `localDepth`), `DEFAULTS`, `readSettings(raw: Record<string, unknown>): GraphSettings`, `Search { words; tags; paths; files }`, `parseSearch(q: string): Search`, `searchWords(q: string): string` (the plain words, for full-text search), `ViewNode { id; title; kind: GraphNode["kind"] | "tag"; tags; inbound }`, `ViewGraph { nodes: ViewNode[]; edges: GraphEdge[] }`, `filterGraph(g: Graph, s: GraphSettings, content: Set<string> | null, center?: string | null): ViewGraph`, `neighbourhood(edges, center, depth): Set<string>`, `radius(n: ViewNode, s: GraphSettings): number`, `forces(s: GraphSettings): { center: number; charge: number; link: number; distance: number }`, `labelAlpha(scale: number, fade: number): number`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/graph.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Graph } from "./api";
import { DEFAULTS, filterGraph, labelAlpha, parseSearch, readSettings, searchWords, type GraphSettings, type ViewGraph } from "./graph";

const g: Graph = {
  nodes: [
    { id: "A.md", title: "A", kind: "note", tags: ["proj"] },
    { id: "B.md", title: "B", kind: "note", tags: ["proj/sub"] },
    { id: "C.md", title: "C", kind: "note", tags: [] },
    { id: "Lonely.md", title: "Lonely", kind: "note", tags: [] },
    { id: "Ghost", title: "Ghost", kind: "unresolved", tags: [] },
    { id: "img/p.png", title: "p.png", kind: "attachment", tags: [] },
  ],
  edges: [
    { source: "A.md", target: "B.md" },
    { source: "B.md", target: "C.md" },
    { source: "A.md", target: "Ghost" },
    { source: "C.md", target: "img/p.png" },
  ],
};
const ids = (v: ViewGraph) => v.nodes.map((n) => n.id);
const s = (o: Partial<GraphSettings> = {}): GraphSettings => ({ ...DEFAULTS, ...o });

describe("graph filters", () => {
  it("hides attachments by default; unresolved and orphans on request", () => {
    expect(ids(filterGraph(g, s(), null))).toEqual(["A.md", "B.md", "C.md", "Lonely.md", "Ghost"]);
    expect(ids(filterGraph(g, s({ showAttachments: true, hideUnresolved: true, showOrphans: false }), null))).toEqual(["A.md", "B.md", "C.md", "img/p.png"]);
  });

  it("drops edges to hidden nodes and counts inbound links", () => {
    const v = filterGraph(g, s(), null);
    expect(v.edges).toHaveLength(3);
    expect(v.nodes.find((n) => n.id === "B.md")!.inbound).toBe(1);
    expect(v.nodes.find((n) => n.id === "A.md")!.inbound).toBe(0);
  });

  it("parses Obsidian's search operators", () => {
    expect(parseSearch('tag:#proj path:"my dir" file:x hello')).toEqual({ words: ["hello"], tags: ["proj"], paths: ["my dir"], files: ["x"] });
    expect(searchWords('tag:#proj hello "big world"')).toBe('hello "big world"');
  });

  it("filters by tag including nested tags, by words, and by full-text hits", () => {
    expect(ids(filterGraph(g, s({ search: "tag:proj" }), null))).toEqual(["A.md", "B.md"]);
    expect(ids(filterGraph(g, s({ search: "ghost" }), null))).toEqual(["Ghost"]);
    expect(ids(filterGraph(g, s({ search: "needle" }), new Set(["C.md"])))).toEqual(["C.md"]);
    expect(ids(filterGraph(g, s({ search: "path:img file:p.png", showAttachments: true }), null))).toEqual(["img/p.png"]);
  });

  it("adds tag nodes linked to their notes", () => {
    const v = filterGraph(g, s({ showTags: true }), null);
    expect(ids(v)).toContain("#proj");
    expect(ids(v)).toContain("#proj/sub");
    expect(v.edges).toContainEqual({ source: "A.md", target: "#proj" });
  });

  it("keeps a local graph's neighbourhood to the chosen depth", () => {
    expect(ids(filterGraph(g, s({ localDepth: 1 }), null, "A.md"))).toEqual(["A.md", "B.md", "Ghost"]);
    expect(ids(filterGraph(g, s({ localDepth: 2 }), null, "A.md"))).toEqual(["A.md", "B.md", "C.md", "Ghost"]);
  });

  it("reads graph.json over the defaults, ignoring wrong types", () => {
    const r = readSettings({ showOrphans: false, repelStrength: "x", other: 1 });
    expect(r.showOrphans).toBe(false);
    expect(r.repelStrength).toBe(10);
  });

  it("fades labels in as the view zooms", () => {
    expect(labelAlpha(0.5, 0)).toBe(0);
    expect(labelAlpha(2, 0)).toBe(1);
    expect(labelAlpha(1, 3)).toBe(1);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test graph`
Expected: FAIL, `./graph` not found.

- [ ] **Step 3: `graph.ts`**

```ts
import type { Graph, GraphEdge, GraphNode } from "./api";

/** Obsidian's graph.json keys, plus the local graph's depth. */
export interface GraphSettings {
  search: string;
  showTags: boolean;
  showAttachments: boolean;
  hideUnresolved: boolean;
  showOrphans: boolean;
  showArrow: boolean;
  textFadeMultiplier: number;
  nodeSizeMultiplier: number;
  lineSizeMultiplier: number;
  centerStrength: number;
  repelStrength: number;
  linkStrength: number;
  linkDistance: number;
  localDepth: number;
}

// Obsidian's defaults.
export const DEFAULTS: GraphSettings = {
  search: "",
  showTags: false,
  showAttachments: false,
  hideUnresolved: false,
  showOrphans: true,
  showArrow: false,
  textFadeMultiplier: 0,
  nodeSizeMultiplier: 1,
  lineSizeMultiplier: 1,
  centerStrength: 0.518713248970312,
  repelStrength: 10,
  linkStrength: 1,
  linkDistance: 250,
  localDepth: 1,
};

export function readSettings(raw: Record<string, unknown>): GraphSettings {
  const out: Record<string, unknown> = { ...DEFAULTS };
  for (const [k, v] of Object.entries(DEFAULTS)) if (typeof raw[k] === typeof v) out[k] = raw[k];
  return out as unknown as GraphSettings;
}

export interface Search { words: string[]; tags: string[]; paths: string[]; files: string[] }

const TERM = /(\w+:)?(?:"([^"]*)"|(\S+))/g;

/** The subset of Obsidian's search operators the graph filter reads: `tag:`, `path:`, `file:` and plain words. */
export function parseSearch(q: string): Search {
  const s: Search = { words: [], tags: [], paths: [], files: [] };
  for (const m of q.matchAll(TERM)) {
    const value = (m[2] ?? m[3] ?? "").toLowerCase();
    if (!value) continue;
    if (m[1] === "tag:") s.tags.push(value.replace(/^#/, ""));
    else if (m[1] === "path:") s.paths.push(value);
    else if (m[1] === "file:") s.files.push(value);
    else s.words.push(m[1] ? m[0].toLowerCase() : value);
  }
  return s;
}

/** The plain words of a search, as the full-text search takes them. */
export function searchWords(q: string): string {
  return [...q.matchAll(TERM)].filter((m) => !["tag:", "path:", "file:"].includes(m[1] ?? "")).map((m) => m[0]).join(" ");
}

export interface ViewNode { id: string; title: string; kind: GraphNode["kind"] | "tag"; tags: string[]; inbound: number }
export interface ViewGraph { nodes: ViewNode[]; edges: GraphEdge[] }

function matches(n: GraphNode, s: Search, content: Set<string> | null): boolean {
  const path = n.id.toLowerCase();
  const name = path.slice(path.lastIndexOf("/") + 1);
  const title = n.title.toLowerCase();
  if (s.tags.some((t) => !n.tags.some((x) => x === t || x.startsWith(`${t}/`)))) return false;
  if (s.paths.some((p) => !path.includes(p))) return false;
  if (s.files.some((f) => !name.includes(f))) return false;
  if (s.words.length === 0 || content?.has(n.id)) return true;
  return s.words.every((w) => title.includes(w) || path.includes(w));
}

/** Nodes within `depth` links of `center`, following links either way. */
export function neighbourhood(edges: GraphEdge[], center: string, depth: number): Set<string> {
  const adj = new Map<string, string[]>();
  const add = (a: string, b: string) => adj.set(a, [...(adj.get(a) ?? []), b]);
  for (const e of edges) {
    add(e.source, e.target);
    add(e.target, e.source);
  }
  const seen = new Set([center]);
  let frontier = [center];
  for (let d = 0; d < depth; d++) {
    const next: string[] = [];
    for (const id of frontier) {
      for (const m of adj.get(id) ?? []) {
        if (!seen.has(m)) {
          seen.add(m);
          next.push(m);
        }
      }
    }
    frontier = next;
  }
  return seen;
}

/** What the graph draws. `content` holds the notes full-text search found for the plain words. */
export function filterGraph(g: Graph, s: GraphSettings, content: Set<string> | null, center?: string | null): ViewGraph {
  const search = parseSearch(s.search);
  const searching = Object.values(search).some((terms) => terms.length > 0);
  let nodes: ViewNode[] = g.nodes
    .filter((n) => (n.kind !== "attachment" || s.showAttachments) && (n.kind !== "unresolved" || !s.hideUnresolved))
    .filter((n) => !searching || matches(n, search, content))
    .map((n) => ({ ...n, inbound: 0 }));
  const kept = new Set(nodes.map((n) => n.id));
  let edges = g.edges.filter((e) => kept.has(e.source) && kept.has(e.target));
  if (s.showTags) {
    const tags = new Set<string>();
    for (const n of nodes) {
      for (const t of n.tags) {
        tags.add(t);
        edges.push({ source: n.id, target: `#${t}` });
      }
    }
    for (const t of [...tags].sort()) nodes.push({ id: `#${t}`, title: `#${t}`, kind: "tag", tags: [], inbound: 0 });
  }
  if (center) {
    const near = neighbourhood(edges, center, s.localDepth);
    nodes = nodes.filter((n) => near.has(n.id));
    edges = edges.filter((e) => near.has(e.source) && near.has(e.target));
  }
  if (!s.showOrphans) {
    const linked = new Set(edges.flatMap((e) => [e.source, e.target]));
    nodes = nodes.filter((n) => linked.has(n.id) || n.id === center);
  }
  const inbound = new Map<string, number>();
  for (const e of edges) inbound.set(e.target, (inbound.get(e.target) ?? 0) + 1);
  for (const n of nodes) n.inbound = inbound.get(n.id) ?? 0;
  return { nodes, edges };
}

/** Obsidian sizes a node by its links. */
export function radius(n: ViewNode, s: GraphSettings): number {
  return (4 + Math.sqrt(n.inbound) * 2) * s.nodeSizeMultiplier;
}

/** Obsidian's slider values in d3-force units, scaled so the defaults space a vault comfortably. */
export function forces(s: GraphSettings) {
  return { center: s.centerStrength * 0.1, charge: -s.repelStrength * 20, link: s.linkStrength, distance: s.linkDistance / 5 };
}

/** Labels fade in as the view zooms; the text fade slider moves the threshold. */
export function labelAlpha(scale: number, fade: number): number {
  return Math.min(1, Math.max(0, (scale - 0.8 + fade * 0.2) * 3));
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ui && pnpm check && pnpm test`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/graph.ts ui/src/lib/graph.test.ts
git commit -m "feat(ui): graph settings, search operators and filters"
```

### Task 11: The graph view

A canvas renderer over a `d3-force` simulation, opened as a tab by *Open graph view* (`Ctrl+G`) or *Open local graph*. Nodes are drawn in batches per colour so ten thousand stay smooth. Settings live in a collapsible panel with Obsidian's sections and persist to `graph.json`, keeping keys this app does not read.

**Files:**
- Create: `ui/src/components/GraphView.svelte`
- Modify: `ui/src/components/Pane.svelte` (graph kind)
- Modify: `ui/src/lib/commands.ts` (two commands)
- Modify: `ui/src/app.css` (graph colours and layout)
- Modify: `ui/package.json` (`d3-force`, `@types/d3-force`)

**Interfaces:**
- Consumes: `filterGraph`, `readSettings`, `searchWords`, `forces`, `radius`, `labelAlpha`, `DEFAULTS`, `GraphSettings`, `ViewGraph`, `ViewNode` (Task 10); `api.graph`, `api.getGraphConfig`, `api.setGraphConfig`, `api.search`, `api.createNote`; `app.lastNote`, `app.openNote`, `app.refresh`.
- Produces: `GraphView` with prop `local: boolean`; tab paths `graph:global` and `graph:local`; commands `graph` (`Ctrl+G`) and `local-graph`.

- [ ] **Step 1: Dependencies**

Run: `cd ui && pnpm add d3-force && pnpm add -D @types/d3-force`
Then restart Vite with `pnpm dev --force` if it is running.

- [ ] **Step 2: Commands and pane**

In `ui/src/lib/commands.ts` after `new-base`:

```ts
  { id: "graph", name: "Open graph view", hotkey: "Ctrl+G", run: () => app.openNote("graph:global") },
  { id: "local-graph", name: "Open local graph", hotkey: "", run: () => app.openNote("graph:local") },
```

In `ui/src/components/Pane.svelte` import `GraphView`, add `const kind = $derived(tab ? fileKind(tab.path) : "other");` to the script, and pick the view by kind:

```svelte
    {#key tab.path}
      {#if kind === "note"}
        <NoteView paneId={pane.id} path={tab.path} />
      {:else if kind === "graph"}
        <GraphView local={tab.path === "graph:local"} />
      {:else}
        <FileView path={tab.path} />
      {/if}
    {/key}
```

- [ ] **Step 3: `GraphView.svelte`**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { forceLink, forceManyBody, forceSimulation, forceX, forceY, type Simulation, type SimulationNodeDatum } from "d3-force";
  import { app } from "../lib/state.svelte";
  import { createNote, errorMessage, getGraphConfig, graph as loadGraph, search, setGraphConfig, type Graph } from "../lib/api";
  import { DEFAULTS, filterGraph, forces, labelAlpha, radius, readSettings, searchWords, type GraphSettings, type ViewGraph, type ViewNode } from "../lib/graph";

  let { local }: { local: boolean } = $props();

  interface Node extends SimulationNodeDatum { data: ViewNode; r: number }
  interface Link { source: Node; target: Node }
  type NumKey = { [K in keyof GraphSettings]: GraphSettings[K] extends number ? K : never }[keyof GraphSettings];

  let canvas = $state<HTMLCanvasElement>();
  let data = $state.raw<Graph>({ nodes: [], edges: [] });
  let settings = $state<GraphSettings>({ ...DEFAULTS });
  let content = $state.raw<Set<string> | null>(null);
  let panel = $state(false);
  let loaded = $state(false);
  // graph.json as read, so keys this app does not know are written back unchanged.
  let raw: Record<string, unknown> = {};

  // Simulation state lives outside Svelte's reactivity; d3 mutates it every tick.
  let nodes: Node[] = [];
  let links: Link[] = [];
  let byId = new Map<string, Node>();
  let sim: Simulation<Node, undefined> | undefined;
  let view = { x: 0, y: 0, k: 1 };
  let size = { w: 0, h: 0 };
  let hover: Node | null = null;
  let near = new Set<Node>();
  let press: { sx: number; sy: number; vx: number; vy: number; node: Node | null; moved: boolean } | null = null;
  let frame = 0;

  const say = (e: unknown) => app.say(errorMessage(e));
  const center = $derived(local ? app.lastNote : null);
  const shown = $derived<ViewGraph>(local && !center ? { nodes: [], edges: [] } : filterGraph(data, settings, content, center));

  onMount(() => {
    const c = canvas!;
    getGraphConfig()
      .then((r) => {
        raw = r;
        settings = readSettings(r);
        loaded = true;
      })
      .catch(say);
    const observer = new ResizeObserver(resize);
    observer.observe(c);
    // Registered by hand: the listener must not be passive to stop the page scrolling.
    c.addEventListener("wheel", wheel, { passive: false });
    return () => {
      observer.disconnect();
      c.removeEventListener("wheel", wheel);
      sim?.stop();
      cancelAnimationFrame(frame);
    };
  });

  // The index changed.
  $effect(() => {
    void app.files;
    loadGraph().then((g) => (data = g)).catch(say);
  });

  // Plain words also match note text, through full-text search.
  $effect(() => {
    const words = searchWords(settings.search).trim();
    if (!words) {
      content = null;
      return;
    }
    const t = setTimeout(() => search(words, 100000).then((hits) => (content = new Set(hits.map((h) => h.path)))).catch(say), 200);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const snap = $state.snapshot(settings);
    if (!loaded) return;
    const t = setTimeout(() => setGraphConfig({ ...raw, ...snap }).catch(say), 500);
    return () => clearTimeout(t);
  });

  $effect(() => rebuild(shown, settings));

  $effect(() => {
    void app.config?.theme;
    draw();
  });

  // Nodes keep their positions across filter changes, so the layout does not jump.
  function rebuild(g: ViewGraph, s: GraphSettings) {
    const old = byId;
    byId = new Map();
    nodes = g.nodes.map((d) => {
      const n: Node = old.get(d.id) ?? { data: d, r: 0 };
      n.data = d;
      n.r = radius(d, s);
      byId.set(d.id, n);
      return n;
    });
    links = g.edges.map((e) => ({ source: byId.get(e.source)!, target: byId.get(e.target)! }));
    const degree = new Map<Node, number>();
    for (const l of links) for (const n of [l.source, l.target]) degree.set(n, (degree.get(n) ?? 0) + 1);
    if (old.size === 0) view.k = Math.min(1.5, Math.max(0.15, Math.sqrt(30 / Math.max(1, nodes.length))));
    const f = forces(s);
    sim?.stop();
    sim = forceSimulation(nodes)
      .force("link", forceLink<Node, Link>(links).distance(f.distance).strength((l) => f.link / Math.min(degree.get(l.source)!, degree.get(l.target)!)))
      .force("charge", forceManyBody<Node>().strength(f.charge).distanceMax(2000))
      .force("x", forceX<Node>(0).strength(f.center))
      .force("y", forceY<Node>(0).strength(f.center))
      .alpha(old.size ? 0.3 : 1)
      .on("tick", draw);
    hover = null;
    near = new Set();
    draw();
  }

  function resize() {
    const c = canvas!;
    const box = c.getBoundingClientRect();
    if (!size.w) view = { ...view, x: box.width / 2, y: box.height / 2 };
    size = { w: box.width, h: box.height };
    const dpr = devicePixelRatio || 1;
    c.width = Math.round(box.width * dpr);
    c.height = Math.round(box.height * dpr);
    draw();
  }

  function draw() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(paint);
  }

  function paint() {
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    const css = getComputedStyle(c);
    const color = (name: string) => css.getPropertyValue(name).trim();
    const dpr = devicePixelRatio || 1;
    const { x: ox, y: oy, k } = view;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy);
    const dim = hover ? 0.25 : 1;

    ctx.lineWidth = Math.max(settings.lineSizeMultiplier, 0.5 / k);
    for (const lit of [false, true]) {
      ctx.beginPath();
      for (const l of links) {
        if ((l.source === hover || l.target === hover) !== lit) continue;
        ctx.moveTo(l.source.x!, l.source.y!);
        ctx.lineTo(l.target.x!, l.target.y!);
      }
      ctx.strokeStyle = color(lit ? "--accent" : "--graph-line");
      ctx.globalAlpha = lit ? 1 : dim;
      ctx.stroke();
    }

    if (settings.showArrow) {
      ctx.beginPath();
      for (const { source: a, target: b } of links) {
        const dx = b.x! - a.x!;
        const dy = b.y! - a.y!;
        const len = Math.hypot(dx, dy) || 1;
        const [ux, uy] = [dx / len, dy / len];
        const tip = [b.x! - ux * b.r, b.y! - uy * b.r];
        const s = 3 + ctx.lineWidth * 2;
        ctx.moveTo(tip[0], tip[1]);
        ctx.lineTo(tip[0] - ux * s - uy * s * 0.6, tip[1] - uy * s + ux * s * 0.6);
        ctx.lineTo(tip[0] - ux * s + uy * s * 0.6, tip[1] - uy * s - ux * s * 0.6);
        ctx.closePath();
      }
      ctx.fillStyle = color("--graph-line");
      ctx.globalAlpha = dim;
      ctx.fill();
    }

    // One path per colour and opacity keeps ten thousand nodes cheap.
    const groups = new Map<string, Node[]>();
    for (const n of nodes) {
      const accent = n === hover || near.has(n);
      const key = `${accent ? "accent" : n.data.kind}|${accent || !hover ? 1 : dim}`;
      const g = groups.get(key);
      if (g) g.push(n);
      else groups.set(key, [n]);
    }
    for (const [key, group] of groups) {
      const [kind, alpha] = key.split("|");
      ctx.globalAlpha = Number(alpha);
      ctx.beginPath();
      for (const n of group) {
        ctx.moveTo(n.x! + n.r, n.y!);
        ctx.arc(n.x!, n.y!, n.r, 0, 2 * Math.PI);
      }
      if (kind === "unresolved") {
        ctx.strokeStyle = color("--graph-note");
        ctx.lineWidth = 1.2;
        ctx.stroke();
      } else {
        ctx.fillStyle = color(kind === "accent" ? "--accent" : `--graph-${kind}`);
        ctx.fill();
      }
    }

    const fade = labelAlpha(k, settings.textFadeMultiplier);
    ctx.font = `${12 / k}px ${color("--font-ui")}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    ctx.fillStyle = color("--fg");
    for (const n of nodes) {
      const lit = n === hover || near.has(n);
      const a = lit ? 1 : hover ? fade * dim : fade;
      const sx = n.x! * k + ox;
      const sy = n.y! * k + oy;
      if (a < 0.02 || sx < -100 || sx > size.w + 100 || sy < -20 || sy > size.h + 20) continue;
      ctx.globalAlpha = a;
      ctx.fillText(n.data.title, n.x!, n.y! + n.r + 2 / k);
    }
    ctx.globalAlpha = 1;
  }

  function point(e: MouseEvent) {
    const b = canvas!.getBoundingClientRect();
    const sx = e.clientX - b.left;
    const sy = e.clientY - b.top;
    return { sx, sy, x: (sx - view.x) / view.k, y: (sy - view.y) / view.k };
  }

  function nodeAt(x: number, y: number): Node | null {
    let best: Node | null = null;
    let dist = Infinity;
    for (const n of nodes) {
      const d = Math.hypot(n.x! - x, n.y! - y);
      if (d < n.r + 4 / view.k && d < dist) {
        best = n;
        dist = d;
      }
    }
    return best;
  }

  function setHover(n: Node | null) {
    if (n === hover) return;
    hover = n;
    near = new Set();
    for (const l of n ? links : []) {
      if (l.source === n) near.add(l.target);
      if (l.target === n) near.add(l.source);
    }
    canvas!.style.cursor = n ? "pointer" : "grab";
    draw();
  }

  function down(e: PointerEvent) {
    const p = point(e);
    canvas!.setPointerCapture(e.pointerId);
    const node = nodeAt(p.x, p.y);
    press = { sx: p.sx, sy: p.sy, vx: view.x, vy: view.y, node, moved: false };
    if (node) {
      node.fx = node.x;
      node.fy = node.y;
      sim?.alphaTarget(0.3).restart();
    }
  }

  function move(e: PointerEvent) {
    const p = point(e);
    if (!press) return setHover(nodeAt(p.x, p.y));
    if (Math.hypot(p.sx - press.sx, p.sy - press.sy) > 3) press.moved = true;
    if (press.node) {
      press.node.fx = p.x;
      press.node.fy = p.y;
    } else {
      view = { ...view, x: press.vx + p.sx - press.sx, y: press.vy + p.sy - press.sy };
      draw();
    }
  }

  function up() {
    const p = press;
    press = null;
    if (!p?.node) return;
    p.node.fx = p.node.fy = null;
    sim?.alphaTarget(0);
    if (!p.moved) void openNode(p.node.data);
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    const p = point(e);
    const k = Math.min(8, Math.max(0.05, view.k * Math.exp(-e.deltaY * 0.0015)));
    view = { x: p.sx - p.x * k, y: p.sy - p.y * k, k };
    draw();
  }

  // As in Obsidian: a note opens, an unresolved link creates its note, a tag searches for itself.
  async function openNode(n: ViewNode) {
    try {
      if (n.kind === "tag") {
        settings.search = `tag:${n.id}`;
      } else if (n.kind === "unresolved") {
        const path = /\.md$/i.test(n.id) ? n.id : `${n.id}.md`;
        await createNote(path);
        await app.refresh();
        await app.openNote(path);
      } else {
        await app.openNote(n.id);
      }
    } catch (e) {
      say(e);
    }
  }
</script>

{#snippet slider(label: string, key: NumKey, min: number, max: number, step: number)}
  <label class="slider">{label}<input type="range" {min} {max} {step} bind:value={settings[key]} /></label>
{/snippet}

<div class="graph">
  <canvas
    bind:this={canvas}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointerleave={() => {
      if (!press) setHover(null);
    }}
  ></canvas>
  {#if local && !center}<div class="graph-empty">Open a note to see its local graph</div>{/if}
  <div class="graph-controls" class:open={panel}>
    <button class="graph-toggle" title="Graph settings" onclick={() => (panel = !panel)}>
      {#if panel}×{:else}<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="3" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" /></svg>{/if}
    </button>
    {#if panel}
      <h4>Filters</h4>
      <input placeholder="Search files…" bind:value={settings.search} />
      {#if local}<label class="slider">Depth {settings.localDepth}<input type="range" min="1" max="3" step="1" bind:value={settings.localDepth} /></label>{/if}
      <label><input type="checkbox" bind:checked={settings.showTags} /> Tags</label>
      <label><input type="checkbox" bind:checked={settings.showAttachments} /> Attachments</label>
      <label><input type="checkbox" bind:checked={settings.hideUnresolved} /> Existing files only</label>
      <label><input type="checkbox" bind:checked={settings.showOrphans} /> Orphans</label>
      <h4>Display</h4>
      <label><input type="checkbox" bind:checked={settings.showArrow} /> Arrows</label>
      {@render slider("Text fade threshold", "textFadeMultiplier", -3, 3, 0.1)}
      {@render slider("Node size", "nodeSizeMultiplier", 0.1, 5, 0.1)}
      {@render slider("Link thickness", "lineSizeMultiplier", 0.1, 5, 0.1)}
      <h4>Forces</h4>
      {@render slider("Center force", "centerStrength", 0, 1, 0.01)}
      {@render slider("Repel force", "repelStrength", 0, 20, 0.1)}
      {@render slider("Link force", "linkStrength", 0, 1, 0.01)}
      {@render slider("Link distance", "linkDistance", 30, 500, 1)}
    {/if}
  </div>
</div>
```

`localDepth` is a number, so the depth slider uses the same binding as the others.

- [ ] **Step 4: Styles**

In `ui/src/app.css`, add to `:root`: `--graph-note: #8a919c; --graph-line: #d3d7dd; --graph-tag: #4f9d69; --graph-attachment: #c49a2c;` and to both dark blocks: `--graph-note: #a2a9b3; --graph-line: #3d424a; --graph-tag: #6fbf88; --graph-attachment: #d8b04a;`. Append:

```css
.graph { position: relative; flex: 1; min-height: 0; overflow: hidden; background: var(--bg); }
.graph canvas { position: absolute; inset: 0; width: 100%; height: 100%; cursor: grab; touch-action: none; }
.graph-empty { position: absolute; inset: 0; display: grid; place-items: center; color: var(--fg-muted); pointer-events: none; }
.graph-controls { position: absolute; top: 8px; right: 8px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); font-size: 13px; max-height: calc(100% - 16px); overflow: auto; }
.graph-controls.open { width: 240px; padding: 0 12px 12px; box-shadow: 0 6px 20px rgba(0,0,0,.12); }
.graph-toggle { float: right; color: var(--fg-muted); padding: 4px 6px; }
.graph-controls.open .graph-toggle { margin-right: -8px; }
.graph-controls h4 { margin: 12px 0 6px; font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--fg-muted); font-weight: 500; }
.graph-controls label { display: flex; align-items: center; gap: 8px; margin: 5px 0; }
.graph-controls label.slider { flex-direction: column; align-items: stretch; gap: 0; }
.graph-controls input:not([type]) { width: 100%; }
.graph-controls input[type="range"] { accent-color: var(--accent); border: none; padding: 0; }
```

- [ ] **Step 5: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: pass.

- [ ] **Step 6: Verify in the window**

Real window first: open a vault with a few dozen linked notes, `Ctrl+G`, screenshot; hover a node, open the panel, switch themes. Otherwise headless Chromium with a fixture whose `graph` has about thirty nodes (notes, one unresolved, one attachment, tags) and a workspace with one pane holding `graph:global`; screenshot after `SETTLE=4000`, then with `EVAL` clicking `.graph-toggle` and checking *Tags* and *Attachments*. Then a second fixture with `graph:local` beside a note, where `lastNote` comes from the note being the active pane first. Adjust the factors in `forces()` until the default layout neither collapses into a ball nor flies apart, and note the values in the commit.

- [ ] **Step 7: Commit**

```bash
git add ui/package.json ui/pnpm-lock.yaml ui/src
git commit -m "feat(ui): graph view with filters, forces and local graph"
```

### Task 12: The base view

Opening a `.base` file shows its first view as a table. A picker switches views, a header click sorts and writes `sort` back, note property cells edit the note, unsupported parts show in a bar above the table, and *Source* edits the YAML.

**Files:**
- Create: `ui/src/lib/bases.ts`, `ui/src/lib/bases.test.ts`
- Create: `ui/src/components/PropertyValue.svelte`
- Create: `ui/src/components/YamlEditor.svelte`
- Create: `ui/src/components/BaseView.svelte`
- Modify: `ui/src/components/Properties.svelte` (uses `PropertyValue`)
- Modify: `ui/src/components/Pane.svelte` (base kind)
- Modify: `ui/src/app.css`

**Interfaces:**
- Consumes: `runBase`, `setBaseSort`, `setProperty`, `readNote`, `writeNote`, `BaseTable`, `Column`, `SortKey` (Task 7); `propKind`, `addItem`, `removeItem` from `properties.ts`.
- Produces: `nextSort(sort: SortKey[], property: string): SortKey[]`, `sortMark(sort: SortKey[], id: string): string`, `cellText(v: unknown): string`; `PropertyValue` with props `value: unknown`, `onCommit: (v: unknown) => void`; `YamlEditor` with props `text: string`, `onchange: (t: string) => void`; `BaseView` with prop `path: string`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/bases.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { cellText, nextSort, sortMark } from "./bases";

describe("base table helpers", () => {
  it("sorts by a column, flipping it when it already leads", () => {
    expect(nextSort([], "note.a")).toEqual([{ property: "note.a", direction: "ASC" }]);
    expect(nextSort([{ property: "a", direction: "ASC" }], "note.a")).toEqual([{ property: "a", direction: "DESC" }]);
    expect(nextSort([{ property: "note.a", direction: "DESC" }], "note.a")).toEqual([{ property: "note.a", direction: "ASC" }]);
    expect(nextSort([{ property: "note.a", direction: "ASC" }], "file.name")).toEqual([{ property: "file.name", direction: "ASC" }]);
  });

  it("marks the column that leads the sort", () => {
    expect(sortMark([{ property: "rating", direction: "DESC" }], "note.rating")).toBe(" ↓");
    expect(sortMark([{ property: "note.rating", direction: "ASC" }], "note.rating")).toBe(" ↑");
    expect(sortMark([], "note.rating")).toBe("");
  });

  it("shows read-only cells as text", () => {
    expect(cellText(null)).toBe("");
    expect(cellText(["a", 1])).toBe("a, 1");
    expect(cellText(true)).toBe("true");
    expect(cellText({ a: 1 })).toBe('{"a":1}');
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test bases`
Expected: FAIL, `./bases` not found.

- [ ] **Step 3: `bases.ts`**

```ts
import type { SortKey } from "./api";

// A bare property id in a base file means a note property.
const norm = (p: string) => (p.includes(".") ? p : `note.${p}`);

/** A header click sorts by that column, or flips it when it already leads, as one sort key. */
export function nextSort(sort: SortKey[], property: string): SortKey[] {
  const first = sort[0];
  if (first && norm(first.property) === norm(property)) {
    return [{ property: first.property, direction: first.direction === "ASC" ? "DESC" : "ASC" }];
  }
  return [{ property, direction: "ASC" }];
}

export function sortMark(sort: SortKey[], id: string): string {
  const first = sort[0];
  if (!first || norm(first.property) !== norm(id)) return "";
  return first.direction === "DESC" ? " ↓" : " ↑";
}

export function cellText(v: unknown): string {
  if (v == null) return "";
  if (Array.isArray(v)) return v.map(cellText).join(", ");
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
```

- [ ] **Step 4: `PropertyValue.svelte`, shared with the properties pane**

```svelte
<script lang="ts">
  import { addItem, propKind, removeItem } from "../lib/properties";

  let { value, onCommit }: { value: unknown; onCommit: (v: unknown) => void } = $props();
  const kind = $derived(propKind(value));
  const list = $derived(Array.isArray(value) ? value : []);
  const text = (v: unknown) => (v == null ? "" : typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if kind === "checkbox"}
  <input type="checkbox" checked={value === true} onchange={(e) => onCommit(e.currentTarget.checked)} />
{:else if kind === "number"}
  <input type="number" value={value as number} onchange={(e) => onCommit(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
{:else if kind === "date"}
  <input type="date" value={value as string} onchange={(e) => onCommit(e.currentTarget.value || null)} />
{:else if kind === "datetime"}
  <input type="datetime-local" value={(value as string).slice(0, 16)} onchange={(e) => onCommit(e.currentTarget.value || null)} />
{:else if kind === "list"}
  <div class="chips">
    {#each list as item, i (i)}
      <span class="chip">{text(item)}<button title="Remove" onclick={() => onCommit(removeItem(list, i))}>×</button></span>
    {/each}
    <input
      class="chip-input"
      placeholder="Add"
      onkeydown={(e) => {
        if (e.key !== "Enter") return;
        onCommit(addItem(list, e.currentTarget.value));
        e.currentTarget.value = "";
      }}
    />
  </div>
{:else}
  <input value={text(value)} onchange={(e) => onCommit(e.currentTarget.value)} />
{/if}
```

In `Properties.svelte`, the whole `{#if kind === "checkbox"} … {/if}` chain inside the `each` becomes `<PropertyValue value={v} onCommit={(x) => commit(k, x)} />`; drop the now unused `propKind`, `addItem`, `removeItem`, `list` and `text` there.

- [ ] **Step 5: `YamlEditor.svelte`**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection, keymap, lineNumbers } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
  import { yaml } from "@codemirror/lang-yaml";
  import { editorTheme } from "../editor/theme";

  let { text, onchange }: { text: string; onchange: (t: string) => void } = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          lineNumbers(),
          yaml(),
          editorTheme,
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString());
          }),
        ],
      }),
    });
    view.focus();
    return () => view.destroy();
  });
</script>

<div class="yaml" bind:this={host}></div>
```

- [ ] **Step 6: `BaseView.svelte`**

```svelte
<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { errorMessage, readNote, runBase, setBaseSort, setProperty, writeNote, type BaseTable, type Column } from "../lib/api";
  import { cellText, nextSort, sortMark } from "../lib/bases";
  import PropertyValue from "./PropertyValue.svelte";
  import YamlEditor from "./YamlEditor.svelte";

  let { path }: { path: string } = $props();
  let view = $state(0);
  let mode = $state<"table" | "source">("table");
  let table = $state.raw<BaseTable | null>(null);
  let failure = $state<string | null>(null);
  let source = $state("");
  let pending = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Bumped after this view writes, so the table reads the file again.
  let version = $state(0);

  const LINKS = ["file.name", "file.basename", "file.path"];
  const say = (e: unknown) => app.say(errorMessage(e));

  $effect(() => {
    void app.files; // the index changed
    void version;
    runBase(path, view)
      .then((t) => {
        table = t;
        failure = null;
      })
      .catch((e) => {
        table = null;
        failure = errorMessage(e);
      });
  });

  async function sortBy(c: Column) {
    if (!table) return;
    try {
      await setBaseSort(path, view, nextSort(table.sort, c.id));
      version++;
    } catch (e) {
      say(e);
    }
  }

  async function edit(note: string, c: Column, value: unknown) {
    try {
      const open = app.docs[note];
      if (open) await app.save(open); // a dirty buffer would conflict with the rewrite
      await setProperty(note, c.id.slice("note.".length), value);
      version++;
    } catch (e) {
      say(e);
    }
  }

  async function flush() {
    clearTimeout(timer);
    if (!pending) return;
    pending = false;
    await writeNote(path, source);
  }

  async function showSource() {
    try {
      source = (await readNote(path)).text;
      mode = "source";
    } catch (e) {
      say(e);
    }
  }

  async function showTable() {
    try {
      await flush();
      version++;
      mode = "table";
    } catch (e) {
      say(e);
    }
  }

  function onSource(text: string) {
    source = text;
    pending = true;
    clearTimeout(timer);
    timer = setTimeout(() => flush().catch(say), 500);
  }

  onDestroy(() => void flush().catch(say));
</script>

<div class="base">
  <div class="modes">
    <button class:active={mode === "table"} onclick={showTable}>table</button>
    <button class:active={mode === "source"} onclick={showSource}>source</button>
    {#if table && mode === "table"}
      {#if table.views.length > 1}
        <select bind:value={view}>
          {#each table.views as name, i (i)}<option value={i}>{name}</option>{/each}
        </select>
      {:else}
        <span class="base-view">{table.views[0]}</span>
      {/if}
      <span class="base-count">{table.rows.length} {table.rows.length === 1 ? "result" : "results"}</span>
    {/if}
  </div>
  {#if mode === "source"}
    <div class="base-source"><YamlEditor text={source} onchange={onSource} /></div>
  {:else}
    {#if failure || table?.errors.length}
      <div class="base-errors">
        {#if failure}<div>{failure}</div>{/if}
        {#each table?.errors ?? [] as e (e)}<div>{e}</div>{/each}
      </div>
    {/if}
    {#if table}
      <div class="base-scroll">
        <table>
          <thead>
            <tr>
              {#each table.columns as c (c.id)}
                <th><button onclick={() => sortBy(c)}>{c.label}{sortMark(table.sort, c.id)}</button></th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each table.rows as r (r.path)}
              <tr>
                {#each table.columns as c, i (c.id)}
                  <td>
                    {#if LINKS.includes(c.id)}
                      <button class="base-link" onclick={() => app.openNote(r.path).catch(say)}>{cellText(r.cells[i])}</button>
                    {:else if c.editable}
                      <PropertyValue value={r.cells[i]} onCommit={(v) => edit(r.path, c, v)} />
                    {:else}
                      {cellText(r.cells[i])}
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</div>
```

In `Pane.svelte` add `{:else if kind === "base"}<BaseView path={tab.path} />` before the `FileView` fallback.

- [ ] **Step 7: Styles**

Append to `ui/src/app.css`:

```css
.base { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.base .modes { align-items: center; }
.base select { font: inherit; color: var(--fg); background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); padding: 0 4px; margin-left: 8px; }
.base-view { margin-left: 8px; color: var(--fg); }
.base-count { margin-left: 8px; color: var(--fg-muted); }
.base-errors { background: var(--mark); padding: 6px 12px; font-size: 13px; }
.base-scroll { flex: 1; overflow: auto; }
.base table { border-collapse: collapse; min-width: 100%; font-size: 13px; }
.base th { position: sticky; top: 0; z-index: 1; background: var(--bg-2); text-align: left; font-weight: 500; border-bottom: 1px solid var(--border); white-space: nowrap; }
.base th button { width: 100%; text-align: left; padding: 6px 10px; color: var(--fg-muted); }
.base th button:hover { color: var(--fg); }
.base td { border-bottom: 1px solid var(--border); padding: 3px 10px; vertical-align: middle; min-width: 120px; max-width: 360px; }
.base td input:not([type="checkbox"]) { width: 100%; border-color: transparent; background: transparent; padding: 2px 4px; }
.base td input:not([type="checkbox"]):hover, .base td input:not([type="checkbox"]):focus { border-color: var(--border); }
.base-link { color: var(--accent); text-align: left; padding: 2px 0; }
.base-link:hover { text-decoration: underline; }
.base-source { flex: 1; min-height: 0; overflow: auto; }
.yaml { height: 100%; }
.yaml .cm-editor .cm-content { font-family: var(--font-mono); font-size: 14px; }
```

- [ ] **Step 8: Check and test**

Run: `cd ui && pnpm check && pnpm test`
Expected: pass.

- [ ] **Step 9: Verify in the window**

Real window: a vault with five book notes carrying `status`, `rating` and `tags`, and `Books.base` holding two views, one formula and a `file.ctime` filter in a third view. Screenshot the first view, the view picker, a sorted column, an edited checkbox, the source mode, and the third view's unsupported bar. Otherwise headless Chromium with `bases` fixtures for the same tables, and `EVAL` clicking a header: `window.__calls` holds `set_base_sort` with the flipped direction.

- [ ] **Step 10: Commit**

```bash
git add ui/src
git commit -m "feat(ui): base table view with sort, cell edits and source"
```

### Task 13: Documentation

**Files:**
- Create: `docs/bases.md`
- Modify: `docs/smoke.md`

- [ ] **Step 1: `docs/bases.md`**

```markdown
# Bases

A `.base` file is Obsidian's YAML. engram-notes reads the same files and
implements the table view over notes. What it does not support is listed in
the view as unsupported; a filter it cannot read shows no rows rather than a
guess.

## The file

    filters:            # an expression, or and / or / not holding a list
      and:
        - file.hasTag("book")
        - 'status != "done"'
    formulas:
      pages_left: "pages - read"
    properties:
      note.status:
        displayName: Status
    views:
      - type: table
        name: Reading
        filters:        # combined with the global filters by AND
          not:
            - file.inFolder("Archive")
        order: [file.name, note.status, formula.pages_left]
        sort:
          - property: note.status
            direction: ASC
        limit: 50

`not` holds when none of its filters hold. A view without `order` shows
`file.name`. Clicking a column header sorts by it and writes `sort` back to
the file. Note property cells edit the note's frontmatter; file fields and
formulas are read-only. Rows are notes; other files are not listed.

## Expressions

| Supported | Notes |
| --- | --- |
| `"text"`, `'text'`, `12`, `2.5`, `true`, `false`, `null` | |
| `+ - * / %`, `( )` | `+` joins text when either side is text |
| `== != > < >= <=` | a date compares with a string naming a date |
| `! && \|\|` | |
| `note.status`, `status`, `note["due date"]` | a missing property is `null` |
| `formula.name` | |
| `file.name`, `file.basename`, `file.path`, `file.folder`, `file.ext` | the vault root folder is `/` |
| `file.size`, `file.mtime` | |
| `file.tags` | with `#`, body and frontmatter tags |
| `file.links` | resolved note paths |
| `file.hasTag("a", "b")` | any of them, nested tags included |
| `file.inFolder("x")` | sub-folders included |
| `file.hasLink("Note")` | by path or name, as a link resolves |
| `file.hasProperty("x")` | |
| `date("2026-09-13")`, `date("2026-09-13 10:30")`, `now()`, `today()` | |
| `date + "1 week"`, `now() - "2d"` | units `y M w d h m s` and their words; `date - date` is milliseconds |
| `if(condition, then, else?)` | |
| `.contains(x)` | substring of text, element of a list |
| `.isEmpty()` | `null`, empty text or an empty list |

Not supported yet, and reported when used: every other function and method
(`link()`, `list()`, `.lower()`, `.format()`, `.map()` …), fields such as
`.length`, indexing with `[ ]` other than `note["…"]`, `this`, `file.ctime`,
`file.backlinks`, `file.embeds`, `file.properties`, views other than
`table`, `groupBy` and `summaries`.
```

- [ ] **Step 2: Smoke lines**

Append to `docs/smoke.md` before the final `cargo test` line, renumbering it last:

```markdown
24. Right-click a folder: rename it in place; links with its path are rewritten and open tabs follow.
25. Delete a folder: it and its notes go to the trash and their tabs close.
26. Drag a note and a folder onto another folder: both move and links follow; dropping on empty space moves to the root.
27. New folder appears as Untitled, ready to type a name; an empty folder stays listed.
28. `![[pic.png|200]]` and `![caption](img/pic.png)` show images in live preview and reading view.
29. Ctrl+G opens the graph: hover highlights neighbours, click opens a note, wheel zooms, drag pans and moves nodes.
30. Graph filters (search with `tag:` and `path:`, tags, attachments, existing files only, orphans) and forces change the view and survive a restart in `.engram-notes/graph.json`.
31. Open local graph follows the note last active and its depth slider reaches three links.
32. Create new base, add a filter in its source, switch back to the table: rows match, a header click sorts and writes `sort`, a cell edit rewrites the note's frontmatter.
33. A base using `file.ctime` lists it as unsupported and shows no rows.
```

- [ ] **Step 3: Commit**

```bash
git add docs/bases.md docs/smoke.md
git commit -m "docs: bases subset and smoke lines for graph, bases, explorer"
```

