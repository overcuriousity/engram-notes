# Foundation Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the seven gaps the foundation handoff lists, so the graph-and-bases plan starts from a base that matches the spec's foundation sections.

**Architecture:** Core changes stay in `core` with tests: block references and anchors in the parser and index, corrupt-index recovery, an open-time benchmark. The Tauri shell reports recovery and watcher state and gains serialisation smoke tests. The frontend moves from one tab strip to a layout tree of panes, opens attachments in a preview tab, jumps to headings and blocks, and edits properties by type. Pure logic the UI needs (layout operations, palette ranking, property kinds, decoration building) lives in plain TypeScript modules with vitest tests.

**Tech Stack:** as the foundation plan: Rust 2024, rusqlite 0.40, Tauri 2.11 (plus the `protocol-asset` feature), Svelte 5, CodeMirror 6, markdown-it 15, vitest 5.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`, sections *The vault*, *The index*, *The editor and UI*, *Error handling*, *Testing*. The gaps come from `docs/superpowers/plans/2026-09-12-handoff.md`.

## Global Constraints

- `AGENTS.md` applies: short comments that say why, `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean, `core` has no Tauri dependency, conventional commits under 72 characters.
- Files are the truth. A schema change bumps `SCHEMA_VERSION` and rebuilds; no migrations.
- Obsidian is the reference: `[[Note#^id]]` names a block whose line ends in ` ^id`; following a heading or block link scrolls to it; *Split right* and *Split down* open the current note in a new pane; closing a pane's last tab closes the pane unless it is the only one; list properties show as removable chips.
- The frontend never touches the filesystem. Attachments reach the webview through Tauri's asset protocol, scoped to the open vault at runtime.
- Tests run without a window, a model or the network. UI changes are verified in the running app using the handoff's last section before a task is called done.
- TypeScript stays on 5.x. After adding a CodeMirror or Tauri package, restart Vite with `pnpm dev --force`.

## Machine prerequisites

Building `src-tauri` on Fedora needs, once:

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel libappindicator-gtk3-devel dbus-devel glib2-devel
```

Tasks 1 to 3 need only Rust. Everything touching `src-tauri` needs the packages.

## File structure

```
core/src/parse.rs               + Link.block, ParsedNote.blocks (Block {id, line})
core/src/index/schema.sql       + links.block, + blocks table; SCHEMA_VERSION 2
core/src/index/rebuild.rs       writes blocks; skips resolve_all when nothing changed
core/src/index/query.rs         + anchor_line(path, fragment)
core/src/index/mod.rs           + open_or_recreate (corrupt file deleted, rebuilt)
core/src/rename.rs              keeps #^block when rewriting
core/src/watch.rs               callback receives Result<Vec<Change>, String>
core/tests/open_time.rs         ignored benchmark: 10 000 notes, second open < 2 s
src-tauri/src/commands.rs       VaultInfo.index_recreated, .watch_error; rescan, anchor_line,
                                attachment_path, open_external; watch-failed event
src-tauri/Cargo.toml            tauri feature protocol-asset
src-tauri/tauri.conf.json       app.security.assetProtocol.enable
ui/src/lib/layout.ts            layout tree: split, close, find, replace paths
ui/src/lib/layout.test.ts
ui/src/lib/textdiff.ts          smallest replacement, so a second editor on a note keeps its cursor
ui/src/lib/textdiff.test.ts
ui/src/lib/state.svelte.ts      docs by path, panes in a layout tree, jumps, focus rescan
ui/src/lib/palette.ts           note ranking and command filtering, from Palette.svelte
ui/src/lib/palette.test.ts
ui/src/lib/properties.ts        property kinds and value parsing
ui/src/lib/properties.test.ts
ui/src/lib/files.ts             file kind from the extension: note, image, pdf, other
ui/src/lib/files.test.ts
ui/src/lib/commands.test.ts     chords and hotkey overrides
ui/src/editor/livePreview.ts    buildDecorations(state, ranges) exported, block ids muted
ui/src/editor/livePreview.test.ts
ui/src/lib/render.ts            block ids hidden in reading view
ui/src/lib/wikilink.ts          block fragment in WikiLink and displayText
ui/src/components/Workspace.svelte  renders the layout tree recursively
ui/src/components/Pane.svelte       one pane: tab strip and its active view
ui/src/components/FileView.svelte   image, PDF, or open-in-default-app
ui/src/components/Tabs.svelte       tabs of one pane
ui/src/components/NoteView.svelte   doc by path, jump handling
ui/src/components/Editor.svelte     jump prop
ui/src/components/Properties.svelte typed editors, list chips
docs/smoke.md                   lines for splits, attachments, block links, properties
```

---

### Task 1: Block references and anchor lines in core

Gap 4. `[[Note#^id]]` is a block link, not a heading link. The parser records block ids (` ^id` at a line end), the index stores them, and `anchor_line` maps a heading or block fragment to a source line so the UI can scroll there.

**Files:**
- Modify: `core/src/parse.rs`
- Modify: `core/src/index/schema.sql`, `core/src/index/mod.rs` (`SCHEMA_VERSION`)
- Modify: `core/src/index/rebuild.rs` (`write_note`)
- Modify: `core/src/index/query.rs`
- Modify: `core/src/rename.rs`
- Test: in-file test modules of `parse.rs`, `index/resolve.rs`, `rename.rs`

**Interfaces:**
- Produces: `parse::Link.block: Option<String>`; `parse::Block { id: String, line: u32 }`; `ParsedNote.blocks: Vec<Block>`; `query::LinkRow.block: Option<String>`; `Index::anchor_line(&self, path: &str, fragment: &str) -> Result<Option<u32>>` where `fragment` is what follows `#` (`"^id"` or `"Heading"`), line is 1-based in the source file.

- [ ] **Step 1: Write the failing tests**

In `core/src/parse.rs` tests:

```rust
    #[test]
    fn block_links_and_block_ids() {
        let src = "[[Note#^abc]] [[Note#Sec]]\n\nA paragraph ^para-1\n\n- item ^li\n```\nx ^fenced\n```\nnot^inword";
        let n = parse(src);
        assert_eq!(n.links[0].block.as_deref(), Some("abc"));
        assert_eq!(n.links[0].heading, None);
        assert_eq!(n.links[1].heading.as_deref(), Some("Sec"));
        assert_eq!(n.links[1].block, None);
        let b: Vec<_> = n.blocks.iter().map(|b| (b.id.as_str(), b.line)).collect();
        assert_eq!(b, vec![("para-1", 3), ("li", 5)]);
    }
```

In `core/src/index/resolve.rs` tests:

```rust
    #[test]
    fn block_links_are_stored_and_anchors_found() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("T.md"),
            "---\na: 1\n---\n# Intro\ntext ^b1\n## Deep Part\n",
        )
        .unwrap();
        fs::write(d.path().join("S.md"), "[[T#^b1]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let out = ix.outgoing("S.md").unwrap();
        assert_eq!(out[0].block.as_deref(), Some("b1"));
        assert_eq!(out[0].heading, None);
        assert_eq!(ix.anchor_line("T.md", "^b1").unwrap(), Some(5));
        assert_eq!(ix.anchor_line("T.md", "deep part").unwrap(), Some(6));
        assert_eq!(ix.anchor_line("T.md", "Intro#Deep Part").unwrap(), Some(6));
        assert_eq!(ix.anchor_line("T.md", "^nope").unwrap(), None);
    }
```

In `core/src/rename.rs` tests:

```rust
    #[test]
    fn rewrite_keeps_block_fragment() {
        let out = rewrite_links("[[Old#^b1|see]]", "Old.md", "New.md", true);
        assert_eq!(out, "[[New#^b1|see]]");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes-core block`
Expected: compile errors, no field `block` on `Link`, no method `anchor_line`.

- [ ] **Step 3: Parser**

In `core/src/parse.rs`, add `pub block: Option<String>,` to `Link` after `heading`, add the block type and regex, and `pub blocks: Vec<Block>,` to `ParsedNote` after `headings`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Block {
    pub id: String,
    pub line: u32,
}
```

```rust
// Obsidian's block id: `^id` ending a line, after a space or alone on it.
static BLOCK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(?:^|[ \t])\^([A-Za-z0-9-]+)[ \t\r]*$").unwrap());
```

In the wikilink loop replace the `heading:` expression with a split of the fragment:

```rust
        let fragment = c
            .get(3)
            .map(|h| h.as_str().trim())
            .filter(|h| !h.is_empty());
        let (heading, block) = match fragment {
            Some(f) if f.starts_with('^') => (None, Some(f[1..].to_owned())),
            Some(f) => (Some(f.to_owned()), None),
            None => (None, None),
        };
        links.push(Link {
            target: c[2].trim().to_owned(),
            heading,
            block,
            alias: c
                .get(4)
                .map(|a| a.as_str().trim().to_owned())
                .filter(|a| !a.is_empty()),
            // kind, line, start, end unchanged
```

Markdown links in `walk_markdown` get `block: None`. Before building `ParsedNote`:

```rust
    let blocks = BLOCK_ID
        .captures_iter(body)
        .map(|c| c.get(1).unwrap())
        .map(|m| (body_offset + m.start(), m.as_str()))
        .filter(|(pos, _)| !in_ranges(*pos, &code_ranges))
        .map(|(pos, id)| Block {
            id: id.to_owned(),
            line: line_of(source, pos),
        })
        .collect();
```

and add `blocks,` to the struct literal.

- [ ] **Step 4: Schema and writer**

`core/src/index/mod.rs`: `pub const SCHEMA_VERSION: &str = "2";`

`core/src/index/schema.sql`: in `links`, add `block TEXT,` after `heading TEXT,`. Append:

```sql
CREATE TABLE IF NOT EXISTS blocks (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  id TEXT NOT NULL,
  line INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS blocks_path ON blocks(path);
```

`core/src/index/rebuild.rs`, in `write_note`, the link insert becomes:

```rust
    let mut ins = tx.prepare_cached(
        "INSERT INTO links(src_path, target_raw, target_path, kind, heading, block, alias, line, start, \"end\")
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
```

with `l.block,` after `l.heading,` in the params, and after the headings insert:

```rust
    let mut ins = tx.prepare_cached("INSERT INTO blocks(path, id, line) VALUES (?1, ?2, ?3)")?;
    for b in &note.blocks {
        ins.execute(params![entry.path, b.id, b.line])?;
    }
```

- [ ] **Step 5: Queries**

`core/src/index/query.rs`: add `pub block: Option<String>,` to `LinkRow` after `heading`; append `, l.block` to `LINK_COLUMNS`; in `link_row` add `block: r.get(9)?,`. Add to `impl Index`:

```rust
    /// The source line a `heading` or `^block` fragment points at in `path`.
    pub fn anchor_line(&self, path: &str, fragment: &str) -> Result<Option<u32>> {
        let (sql, key) = match fragment.strip_prefix('^') {
            Some(id) => (
                "SELECT line FROM blocks WHERE path=?1 AND lower(id)=lower(?2) LIMIT 1",
                id,
            ),
            // `[[Note#A#B]]` names heading B under A; the last part finds it.
            None => (
                "SELECT line FROM headings WHERE path=?1 AND lower(text)=lower(?2) ORDER BY line LIMIT 1",
                fragment.rsplit('#').next().unwrap_or(fragment).trim(),
            ),
        };
        Ok(self
            .conn
            .query_row(sql, rusqlite::params![path, key], |r| r.get(0))
            .optional()?)
    }
```

- [ ] **Step 6: Rename keeps the block**

`core/src/rename.rs`, in the `Wiki | Embed` arm, replace the `heading` binding and use `fragment` in the `format!`:

```rust
                let fragment = match (&l.heading, &l.block) {
                    (Some(h), _) => format!("#{h}"),
                    (None, Some(b)) => format!("#^{b}"),
                    (None, None) => String::new(),
                };
                out.push_str(&format!("{bang}[[{new_wiki}{fragment}{alias}]]"));
```

- [ ] **Step 7: Run the core tests**

Run: `cargo test -p engram-notes-core && cargo clippy -p engram-notes-core --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all pass, 40 tests.

- [ ] **Step 8: Commit**

```bash
git add core
git commit -m "feat(core): block references and anchor lines"
```

### Task 2: A corrupt index is deleted and rebuilt

Gap 3, core half. The spec: "a corrupt index is deleted and rebuilt with a notice". Only corruption triggers deletion; a locked or unreadable file stays an error, so a second instance never deletes the first one's index.

**Files:**
- Modify: `core/src/index/mod.rs`
- Test: `core/src/index/rebuild.rs` tests

**Interfaces:**
- Produces: `Index::open_or_recreate(path: &Path) -> Result<(Index, bool)>`; the flag is `true` when a corrupt file was replaced.

- [ ] **Step 1: Write the failing test**

In `core/src/index/rebuild.rs` tests:

```rust
    #[test]
    fn corrupt_file_is_recreated() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ix.db");
        fs::write(&p, vec![b'x'; 4096]).unwrap();
        assert!(Index::open(&p).is_err());
        let (ix, recreated) = Index::open_or_recreate(&p).unwrap();
        assert!(recreated);
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 0);
        drop(ix);
        let (_ix, recreated) = Index::open_or_recreate(&p).unwrap();
        assert!(!recreated);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p engram-notes-core corrupt_file`
Expected: compile error, no function `open_or_recreate`.

- [ ] **Step 3: Implement**

In `core/src/index/mod.rs`, replace `open`, `open_in_memory` and the signatures of `prepare` and `drop_all` (their bodies only use rusqlite, so `?` still works):

```rust
use rusqlite::{Connection, ErrorCode};
use std::path::{Path, PathBuf};

impl Index {
    pub fn open(path: &Path) -> Result<Index> {
        create_parent(path)?;
        Ok(Index::connect(path)?)
    }

    /// Like `open`, but a corrupt file is deleted and created afresh; the flag
    /// says whether that happened.
    pub fn open_or_recreate(path: &Path) -> Result<(Index, bool)> {
        create_parent(path)?;
        match Index::connect(path) {
            Ok(ix) => Ok((ix, false)),
            Err(e) if is_corrupt(&e) => {
                for suffix in ["", "-wal", "-shm"] {
                    let mut name = path.as_os_str().to_owned();
                    name.push(suffix);
                    let p = PathBuf::from(name);
                    match std::fs::remove_file(&p) {
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err(Error::io(p, e));
                        }
                        _ => {}
                    }
                }
                Ok((Index::connect(path)?, true))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn open_in_memory() -> Result<Index> {
        let mut ix = Index {
            conn: Connection::open_in_memory()?,
        };
        ix.prepare()?;
        Ok(ix)
    }

    fn connect(path: &Path) -> rusqlite::Result<Index> {
        let mut ix = Index {
            conn: Connection::open(path)?,
        };
        let check: String = ix.conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            let code = rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT);
            return Err(rusqlite::Error::SqliteFailure(code, Some(check)));
        }
        ix.prepare()?;
        Ok(ix)
    }

    fn prepare(&mut self) -> rusqlite::Result<()> { /* body unchanged */ }

    fn drop_all(&mut self) -> rusqlite::Result<()> { /* body unchanged */ }
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    Ok(())
}

fn is_corrupt(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}
```

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p engram-notes-core && cargo clippy -p engram-notes-core --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat(core): delete and rebuild a corrupt index"
```

### Task 3: Measure the two-second open

Gap 6. An ignored integration test builds a vault of ten thousand notes with frontmatter, links and tags, opens it twice the way `open_vault` does, and asserts the second open is under two seconds. The rebuild stops re-resolving every link when nothing changed, since that is the one step of a no-change open that grows with the link count.

**Files:**
- Create: `core/tests/open_time.rs`
- Modify: `core/src/index/rebuild.rs` (`rebuild`)

**Interfaces:**
- Consumes: `Vault::open`, `Index::open_or_recreate`, `Index::rebuild` returning `RebuildStats`.

- [ ] **Step 1: Write the benchmark**

```rust
//! The spec's bound: ten thousand notes open in under two seconds the second
//! time. Run: `cargo test --release -p engram-notes-core --test open_time -- --ignored --nocapture`

use engram_core::index::Index;
use engram_core::vault::Vault;
use std::time::{Duration, Instant};

#[test]
#[ignore = "benchmark, run in release"]
fn second_open_of_ten_thousand_notes_is_under_two_seconds() {
    let vault_dir = tempfile::tempdir().unwrap();
    let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(20);
    for i in 0..10_000 {
        let dir = vault_dir.path().join(format!("f{}", i % 50));
        std::fs::create_dir_all(&dir).unwrap();
        let next = (i + 1) % 10_000;
        let far = (i * 7) % 10_000;
        let text = format!(
            "---\ntags: [t{}]\nn: {i}\n---\n# Note {i}\n\nSee [[Note {next}]] and [[Note {far}#Note {far}]] #topic/{}\n\n{filler}\n",
            i % 20,
            i % 30
        );
        std::fs::write(dir.join(format!("Note {i}.md")), text).unwrap();
    }
    let data_dir = tempfile::tempdir().unwrap();
    let db = data_dir.path().join("index.db");
    let open = || {
        let start = Instant::now();
        let vault = Vault::open(vault_dir.path()).unwrap();
        let (mut ix, _) = Index::open_or_recreate(&db).unwrap();
        let stats = ix.rebuild(&vault).unwrap();
        (start.elapsed(), stats)
    };
    let (first, s1) = open();
    let (second, s2) = open();
    println!(
        "first open {first:?} ({} added), second open {second:?} ({} unchanged)",
        s1.added, s2.unchanged
    );
    assert_eq!(s2.unchanged, 10_000);
    assert!(second < Duration::from_secs(2), "second open took {second:?}");
}
```

- [ ] **Step 2: Measure before the change**

Run: `cargo test --release -p engram-notes-core --test open_time -- --ignored --nocapture`
Expected: prints both times. Note them for the handoff.

- [ ] **Step 3: Skip resolution when nothing changed**

In `Index::rebuild`, replace `self.resolve_all()?;` with:

```rust
        // Nothing added, changed or removed leaves every resolution valid.
        if stats.added + stats.updated + stats.removed > 0 {
            self.resolve_all()?;
        }
```

- [ ] **Step 4: Measure again and run the suite**

Run: `cargo test --release -p engram-notes-core --test open_time -- --ignored --nocapture && cargo test -p engram-notes-core`
Expected: second open under two seconds, all tests pass. If the second open is still over two seconds, stop and report the numbers before optimising further.

- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "test(core): benchmark the second open of 10k notes"
```

### Task 4: The shell reports recovery and watcher failure

Gap 3, shell and UI half, and gap 5's command serialisation tests. `open_vault` uses `open_or_recreate` and says so; a watcher that fails to start or fails later is reported, and the UI then rescans whenever the window gains focus. `anchor_line` becomes a command for Task 7.

**Files:**
- Modify: `core/src/watch.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/state.svelte.ts`
- Test: `src-tauri/src/commands.rs` test module, `core/src/watch.rs` test

**Interfaces:**
- Consumes: `Index::open_or_recreate` (Task 2), `Index::anchor_line` and `LinkRow.block` (Task 1).
- Produces: `watch(vault, on_event: impl Fn(Result<Vec<Change>, String>) + Send + 'static)`; `VaultInfo { root, config, stats, index_recreated: bool, watch_error: Option<String> }`; commands `rescan() -> RebuildStats`, `anchor_line(path, fragment) -> Option<u32>`; event `watch-failed` with a string payload. In `api.ts`: `rescan()`, `anchorLine(path, fragment)`, `onWatchFailed(f)`; in the store: `watching: boolean`, `rescan()`.

- [ ] **Step 1: Write the failing serialisation tests**

Append to `src-tauri/src/commands.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Each shape is what `ui/src/lib/api.ts` declares.
    #[test]
    fn vault_info() {
        let info = VaultInfo {
            root: "/v".into(),
            config: AppConfig::default(),
            stats: RebuildStats {
                added: 1,
                ..Default::default()
            },
            index_recreated: true,
            watch_error: None,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["root"], "/v");
        assert_eq!(v["config"]["editor"]["default_mode"], "live");
        assert_eq!(v["config"]["daily_notes"]["template"], json!(null));
        assert_eq!(
            v["stats"],
            json!({"added": 1, "updated": 0, "removed": 0, "unchanged": 0})
        );
        assert_eq!(v["index_recreated"], true);
        assert_eq!(v["watch_error"], json!(null));
    }

    #[test]
    fn errors_changes_notes_and_links() {
        let e = CommandError::from(engram_core::Error::NotFound("x.md".into()));
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            json!({"code": "not_found", "message": "not found: x.md"})
        );
        let c = Change {
            path: "a.md".into(),
            kind: ChangeKind::Removed,
        };
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            json!({"path": "a.md", "kind": "removed"})
        );
        let n = NoteText {
            path: "a.md".into(),
            text: "t".into(),
            mtime_ms: 5,
        };
        assert_eq!(
            serde_json::to_value(&n).unwrap(),
            json!({"path": "a.md", "text": "t", "mtime_ms": 5})
        );
        let row = LinkRow {
            src_path: "a.md".into(),
            target_raw: "B".into(),
            target_path: None,
            kind: "wiki".into(),
            heading: None,
            block: Some("x".into()),
            alias: None,
            line: 3,
            context: "c".into(),
        };
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            json!({"src_path": "a.md", "target_raw": "B", "target_path": null, "kind": "wiki",
                   "heading": null, "block": "x", "alias": null, "line": 3, "context": "c"})
        );
    }

    #[test]
    fn rename_plan_from_the_ui() {
        let p: RenamePlan =
            serde_json::from_value(json!({"from": "a.md", "to": "b.md", "affected": ["c.md"]}))
                .unwrap();
        assert_eq!(p.affected, vec!["c.md"]);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p engram-notes`
Expected: compile error, `VaultInfo` has no field `index_recreated`.

- [ ] **Step 3: Watcher errors reach the caller**

In `core/src/watch.rs`, rename the parameter to `on_event` with type `impl Fn(std::result::Result<Vec<Change>, String>) + Send + 'static`, and in the handler:

```rust
            let events = match res {
                Ok(events) => events,
                Err(errors) => {
                    let msg: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                    on_event(Err(msg.join("; ")));
                    return;
                }
            };
```

and `on_event(Ok(out));` where it called `on_change(out)`. The test's callback becomes `move |c| sink.lock().unwrap().extend(c.unwrap_or_default())`.

- [ ] **Step 4: Commands**

In `src-tauri/src/commands.rs`, `VaultInfo` gains:

```rust
    pub index_recreated: bool,
    /// Why the watcher could not start; `None` while it runs.
    pub watch_error: Option<String>,
```

`open_vault` becomes:

```rust
#[tauri::command]
pub fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<VaultInfo> {
    let vault = Vault::open(&path)?;
    let (mut index, index_recreated) = Index::open_or_recreate(&config::index_path(&vault)?)?;
    let stats = index.rebuild(&vault)?;
    let cfg = config::load_config(&vault)?;
    let root = vault.root().to_string_lossy().into_owned();
    remember(&root);

    let handle = app.clone();
    let (watcher, watch_error) =
        match engram_core::watch::watch(&vault, move |ev| on_watch(&handle, ev)) {
            Ok(w) => (Some(w), None),
            Err(e) => (None, Some(e.to_string())),
        };

    *state.open.lock().unwrap() = Some(Open {
        vault,
        index,
        config: cfg.clone(),
        _watcher: watcher,
    });
    Ok(VaultInfo {
        root,
        config: cfg,
        stats,
        index_recreated,
        watch_error,
    })
}

// Runs on the watcher thread. After a failure the UI rescans on focus.
fn on_watch(app: &AppHandle, event: Result<Vec<Change>, String>) {
    match event {
        Ok(changes) => apply_changes(app, changes),
        Err(message) => {
            let _ = app.emit("watch-failed", message);
        }
    }
}

/// Catches up with edits a failed watcher missed.
#[tauri::command]
pub fn rescan(state: State<AppState>) -> CmdResult<RebuildStats> {
    with_open(&state, |o| Ok(o.index.rebuild(&o.vault)?))
}

#[tauri::command]
pub fn anchor_line(state: State<AppState>, path: String, fragment: String) -> CmdResult<Option<u32>> {
    with_open(&state, |o| Ok(o.index.anchor_line(&path, &fragment)?))
}
```

Register `commands::rescan` and `commands::anchor_line` in `src-tauri/src/lib.rs`.

- [ ] **Step 5: Run the Rust suite**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: all pass.

- [ ] **Step 6: The UI reports and falls back**

`ui/src/lib/api.ts`: add `index_recreated: boolean; watch_error: string | null` to `VaultInfo`, `block: string | null` to `LinkRow` after `heading`, and:

```ts
export const rescan = () => invoke<RebuildStats>("rescan");
export const anchorLine = (path: string, fragment: string) => invoke<number | null>("anchor_line", { path, fragment });
export const onWatchFailed = (f: (message: string) => void): Promise<UnlistenFn> =>
  listen<string>("watch-failed", (e) => f(e.payload));
```

`ui/src/lib/state.svelte.ts`: `import { getCurrentWindow } from "@tauri-apps/api/window";`, a field `watching = $state(true);`, and in `open()` after `this.config = info.config;`:

```ts
    if (info.index_recreated) this.say("The index was damaged and has been rebuilt.");
    this.watching = info.watch_error === null;
    if (info.watch_error) this.say(`File watching is off: ${info.watch_error}. Changes are read when the window gains focus.`);
```

and after the two existing listeners:

```ts
    await api.onWatchFailed((msg) => {
      if (this.watching) this.say(`File watching stopped: ${msg}. Changes are read when the window gains focus.`);
      this.watching = false;
    });
    await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && !this.watching) this.rescan().catch((e) => this.say(api.errorMessage(e)));
    });
```

and the method:

```ts
  // Without a watcher, gaining focus is when outside edits are picked up.
  async rescan() {
    await api.rescan();
    await this.refresh();
    for (const t of [...this.tabs]) {
      try {
        await this.externalChange({ path: t.path, kind: "changed" });
      } catch {
        await this.externalChange({ path: t.path, kind: "removed" });
      }
    }
  }
```

- [ ] **Step 7: Check the UI and verify in the app**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, tests pass.

Then: build with `cargo build -p engram-notes`, start `pnpm dev` in `ui` in the background, find the vault's index with `ls ~/.local/share/engram-notes/vaults/*/index.db`, overwrite the one for a scratch vault with `head -c 4096 /dev/urandom > <that file>`, launch `target/debug/engram-notes <vault>`, take a screenshot with `spectacle -b -n -f -o shot.png`. Expected: the toast "The index was damaged and has been rebuilt." and the explorer and backlinks work. Stop with `pkill -f "[t]arget/debug/engram-notes"`.

- [ ] **Step 8: Commit**

```bash
git add core src-tauri ui
git commit -m "feat: report index recovery and fall back when watching fails"
```

### Task 5: Layout tree operations

Gap 1, pure half. The workspace becomes a tree: a pane holds tabs, a split holds panes or splits in a row or a column with relative sizes. Every structural change is a pure function, tested here; Task 6 wires them into the store.

**Files:**
- Create: `ui/src/lib/layout.ts`, `ui/src/lib/layout.test.ts`
- Create: `ui/src/lib/textdiff.ts`, `ui/src/lib/textdiff.test.ts`

**Interfaces:**
- Produces (all in `layout.ts`): types `Mode = "live" | "source" | "reading"`, `Dir = "row" | "column"`, `TabRef { path; mode }`, `Pane { kind: "pane"; id; tabs: TabRef[]; active }`, `Split { kind: "split"; id; dir; sizes: number[]; children: Node[] }`, `Node = Pane | Split`; functions `panes(n): Pane[]`, `findPane(n, id)`, `findSplit(n, id)`, `maxId(n)`, `isNode(v)`, `split(root, paneId, dir, fresh, splitId): Node`, `removePane(root, id): Node`, `closeTab(root, paneId, index): Node`, `withoutPath(root, path): Node`, `renamePath(root, from, to): Node`.
- Produces: `textDiff(a, b): { from: number; to: number; insert: string }` in `textdiff.ts`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/layout.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { closeTab, findPane, maxId, panes, removePane, renamePath, split, withoutPath, type Node, type Pane } from "./layout";

const pane = (id: number, ...paths: string[]): Pane => ({
  kind: "pane",
  id,
  tabs: paths.map((path) => ({ path, mode: "live" as const })),
  active: paths.length ? 0 : -1,
});
const ids = (n: Node) => panes(n).map((p) => p.id);

describe("layout", () => {
  it("splits beside a pane, reusing a split that runs the same way", () => {
    let root: Node = pane(1, "a.md");
    root = split(root, 1, "row", pane(2, "a.md"), 10);
    expect(root).toMatchObject({ kind: "split", id: 10, dir: "row", sizes: [0.5, 0.5] });
    root = split(root, 2, "row", pane(3), 11);
    expect(root).toMatchObject({ id: 10, sizes: [0.5, 0.25, 0.25] });
    root = split(root, 1, "column", pane(4), 12);
    expect(ids(root)).toEqual([1, 4, 2, 3]);
    expect(root.kind === "split" && root.children[0]).toMatchObject({ kind: "split", id: 12, dir: "column" });
    expect(maxId(root)).toBe(12);
  });

  it("gives a removed pane's space away and collapses a lone child", () => {
    const root = split(split(pane(1), 1, "row", pane(2), 10), 2, "row", pane(3), 11);
    const out = removePane(root, 2);
    expect(out).toMatchObject({ id: 10, sizes: [0.75, 0.25] });
    expect(removePane(out, 3)).toEqual(pane(1));
    expect(removePane(pane(1), 1)).toEqual(pane(1));
  });

  it("activates the neighbour of a closed tab and closes an emptied pane", () => {
    const p = { ...pane(1, "a.md", "b.md", "c.md"), active: 1 };
    const one = closeTab(p, 1, 1) as Pane;
    expect(one.tabs.map((t) => t.path)).toEqual(["a.md", "c.md"]);
    expect(one.active).toBe(1);
    expect((closeTab(p, 1, 0) as Pane).active).toBe(0);
    const two = split(pane(1, "a.md"), 1, "row", pane(2, "b.md"), 10);
    expect(closeTab(two, 2, 0)).toEqual(pane(1, "a.md"));
    expect(closeTab(pane(1, "a.md"), 1, 0)).toEqual(pane(1));
  });

  it("drops and renames a path in every pane", () => {
    const root = split(pane(1, "a.md", "b.md"), 1, "row", pane(2, "a.md"), 10);
    expect(withoutPath(root, "a.md")).toEqual(pane(1, "b.md"));
    const renamed = renamePath(root, "a.md", "z.md");
    expect(panes(renamed).flatMap((p) => p.tabs.map((t) => t.path))).toEqual(["z.md", "b.md", "z.md"]);
    expect(findPane(renamed, 2)?.tabs[0].path).toBe("z.md");
  });
});
```

`ui/src/lib/textdiff.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { textDiff } from "./textdiff";

describe("textDiff", () => {
  it("replaces only the changed middle", () => {
    expect(textDiff("hello world", "hello brave world")).toEqual({ from: 6, to: 6, insert: "brave " });
    expect(textDiff("aaa", "aa")).toEqual({ from: 2, to: 3, insert: "" });
    expect(textDiff("same", "same")).toEqual({ from: 4, to: 4, insert: "" });
    expect(textDiff("", "new")).toEqual({ from: 0, to: 0, insert: "new" });
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test`
Expected: FAIL, cannot resolve `./layout` and `./textdiff`.

- [ ] **Step 3: Implement `ui/src/lib/layout.ts`**

```ts
export type Mode = "live" | "source" | "reading";
export type Dir = "row" | "column";
export interface TabRef { path: string; mode: Mode }
export interface Pane { kind: "pane"; id: number; tabs: TabRef[]; active: number }
export interface Split { kind: "split"; id: number; dir: Dir; sizes: number[]; children: Node[] }
export type Node = Pane | Split;

export function panes(n: Node): Pane[] {
  return n.kind === "pane" ? [n] : n.children.flatMap(panes);
}

export function findPane(n: Node, id: number): Pane | undefined {
  return panes(n).find((p) => p.id === id);
}

export function findSplit(n: Node, id: number): Split | undefined {
  if (n.kind === "pane") return undefined;
  if (n.id === id) return n;
  for (const c of n.children) {
    const s = findSplit(c, id);
    if (s) return s;
  }
  return undefined;
}

export function maxId(n: Node): number {
  return n.kind === "pane" ? n.id : Math.max(n.id, ...n.children.map(maxId));
}

export function isNode(v: unknown): v is Node {
  const k = (v as Node | null)?.kind;
  return k === "pane" || k === "split";
}

/** Puts `fresh` after pane `paneId`: inside its split when that runs in `dir`, else in a new split. */
export function split(root: Node, paneId: number, dir: Dir, fresh: Pane, splitId: number): Node {
  const go = (n: Node): Node => {
    if (n.kind === "pane") {
      return n.id === paneId ? { kind: "split", id: splitId, dir, sizes: [0.5, 0.5], children: [n, fresh] } : n;
    }
    const i = n.children.findIndex((c) => c.kind === "pane" && c.id === paneId);
    if (i >= 0 && n.dir === dir) {
      const half = n.sizes[i] / 2;
      return {
        ...n,
        children: [...n.children.slice(0, i + 1), fresh, ...n.children.slice(i + 1)],
        sizes: [...n.sizes.slice(0, i), half, half, ...n.sizes.slice(i + 1)],
      };
    }
    return { ...n, children: n.children.map(go) };
  };
  return go(root);
}

/** Its space goes to the neighbour before it; a split left with one child becomes that child. */
export function removePane(root: Node, id: number): Node {
  const go = (n: Node): Node => {
    if (n.kind === "pane") return n;
    const i = n.children.findIndex((c) => c.kind === "pane" && c.id === id);
    if (i < 0) return { ...n, children: n.children.map(go) };
    const children = n.children.filter((_, k) => k !== i);
    const sizes = n.sizes.filter((_, k) => k !== i);
    sizes[Math.max(0, i - 1)] += n.sizes[i];
    return children.length === 1 ? children[0] : { ...n, children, sizes };
  };
  return go(root);
}

function mapPanes(root: Node, f: (p: Pane) => Pane): Node {
  return root.kind === "pane" ? f(root) : { ...root, children: root.children.map((c) => mapPanes(c, f)) };
}

// The tab that slides into a closed active tab's place becomes active.
function keepTabs(p: Pane, keep: (t: TabRef, i: number) => boolean): Pane {
  const tabs = p.tabs.filter(keep);
  const before = p.tabs.slice(0, Math.max(0, p.active)).filter(keep).length;
  return { ...p, tabs, active: tabs.length ? Math.min(before, tabs.length - 1) : -1 };
}

/** As in Obsidian, closing a pane's last tab closes the pane unless it is the only one. */
export function closeTab(root: Node, paneId: number, index: number): Node {
  const out = mapPanes(root, (p) => (p.id === paneId ? keepTabs(p, (_, i) => i !== index) : p));
  const p = findPane(out, paneId);
  return p && p.tabs.length === 0 && panes(out).length > 1 ? removePane(out, paneId) : out;
}

export function withoutPath(root: Node, path: string): Node {
  const emptied = panes(root)
    .filter((p) => p.tabs.length > 0 && p.tabs.every((t) => t.path === path))
    .map((p) => p.id);
  let out = mapPanes(root, (p) => keepTabs(p, (t) => t.path !== path));
  for (const id of emptied) if (panes(out).length > 1) out = removePane(out, id);
  return out;
}

export function renamePath(root: Node, from: string, to: string): Node {
  return mapPanes(root, (p) => ({ ...p, tabs: p.tabs.map((t) => (t.path === from ? { ...t, path: to } : t)) }));
}
```

- [ ] **Step 4: Implement `ui/src/lib/textdiff.ts`**

```ts
/** The smallest single replacement turning `a` into `b`, so an editor keeps its cursor and scroll. */
export function textDiff(a: string, b: string): { from: number; to: number; insert: string } {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++;
  return { from: start, to: a.length - end, insert: b.slice(start, b.length - end) };
}
```

- [ ] **Step 5: Run the tests**

Run: `cd ui && pnpm test && pnpm check`
Expected: all pass, 0 errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/layout.ts ui/src/lib/layout.test.ts ui/src/lib/textdiff.ts ui/src/lib/textdiff.test.ts
git commit -m "feat(ui): layout tree operations for split panes"
```

### Task 6: Split panes in the store and the window

Gap 1, wiring. Buffers become one `Doc` per open path, shared by every tab that shows the path; tabs live in the layout tree; the centre renders the tree with draggable dividers. `workspace.json` stores `{layout, activePane}` and still reads the foundation's `{tabs, active}`.

**Files:**
- Modify: `ui/src/lib/state.svelte.ts` (rewrite below)
- Create: `ui/src/components/Workspace.svelte`, `ui/src/components/Pane.svelte`
- Modify: `ui/src/components/Tabs.svelte`, `NoteView.svelte`, `Editor.svelte`, `Explorer.svelte`, `Properties.svelte`, `StatusBar.svelte`, `ui/src/App.svelte`, `ui/src/lib/commands.ts`, `ui/src/app.css`

**Interfaces:**
- Consumes: everything from Task 5; `api.rescan`, `api.onWatchFailed` (Task 4).
- Produces, on `app`: `docs: Record<string, Doc>`, `layout: Node`, `activePane: number`, getters `pane: Pane`, `paneCount`, `activeTab: TabRef | null`, `activeDoc: Doc | null`; methods `openNote(path)`, `activate(paneId, index)`, `focusPane(paneId)`, `setMode(paneId, path, mode)`, `closeTab(paneId, index)`, `split(dir)`, `resize(splitId, sizes)`, `renamed(from, to)`, `forget(path)`, `save(doc)`, `externalChange(change)`, `resolveConflict(doc, keepMine)`, `rescan()`, `persist()`, `say(msg)`. `Doc { path; text; savedText; mtime_ms; conflict }`. `NoteView` props `{ paneId, path }`; `Editor` gains prop `focus: boolean`.

- [ ] **Step 1: Rewrite `ui/src/lib/state.svelte.ts`**

```ts
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import * as L from "./layout";
import type { Dir, Mode, Node, Pane, TabRef } from "./layout";

export interface Doc {
  path: string;
  text: string;
  savedText: string;
  mtime_ms: number;
  conflict: boolean;
}

class AppStateStore {
  root = $state<string | null>(null);
  config = $state<api.AppConfig | null>(null);
  files = $state<api.FileEntry[]>([]);
  // One buffer per open path, shared by every tab showing it.
  docs = $state<Record<string, Doc>>({});
  layout = $state<Node>({ kind: "pane", id: 1, tabs: [], active: -1 });
  activePane = $state(1);
  toast = $state<string | null>(null);
  titles = $state<[string, string][]>([]);
  showLeft = $state(true);
  showRight = $state(true);
  palette = $state<"none" | "files" | "commands">("none");
  leftPane = $state<"files" | "search">("files");
  watching = $state(true);
  private nextId = 2;

  get pane(): Pane {
    return L.findPane(this.layout, this.activePane) ?? L.panes(this.layout)[0];
  }

  get paneCount(): number {
    return L.panes(this.layout).length;
  }

  get activeTab(): TabRef | null {
    const p = this.pane;
    return p.active >= 0 ? p.tabs[p.active] : null;
  }

  get activeDoc(): Doc | null {
    const t = this.activeTab;
    return t ? (this.docs[t.path] ?? null) : null;
  }

  async open(root: string) {
    const info = await api.openVault(root);
    this.root = info.root;
    this.config = info.config;
    if (info.index_recreated) this.say("The index was damaged and has been rebuilt.");
    this.watching = info.watch_error === null;
    if (info.watch_error) this.say(`File watching is off: ${info.watch_error}. Changes are read when the window gains focus.`);
    await this.refresh();
    await this.restore(await api.getWorkspace());
    await api.onIndexChanged(() => this.refresh());
    await api.onFileChanged((c) => this.externalChange(c));
    await api.onWatchFailed((msg) => {
      if (this.watching) this.say(`File watching stopped: ${msg}. Changes are read when the window gains focus.`);
      this.watching = false;
    });
    await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && !this.watching) this.rescan().catch((e) => this.say(api.errorMessage(e)));
    });
  }

  // The foundation's `{tabs, active}` still opens, as one pane.
  private async restore(ws: Record<string, unknown>) {
    const mode = this.config?.editor.default_mode ?? "live";
    let layout: Node = L.isNode(ws.layout)
      ? ws.layout
      : {
          kind: "pane",
          id: 1,
          tabs: ((ws.tabs as string[] | undefined) ?? []).map((path) => ({ path, mode })),
          active: Number(ws.active ?? 0),
        };
    for (const path of new Set(L.panes(layout).flatMap((p) => p.tabs.map((t) => t.path)))) {
      try {
        await this.load(path);
      } catch {
        layout = L.withoutPath(layout, path); // gone since last session
      }
    }
    for (const p of L.panes(layout)) p.active = Math.min(Math.max(p.active, 0), p.tabs.length - 1);
    this.layout = layout;
    this.nextId = L.maxId(layout) + 1;
    const wanted = Number(ws.activePane);
    this.activePane = L.findPane(layout, wanted) ? wanted : L.panes(layout)[0].id;
  }

  async refresh() {
    this.files = await api.listFiles();
    this.titles = await api.titles();
  }

  private async load(path: string) {
    if (this.docs[path]) return;
    const n = await api.readNote(path);
    this.docs[path] = { path, text: n.text, savedText: n.text, mtime_ms: n.mtime_ms, conflict: false };
  }

  /** Opens `path` in the active pane, or activates its tab there. */
  async openNote(path: string) {
    const p = this.pane;
    const i = p.tabs.findIndex((t) => t.path === path);
    if (i >= 0) return this.activate(p.id, i);
    await this.load(path);
    p.tabs.push({ path, mode: this.config?.editor.default_mode ?? "live" });
    p.active = p.tabs.length - 1;
    this.persist();
  }

  activate(paneId: number, index: number) {
    const p = L.findPane(this.layout, paneId);
    if (!p) return;
    p.active = index;
    this.activePane = paneId;
    this.persist();
  }

  focusPane(paneId: number) {
    if (this.activePane === paneId) return;
    this.activePane = paneId;
    this.persist();
  }

  setMode(paneId: number, path: string, mode: Mode) {
    const t = L.findPane(this.layout, paneId)?.tabs.find((x) => x.path === path);
    if (!t) return;
    t.mode = mode;
    this.persist();
  }

  closeTab(paneId: number, index: number) {
    const path = L.findPane(this.layout, paneId)?.tabs[index]?.path;
    this.layout = L.closeTab(this.layout, paneId, index);
    this.afterLayoutChange();
    if (path) void this.release(path);
  }

  /** Obsidian's split right and split down: the current note opens again beside itself. */
  split(dir: Dir) {
    const t = this.activeTab;
    const fresh: Pane = { kind: "pane", id: this.nextId++, tabs: t ? [{ ...t }] : [], active: t ? 0 : -1 };
    this.layout = L.split(this.layout, this.pane.id, dir, fresh, this.nextId++);
    this.activePane = fresh.id;
    this.persist();
  }

  resize(splitId: number, sizes: number[]) {
    const s = L.findSplit(this.layout, splitId);
    if (s) s.sizes = sizes;
  }

  renamed(from: string, to: string) {
    const d = this.docs[from];
    if (d) {
      delete this.docs[from];
      d.path = to;
      this.docs[to] = d;
    }
    this.layout = L.renamePath(this.layout, from, to);
    this.persist();
  }

  forget(path: string) {
    this.layout = L.withoutPath(this.layout, path);
    delete this.docs[path];
    this.afterLayoutChange();
  }

  private afterLayoutChange() {
    if (!L.findPane(this.layout, this.activePane)) this.activePane = L.panes(this.layout)[0].id;
    this.persist();
  }

  private shown(path: string): boolean {
    return L.panes(this.layout).some((p) => p.tabs.some((t) => t.path === path));
  }

  // A buffer no tab shows any more is saved, then dropped.
  private async release(path: string) {
    const d = this.docs[path];
    if (!d || this.shown(path)) return;
    await this.save(d);
    if (!this.shown(path)) delete this.docs[path];
  }

  async save(doc: Doc) {
    if (doc.text === doc.savedText) return;
    try {
      doc.mtime_ms = await api.writeNote(doc.path, doc.text);
      doc.savedText = doc.text;
    } catch (e) {
      this.say(api.errorMessage(e));
    }
  }

  async externalChange(c: api.Change) {
    const doc = this.docs[c.path];
    if (!doc) return;
    if (c.kind === "removed") {
      doc.conflict = doc.text !== doc.savedText;
      return;
    }
    const n = await api.readNote(c.path);
    if (n.text === doc.savedText) return;
    if (doc.text === doc.savedText) {
      doc.text = n.text;
      doc.savedText = n.text;
      doc.mtime_ms = n.mtime_ms;
    } else {
      doc.conflict = true;
    }
  }

  async resolveConflict(doc: Doc, keepMine: boolean) {
    if (keepMine) {
      doc.savedText = "";
      await this.save(doc);
    } else {
      const n = await api.readNote(doc.path);
      doc.text = n.text;
      doc.savedText = n.text;
      doc.mtime_ms = n.mtime_ms;
    }
    doc.conflict = false;
  }

  // Without a watcher, gaining focus is when outside edits are picked up.
  async rescan() {
    await api.rescan();
    await this.refresh();
    for (const path of Object.keys(this.docs)) {
      try {
        await this.externalChange({ path, kind: "changed" });
      } catch {
        await this.externalChange({ path, kind: "removed" });
      }
    }
  }

  persist() {
    if (!this.root) return;
    void api.setWorkspace({ layout: $state.snapshot(this.layout), activePane: this.activePane });
  }

  say(msg: string) {
    this.toast = msg;
    setTimeout(() => (this.toast = null), 4000);
  }
}

export const app = new AppStateStore();
```

- [ ] **Step 2: `ui/src/components/Workspace.svelte`**

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Node } from "../lib/layout";
  import Pane from "./Pane.svelte";
  import Workspace from "./Workspace.svelte";

  let { node }: { node: Node } = $props();

  // A divider trades size between the two children beside it.
  function drag(e: PointerEvent, i: number) {
    if (node.kind !== "split") return;
    e.preventDefault();
    const { id, dir } = node;
    const sizes = [...node.sizes];
    const box = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect();
    const total = dir === "row" ? box.width : box.height;
    const start = dir === "row" ? e.clientX : e.clientY;
    const [a, b] = [sizes[i], sizes[i + 1]];
    const move = (m: PointerEvent) => {
      const d = ((dir === "row" ? m.clientX : m.clientY) - start) / total;
      const shift = Math.max(0.1 - a, Math.min(b - 0.1, d));
      const next = [...sizes];
      next[i] = a + shift;
      next[i + 1] = b - shift;
      app.resize(id, next);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      app.persist();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

{#if node.kind === "pane"}
  <Pane pane={node} />
{:else}
  <div class="split {node.dir}">
    {#each node.children as child, i (`${child.kind}${child.id}`)}
      {#if i > 0}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="divider" onpointerdown={(e) => drag(e, i - 1)}></div>
      {/if}
      <div class="cell" style="flex:{node.sizes[i]} 1 0px"><Workspace node={child} /></div>
    {/each}
  </div>
{/if}
```

- [ ] **Step 3: `ui/src/components/Pane.svelte`**

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Pane } from "../lib/layout";
  import Tabs from "./Tabs.svelte";
  import NoteView from "./NoteView.svelte";

  let { pane }: { pane: Pane } = $props();
  const tab = $derived(pane.active >= 0 ? pane.tabs[pane.active] : null);
</script>

<section
  class="pane"
  class:focused={app.activePane === pane.id && app.paneCount > 1}
  onfocusin={() => app.focusPane(pane.id)}
  onpointerdown={() => app.focusPane(pane.id)}
>
  <Tabs {pane} />
  {#if tab}
    {#key tab.path}<NoteView paneId={pane.id} path={tab.path} />{/key}
  {:else}
    <div class="note empty">No note open</div>
  {/if}
</section>
```

- [ ] **Step 4: `Tabs.svelte`, `NoteView.svelte`, `Editor.svelte`**

`ui/src/components/Tabs.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Pane } from "../lib/layout";
  let { pane }: { pane: Pane } = $props();
  const title = (p: string) => p.split("/").pop()!.replace(/\.md$/i, "");
  const dirty = (p: string) => {
    const d = app.docs[p];
    return !!d && d.text !== d.savedText;
  };
</script>

<div class="tabs">
  {#each pane.tabs as t, i (t.path)}
    <button class:active={i === pane.active} onclick={() => app.activate(pane.id, i)}>
      {title(t.path)}{dirty(t.path) ? " •" : ""}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.closeTab(pane.id, i); }} onkeydown={() => {}}>×</span>
    </button>
  {/each}
</div>
```

`ui/src/components/NoteView.svelte`, script and markup (the `follow` and `toggleTask` bodies stay, with `tab` read as `doc`):

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { findPane } from "../lib/layout";
  import { resolveLink, createNote, tags as apiTags, errorMessage } from "../lib/api";
  import Editor from "./Editor.svelte";
  import Reading from "./Reading.svelte";

  let { paneId, path }: { paneId: number; path: string } = $props();
  // Read from the store so edits mutate state the store owns.
  const doc = $derived(app.docs[path]);
  const mode = $derived(findPane(app.layout, paneId)?.tabs.find((t) => t.path === path)?.mode ?? "live");
  let timer: ReturnType<typeof setTimeout> | undefined;
  let tagList = $state<string[]>([]);
  onMount(async () => {
    tagList = (await apiTags()).map((t) => t.tag);
  });

  function onChange(text: string) {
    if (text === doc.text) return;
    doc.text = text;
    clearTimeout(timer);
    const d = doc;
    timer = setTimeout(() => app.save(d), 500);
  }

  async function follow(target: string) {
    const [name] = target.split("#");
    try {
      let found = await resolveLink(name);
      if (!found) {
        found = `${name}.md`;
        await createNote(found);
        await app.refresh();
      }
      await app.openNote(found);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  function toggleTask(line: number) {
    const lines = doc.text.split("\n");
    if (line < 0 || line >= lines.length) return;
    lines[line] = lines[line].replace(/^(\s*[-*+]\s+)\[( |x|X)\]/, (_, p, c) => `${p}[${c === " " ? "x" : " "}]`);
    onChange(lines.join("\n"));
  }

  const modes = ["live", "source", "reading"] as const;
</script>

{#if doc}
  {#if doc.conflict}
    <div class="conflict">
      This file changed on disk while you had unsaved edits.
      <button onclick={() => app.resolveConflict(doc, false)}>Reload from disk</button>
      <button onclick={() => app.resolveConflict(doc, true)}>Keep mine</button>
    </div>
  {/if}
  <div class="modes">
    {#each modes as m (m)}
      <button class:active={mode === m} onclick={() => app.setMode(paneId, path, m)}>{m}</button>
    {/each}
  </div>
  <div class="note">
    {#if mode === "reading"}
      <Reading text={doc.text} onFollow={follow} onToggleTask={toggleTask} />
    {:else}
      <Editor
        text={doc.text}
        {mode}
        focus={app.activePane === paneId}
        onchange={onChange}
        onblur={() => app.save(doc)}
        onFollow={follow}
        titles={() => app.titles}
        tags={() => tagList}
      />
    {/if}
  </div>
{/if}
```

`onChange` returns early on equal text because a second pane's editor echoes the change it was given.

`ui/src/components/Editor.svelte`: add `focus: boolean` to `Props` and the destructuring; import `Transaction` from `@codemirror/state` and `textDiff` from `../lib/textdiff`; replace `view.focus();` in `onMount` with `if (focus) view.focus();` (several panes mount at once, and focusing one makes it active); replace the last effect with:

```ts
  // Another pane's typing or an outside reload changed the note. Only the
  // difference is applied, so this editor keeps its cursor and scroll.
  $effect(() => {
    if (!view) return;
    const current = view.state.doc.toString();
    if (text !== current) {
      view.dispatch({ changes: textDiff(current, text), annotations: Transaction.addToHistory.of(false) });
    }
  });
```

- [ ] **Step 5: The other consumers**

`ui/src/App.svelte`: drop the `Tabs` and `NoteView` imports, `import Workspace from "./components/Workspace.svelte";`, and the centre becomes:

```svelte
    <main class="centre">
      <Workspace node={app.layout} />
    </main>
```

`ui/src/lib/commands.ts`, replace the `close-tab`, `toggle-mode`, `toggle-reading` and `save` entries and add the splits (no default hotkey, as in Obsidian):

```ts
  { id: "close-tab", name: "Close current tab", hotkey: "Ctrl+W", run: () => { const p = app.pane; if (p.active >= 0) app.closeTab(p.id, p.active); } },
  { id: "toggle-mode", name: "Toggle live preview / source", hotkey: "Ctrl+E", run: () => { const t = app.activeTab; if (t) app.setMode(app.pane.id, t.path, t.mode === "source" ? "live" : "source"); } },
  { id: "toggle-reading", name: "Toggle reading view", hotkey: "Ctrl+Shift+E", run: () => { const t = app.activeTab; if (t) app.setMode(app.pane.id, t.path, t.mode === "reading" ? "live" : "reading"); } },
  { id: "split-right", name: "Split right", hotkey: "", run: () => app.split("row") },
  { id: "split-down", name: "Split down", hotkey: "", run: () => app.split("column") },
  { id: "save", name: "Save", hotkey: "Ctrl+S", run: () => { const d = app.activeDoc; if (d) return app.save(d); } },
```

`ui/src/components/Palette.svelte`: key the `{#each}` by `it.label + it.detail`, since the split commands have no hotkey and an empty detail would repeat.

`ui/src/components/Explorer.svelte`: in `rename`, replace the two `tab` lines with `app.renamed(path, plan.to);`; in `remove`, replace the two `closeTab` lines with `app.forget(path);`.

`ui/src/components/Properties.svelte`, `commit` becomes:

```ts
  async function commit(key: string, raw: string) {
    const doc = app.activeDoc;
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would otherwise conflict with the rewrite
      await setProperty(doc.path, key, parse(raw));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
```

`ui/src/components/StatusBar.svelte`: `words` reads `app.activeDoc` instead of `app.activeTab`, and so does its `{#if}`.

`ui/src/app.css`: replace the `.centre` rule and add:

```css
.centre { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.split { display: flex; flex: 1; min-width: 0; min-height: 0; }
.split.column { flex-direction: column; }
.split > .cell { display: flex; min-width: 0; min-height: 0; overflow: hidden; }
.split > .divider { flex: 0 0 5px; cursor: col-resize; background: linear-gradient(var(--border), var(--border)) center / 1px 100% no-repeat; }
.split.column > .divider { cursor: row-resize; background-size: 100% 1px; }
.split > .divider:hover { background-color: var(--accent-bg); }
.pane { display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; }
.pane.focused .tabs button.active { box-shadow: inset 0 2px 0 var(--accent); }
.note.empty { display: grid; place-items: center; color: var(--fg-muted); }
```

- [ ] **Step 6: Check**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all tests pass. Fix any remaining `app.tabs` or `app.active` reference the checker reports by reading `app.pane` or `app.activeDoc`.

- [ ] **Step 7: Verify in the running app**

Make a scratch vault with `A.md` (a heading, a paragraph, a task) and `B.md` (a callout). Write `<vault>/.engram-notes/workspace.json`:

```json
{"layout":{"kind":"split","id":3,"dir":"row","sizes":[0.5,0.5],"children":[{"kind":"pane","id":1,"tabs":[{"path":"A.md","mode":"live"}],"active":0},{"kind":"split","id":4,"dir":"column","sizes":[0.6,0.4],"children":[{"kind":"pane","id":2,"tabs":[{"path":"B.md","mode":"reading"}],"active":0},{"kind":"pane","id":5,"tabs":[{"path":"A.md","mode":"source"}],"active":0}]}]},"activePane":2}
```

Start `pnpm dev` in `ui` in the background, launch `target/debug/engram-notes <vault>`, screenshot with `spectacle -b -n -f -o shot.png`, read it. Expected: three panes, A live on the left, B reading top right, A source bottom right, dividers drawn, the top-right pane's tab underlined. Then write the foundation format `{"tabs":["A.md","B.md"],"active":1}`, relaunch, screenshot: one pane, B active. Stop with `pkill -f "[t]arget/debug/engram-notes"`.

- [ ] **Step 8: Commit**

```bash
git add ui
git commit -m "feat(ui): split panes with draggable dividers"
```

### Task 7: Attachments open in a preview tab

Gap 2. Clicking a non-markdown file opens it in a tab: images and PDFs render through Tauri's asset protocol, scoped at runtime to the open vault; every attachment offers *Open in default app*. Only notes get a `Doc`.

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Create: `ui/src/lib/files.ts`, `ui/src/lib/files.test.ts`, `ui/src/components/FileView.svelte`
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/state.svelte.ts`, `ui/src/components/Pane.svelte`, `ui/src/components/Properties.svelte`, `ui/src/app.css`

**Interfaces:**
- Consumes: the store and `Pane.svelte` from Task 6.
- Produces: commands `attachment_path(path) -> String` (absolute path) and `open_external(path)`; `api.attachmentPath`, `api.openExternal`; `fileKind(path): "note" | "image" | "pdf" | "other"` in `files.ts`.

- [ ] **Step 1: Write the failing test**

`ui/src/lib/files.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { fileKind } from "./files";

describe("fileKind", () => {
  it("sorts files by what can show them", () => {
    expect(fileKind("a/B.MD")).toBe("note");
    expect(fileKind("pics/x.PNG")).toBe("image");
    expect(fileKind("scan.webp")).toBe("image");
    expect(fileKind("paper.pdf")).toBe("pdf");
    expect(fileKind("data.csv")).toBe("other");
    expect(fileKind("Makefile")).toBe("other");
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ui && pnpm test files`
Expected: FAIL, cannot resolve `./files`.

- [ ] **Step 3: `ui/src/lib/files.ts`**

```ts
export type FileKind = "note" | "image" | "pdf" | "other";

// The image formats Obsidian previews.
const IMAGE = /\.(png|jpe?g|gif|bmp|svg|webp|avif)$/i;

export function fileKind(path: string): FileKind {
  if (/\.md$/i.test(path)) return "note";
  if (IMAGE.test(path)) return "image";
  if (/\.pdf$/i.test(path)) return "pdf";
  return "other";
}
```

Run: `cd ui && pnpm test files`
Expected: PASS.

- [ ] **Step 4: The asset protocol and two commands**

`src-tauri/Cargo.toml`: `tauri = { version = "2", features = ["protocol-asset"] }`.

`src-tauri/tauri.conf.json`: `"security": { "csp": null, "assetProtocol": { "enable": true, "scope": [] } }`. The scope starts empty; `open_vault` grants the vault.

`src-tauri/src/commands.rs`: `use tauri_plugin_opener::OpenerExt;`. In `open_vault`, right after `let vault = Vault::open(&path)?;`:

```rust
    app.asset_protocol_scope()
        .allow_directory(vault.root(), true)
        .map_err(|e| CommandError {
            code: "io",
            message: e.to_string(),
        })?;
```

and add:

```rust
/// A vault file's absolute path, for the webview's asset protocol.
#[tauri::command]
pub fn attachment_path(state: State<AppState>, path: String) -> CmdResult<String> {
    with_open(&state, |o| Ok(o.vault.abs(&path).to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn open_external(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<()> {
    let abs = with_open(&state, |o| Ok(o.vault.abs(&path)))?;
    app.opener()
        .open_path(abs.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| CommandError {
            code: "io",
            message: e.to_string(),
        })
}
```

Register both in `src-tauri/src/lib.rs`.

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean, all pass.

- [ ] **Step 5: The UI**

`ui/src/lib/api.ts`:

```ts
export const attachmentPath = (path: string) => invoke<string>("attachment_path", { path });
export const openExternal = (path: string) => invoke<void>("open_external", { path });
```

`ui/src/lib/state.svelte.ts`: `import { fileKind } from "./files";`. In `restore`, load only notes: `if (fileKind(path) === "note") await this.load(path);` inside the `try`. In `openNote`, replace `await this.load(path);` with `if (fileKind(path) === "note") await this.load(path);`.

`ui/src/components/FileView.svelte`:

```svelte
<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { app } from "../lib/state.svelte";
  import { attachmentPath, openExternal, errorMessage } from "../lib/api";
  import { fileKind } from "../lib/files";

  let { path }: { path: string } = $props();
  const kind = $derived(fileKind(path));
  const name = $derived(path.split("/").pop());
  const size = $derived(app.files.find((f) => f.path === path)?.size ?? 0);
  let src = $state<string | null>(null);

  $effect(() => {
    attachmentPath(path)
      .then((abs) => (src = convertFileSrc(abs)))
      .catch((e) => app.say(errorMessage(e)));
  });

  function external() {
    openExternal(path).catch((e) => app.say(errorMessage(e)));
  }
</script>

<div class="fileview" class:pdf={kind === "pdf"}>
  {#if src && kind === "image"}
    <img {src} alt={name} />
  {:else if src && kind === "pdf"}
    <iframe {src} title={name}></iframe>
  {:else if kind === "other"}
    <div class="name">{name}</div>
    <div class="size">{(size / 1024).toFixed(1)} KB</div>
  {/if}
  <button class="external" onclick={external}>Open in default app</button>
</div>
```

`ui/src/components/Pane.svelte`: `import FileView from "./FileView.svelte";` and `import { fileKind } from "../lib/files";`; inside the `{#key}`:

```svelte
    {#key tab.path}
      {#if fileKind(tab.path) === "note"}
        <NoteView paneId={pane.id} path={tab.path} />
      {:else}
        <FileView path={tab.path} />
      {/if}
    {/key}
```

`ui/src/components/Properties.svelte`: `{#if app.activeTab}` becomes `{#if app.activeDoc}`, since an image has no frontmatter.

`ui/src/app.css`:

```css
.fileview { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; padding: 16px; }
.fileview.pdf { justify-content: stretch; align-items: stretch; }
.fileview img { max-width: 100%; max-height: calc(100% - 48px); object-fit: contain; }
.fileview iframe { flex: 1; border: 0; background: var(--bg-2); }
.fileview .name { font-weight: 500; }
.fileview .size { color: var(--fg-muted); font-size: 12px; }
.fileview .external { align-self: center; padding: 4px 10px; border: 1px solid var(--border); border-radius: var(--radius); color: var(--fg-muted); }
.fileview .external:hover { background: var(--bg-3); color: var(--fg); }
```

- [ ] **Step 6: Check**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all pass.

- [ ] **Step 7: Verify in the running app**

In the scratch vault: `cp src-tauri/icons/128x128.png <vault>/pic.png`, copy any PDF (`find /usr/share -name '*.pdf' | head -1`) to `<vault>/doc.pdf`, and write `data.csv`. Workspace:

```json
{"layout":{"kind":"split","id":4,"dir":"row","sizes":[0.34,0.33,0.33],"children":[{"kind":"pane","id":1,"tabs":[{"path":"pic.png","mode":"live"}],"active":0},{"kind":"pane","id":2,"tabs":[{"path":"doc.pdf","mode":"live"}],"active":0},{"kind":"pane","id":3,"tabs":[{"path":"data.csv","mode":"live"}],"active":0}]},"activePane":1}
```

Restart `pnpm dev --force` (a new Tauri feature changes the bundle), launch, screenshot. Expected: the image centred, the PDF rendered (on WebKitGTK it may show blank or a download prompt; if so, record that in the handoff and keep the button), the CSV's name, size and the button. The right sidebar shows no properties for the image.

- [ ] **Step 8: Commit**

```bash
git add src-tauri ui
git commit -m "feat: preview images and PDFs in a tab"
```

### Task 8: Follow links to headings and blocks; decoration tests

Gap 4, UI half, and gap 5's editor decoration tests. A link with `#Heading` or `#^block` opens the note scrolled to that line, in live, source and reading mode; a backlink opens its source at the mentioning line. Block ids are muted in live preview and hidden in reading view, as in Obsidian. The decoration builder becomes a function of editor state so vitest can run it without a DOM.

**Files:**
- Modify: `ui/src/lib/wikilink.ts`, `ui/src/lib/wikilink.test.ts`
- Modify: `ui/src/lib/render.ts`, `ui/src/lib/render.test.ts`
- Modify: `ui/src/editor/livePreview.ts`; Create: `ui/src/editor/livePreview.test.ts`
- Modify: `ui/src/lib/state.svelte.ts`, `ui/src/components/NoteView.svelte`, `Editor.svelte`, `Reading.svelte`, `Backlinks.svelte`

**Interfaces:**
- Consumes: `api.anchorLine(path, fragment): Promise<number | null>` (Task 4), the store from Tasks 6 and 7.
- Produces: `WikiLink.block?: string`; `linkTarget(l: WikiLink): string` (`"Note"`, `"Note#Heading"` or `"Note#^id"`); `buildDecorations(state: EditorState, ranges: readonly { from: number; to: number }[]): DecorationSet` and `foldFor(state): DecorationSet` exported from `livePreview.ts`; store field `jump: { pane: number; path: string; line: number } | null` and `openNote(path, line?)` where `line` is 1-based; rendered block elements carry `data-source-line` (0-based); `Editor` and `Reading` props `jump: { line: number } | null` and `onJumped: () => void`.

- [ ] **Step 1: Write the failing tests**

Append to `ui/src/lib/wikilink.test.ts` (and add `linkTarget` to its import):

```ts
describe("block links", () => {
  it("separates a block from a heading", () => {
    const [b, h] = findWikilinks("[[N#^b1]] [[N#H]]");
    expect([b.block, b.heading]).toEqual(["b1", undefined]);
    expect([h.block, h.heading]).toEqual([undefined, "H"]);
    expect([displayText(b), linkTarget(b), linkTarget(h)]).toEqual(["N › ^b1", "N#^b1", "N#H"]);
  });
});
```

Append to `ui/src/lib/render.test.ts`:

```ts
  it("hides block ids and marks source lines", () => {
    const h = renderMarkdown("# Top\n\nsome text ^abc\n\n- item ^li");
    expect(h).not.toContain("^abc");
    expect(h).not.toContain("^li");
    expect(h).toContain('<h1 data-source-line="0">');
    expect(h).toContain('<p data-source-line="2">some text</p>');
  });
```

`ui/src/editor/livePreview.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { ensureSyntaxTree } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { yamlFrontmatter } from "@codemirror/lang-yaml";
import { buildDecorations, foldFor } from "./livePreview";

function stateOf(doc: string, cursor: number) {
  const state = EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [yamlFrontmatter({ content: markdown({ base: markdownLanguage }) })],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

// Each decoration as the text it covers and a name: widget class, CSS class, or "hide".
function decos(doc: string, cursor = doc.length) {
  const out: { text: string; kind: string }[] = [];
  buildDecorations(stateOf(doc, cursor), [{ from: 0, to: doc.length }]).between(0, doc.length, (from, to, d) => {
    const spec = d.spec;
    const kind = spec.widget ? spec.widget.constructor.name : (spec.class ?? "hide");
    out.push({ text: doc.slice(from, to), kind });
  });
  return out;
}

describe("live preview decorations", () => {
  it("hides heading marks except on the cursor line", () => {
    expect(decos("# Title\n\ntext")).toContainEqual({ text: "# ", kind: "hide" });
    expect(decos("# Title\n\ntext", 0)).not.toContainEqual({ text: "# ", kind: "hide" });
  });

  it("draws wikilinks, showing the source on the cursor line", () => {
    expect(decos("[[Note|Shown]] x\n\nend")).toContainEqual({ text: "[[Note|Shown]]", kind: "WikiWidget" });
    expect(decos("[[Note|Shown]] x\n\nend", 0)).toContainEqual({ text: "[[Note|Shown]]", kind: "cm-wikilink-src" });
  });

  it("draws task markers as checkboxes", () => {
    const d = decos("- [ ] a\n- [x] b\n\nz");
    expect(d.filter((x) => x.kind === "CheckboxWidget").map((x) => x.text)).toEqual(["[ ]", "[x]"]);
  });

  it("styles callouts and replaces their marker", () => {
    const d = decos("> [!note] Hi\n> body\n\nz");
    expect(d.filter((x) => x.kind === "cm-callout")).toHaveLength(2);
    expect(d).toContainEqual({ text: "[!note]", kind: "CalloutTitle" });
  });

  it("mutes block ids", () => {
    expect(decos("para ^abc\n\nz")).toContainEqual({ text: "^abc", kind: "cm-block-id" });
  });

  it("folds frontmatter unless the cursor is inside it", () => {
    const doc = "---\na: 1\n---\nbody";
    expect(foldFor(stateOf(doc, doc.length)).size).toBe(1);
    expect(foldFor(stateOf(doc, 5)).size).toBe(0);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test`
Expected: FAIL: `linkTarget` and `buildDecorations` are not exported, block ids still rendered.

- [ ] **Step 3: `wikilink.ts`**

Add `block?: string;` to `WikiLink` after `heading`. In `findWikilinks`, split the fragment:

```ts
  for (const m of text.matchAll(RE)) {
    const fragment = m[3]?.trim() || undefined;
    const block = fragment?.startsWith("^") ? fragment.slice(1) : undefined;
    out.push({
      target: m[2].trim(),
      heading: block ? undefined : fragment,
      block,
      alias: m[4]?.trim() || undefined,
      embed: m[1] === "!",
      from: m.index,
      to: m.index + m[0].length,
    });
  }
```

`displayText` and a new `linkTarget`:

```ts
export function displayText(l: WikiLink): string {
  if (l.alias) return l.alias;
  if (l.block) return `${l.target} › ^${l.block}`;
  return l.heading ? `${l.target} › ${l.heading}` : l.target;
}

/** The link as `follow` takes it: target plus `#heading` or `#^block`. */
export function linkTarget(l: WikiLink): string {
  if (l.block) return `${l.target}#^${l.block}`;
  return l.heading ? `${l.target}#${l.heading}` : l.target;
}
```

- [ ] **Step 4: `render.ts`**

Import `linkTarget`; in `wikilinkHtml` replace the `target` line with `const target = linkTarget(l);`. Add two core rules at the end of the file's rule registrations:

```ts
// Obsidian hides `^id` in reading view; the id stays in the file. Runs
// before `tags`, which turns text tokens holding a tag into html.
md.core.ruler.before("tags", "block_ids", (state) => {
  for (const tok of state.tokens) {
    const kids = tok.type === "inline" ? tok.children : null;
    const last = kids?.[kids.length - 1];
    if (last?.type === "text") last.content = last.content.replace(/(^|\s)\^[A-Za-z0-9-]+\s*$/, "");
  }
});

// Block elements carry their 0-based source line so a jump can find them.
md.core.ruler.push("source_lines", (state) => {
  for (const tok of state.tokens) {
    if (tok.map && tok.nesting === 1) tok.attrSet("data-source-line", String(tok.map[0]));
  }
});
```

The existing expectations still hold: markdown-it prints attributes in the order they were set, so the callout's `class` comes before `data-source-line`, and checkboxes keep their own `data-line`.

- [ ] **Step 5: `livePreview.ts`**

Import `linkTarget`. Rename `function build(view: EditorView)` to:

```ts
export function buildDecorations(state: EditorState, ranges: readonly { from: number; to: number }[]): DecorationSet {
  const active = activeLines(state);
```

and iterate `for (const { from, to } of ranges)`. In the wikilink loop use `const target = linkTarget(l);`. After the wikilink loop, inside the same range loop:

```ts
    for (const m of text.matchAll(BLOCK_ID)) {
      const start = from + m.index + m[1].length;
      push(start, start + m[2].length, blockId);
    }
```

with, at the top of the file:

```ts
const BLOCK_ID = /(^|[ \t])(\^[A-Za-z0-9-]+)[ \t]*$/gm;
const blockId = Decoration.mark({ class: "cm-block-id" });
```

Export `foldFor` (`export function foldFor`). The plugin calls `buildDecorations(view.state, view.visibleRanges)` in both places. Add to the base theme: `".cm-block-id": { color: "var(--fg-muted)", fontSize: "0.85em" },`.

Run: `cd ui && pnpm test`
Expected: all pass.

- [ ] **Step 6: Jumps**

`ui/src/lib/state.svelte.ts`: a field `jump = $state<{ pane: number; path: string; line: number } | null>(null);` and `openNote` becomes:

```ts
  /** Opens `path` in the active pane, or activates its tab there; `line` scrolls to a 1-based line. */
  async openNote(path: string, line?: number) {
    const p = this.pane;
    const i = p.tabs.findIndex((t) => t.path === path);
    if (i >= 0) {
      this.activate(p.id, i);
    } else {
      if (fileKind(path) === "note") await this.load(path);
      p.tabs.push({ path, mode: this.config?.editor.default_mode ?? "live" });
      p.active = p.tabs.length - 1;
      this.persist();
    }
    if (line) this.jump = { pane: p.id, path, line };
  }
```

`ui/src/components/NoteView.svelte`: import `anchorLine`; add

```ts
  const jump = $derived(app.jump && app.jump.pane === paneId && app.jump.path === path ? app.jump : null);
  const jumped = () => (app.jump = null);
```

replace `follow` with:

```ts
  async function follow(target: string) {
    const hash = target.indexOf("#");
    const name = hash < 0 ? target : target.slice(0, hash);
    const fragment = hash < 0 ? "" : target.slice(hash + 1);
    try {
      let found = await resolveLink(name);
      if (!found) {
        found = `${name}.md`;
        await createNote(found);
        await app.refresh();
      }
      const line = fragment ? await anchorLine(found, fragment) : null;
      await app.openNote(found, line ?? undefined);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
```

and pass `{jump} onJumped={jumped}` to both `<Reading>` and `<Editor>`.

`ui/src/components/Editor.svelte`: add `jump: { line: number } | null` and `onJumped: () => void` to `Props`; declare the view as `let view = $state.raw<EditorView>();` so effects re-run once it exists; add:

```ts
  $effect(() => {
    if (!view || !jump) return;
    const doc = view.state.doc;
    const line = doc.line(Math.min(Math.max(jump.line, 1), doc.lines));
    view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: "start", yMargin: 24 }) });
    view.focus();
    onJumped();
  });
```

`ui/src/components/Reading.svelte`: add the two props and a host element:

```svelte
<script lang="ts">
  import { renderMarkdown } from "../lib/render";
  interface Props {
    text: string;
    jump: { line: number } | null;
    onJumped: () => void;
    onFollow: (t: string) => void;
    onToggleTask: (line: number) => void;
  }
  let { text, jump, onJumped, onFollow, onToggleTask }: Props = $props();
  const html = $derived(renderMarkdown(text));
  let host = $state<HTMLDivElement>();

  // Elements come in source order, so the last one starting at or before the line holds it.
  $effect(() => {
    if (!host || !jump) return;
    void html;
    let best: HTMLElement | undefined;
    for (const el of host.querySelectorAll<HTMLElement>("[data-source-line]")) {
      if (Number(el.dataset.sourceLine) <= jump.line - 1) best = el;
    }
    best?.scrollIntoView({ block: "start" });
    onJumped();
  });
  // onClick unchanged
</script>
```

with `bind:this={host}` on the `.reading` div.

`ui/src/components/Backlinks.svelte`: `onclick={() => app.openNote(r.src_path, r.line)}`.

- [ ] **Step 7: Check**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all pass.

- [ ] **Step 8: Verify in the running app**

Scratch vault: `Long.md` with 80 filler lines, then `## Target`, 40 more lines, then `the block ^blk`; `Links.md` containing `[[Long#Target]]` and `[[Long#^blk]]`. Seed a row split: `Links.md` live on the left, `Long.md` live on the right scrolled nowhere in particular. Screenshot. Expected: the links read `Long › Target` and `Long › ^blk`; `^blk` is muted wherever it is visible. Switch `Long.md` to reading in `workspace.json`, relaunch, screenshot: no `^blk` in the rendered text. The scroll on click needs a mouse; add it to the handoff's list of things to check by hand.

- [ ] **Step 9: Commit**

```bash
git add ui
git commit -m "feat(ui): follow links to headings and blocks"
```

### Task 9: Palette logic and command tests

Gap 5, the command palette. Ranking and filtering move out of `Palette.svelte` into `lib/palette.ts`; hotkey chords and the `app.json` overrides get tests.

**Files:**
- Create: `ui/src/lib/palette.ts`, `ui/src/lib/palette.test.ts`, `ui/src/lib/commands.test.ts`
- Modify: `ui/src/components/Palette.svelte`

**Interfaces:**
- Consumes: `Command`, `allCommands`, `chord` from `commands.ts`; `app.config` from the store.
- Produces: `matchNotes(titles: [string, string][], query: string): { path: string; title: string }[]`, `createName(matches, query): string | null`, `matchCommands(cmds: Command[], query: string): Command[]`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/palette.test.ts`:

```ts
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
```

`ui/src/lib/commands.test.ts`:

```ts
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
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd ui && pnpm test`
Expected: FAIL, cannot resolve `./palette`. `commands.test.ts` passes already; it pins behaviour the refactor must keep.

- [ ] **Step 3: `ui/src/lib/palette.ts`**

```ts
import type { Command } from "./commands";

export interface NoteMatch { path: string; title: string }

function score(title: string, needle: string): number {
  const t = title.toLowerCase();
  return t === needle ? 3 : t.startsWith(needle) ? 2 : t.includes(needle) ? 1 : 0;
}

/** Notes whose title or path contains the query, best title matches first. */
export function matchNotes(titles: [string, string][], query: string): NoteMatch[] {
  const needle = query.toLowerCase().trim();
  return titles
    .filter(([p, t]) => !needle || t.toLowerCase().includes(needle) || p.toLowerCase().includes(needle))
    .sort((a, b) => score(b[1], needle) - score(a[1], needle))
    .slice(0, 30)
    .map(([path, title]) => ({ path, title }));
}

/** The note name a Create entry offers, or null when a title already matches exactly. */
export function createName(matches: NoteMatch[], query: string): string | null {
  const name = query.trim();
  if (!name || matches.some((m) => m.title.toLowerCase() === name.toLowerCase())) return null;
  return name;
}

export function matchCommands(cmds: Command[], query: string): Command[] {
  const needle = query.toLowerCase().trim();
  return cmds.filter((c) => c.name.toLowerCase().includes(needle));
}
```

- [ ] **Step 4: `Palette.svelte` uses it**

Replace `score` and the `items` derivation:

```ts
  import { createName, matchCommands, matchNotes } from "../lib/palette";

  const items = $derived.by((): Item[] => {
    if (app.palette === "commands") {
      return matchCommands(allCommands(), q).map((c) => ({ label: c.name, detail: c.hotkey, run: c.run }));
    }
    const matches = matchNotes(app.titles, q);
    const notes: Item[] = matches.map((m) => ({ label: m.title, detail: m.path, run: () => app.openNote(m.path) }));
    const name = createName(matches, q);
    if (name) {
      notes.push({
        label: `Create "${name}"`,
        detail: `${name}.md`,
        run: async () => {
          await createNote(`${name}.md`);
          await app.refresh();
          await app.openNote(`${name}.md`);
        },
      });
    }
    return notes;
  });
```

The `{#each}` stays keyed by `it.label + it.detail` (Task 6).

- [ ] **Step 5: Check**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all pass.

- [ ] **Step 6: Commit**

```bash
git add ui
git commit -m "test(ui): palette ranking and command hotkeys"
```

### Task 10: Typed property editors

Gap 7. Properties show as Obsidian shows them: lists as removable chips with an input to add one, checkboxes, number, date and date-time inputs, text otherwise, and a remove button per property. The kind is read from the value with the index's rule.

**Files:**
- Create: `ui/src/lib/properties.ts`, `ui/src/lib/properties.test.ts`
- Modify: `ui/src/components/Properties.svelte`, `ui/src/app.css`

**Interfaces:**
- Consumes: `app.activeDoc`, `app.save`, `api.setProperty`, `api.properties`.
- Produces: `propKind(v: unknown): "checkbox" | "number" | "date" | "datetime" | "list" | "text"`, `parseValue(raw: string): unknown`, `addItem(list: unknown[], raw: string): unknown[]`, `removeItem(list: unknown[], index: number): unknown[]`.

- [ ] **Step 1: Write the failing test**

`ui/src/lib/properties.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { addItem, parseValue, propKind, removeItem } from "./properties";

describe("properties", () => {
  it("reads the kind from the value, as the index types it", () => {
    expect(propKind(true)).toBe("checkbox");
    expect(propKind(3.5)).toBe("number");
    expect(propKind(["a"])).toBe("list");
    expect(propKind("2026-09-12")).toBe("date");
    expect(propKind("2026-09-12T10:30")).toBe("datetime");
    expect(propKind("hello")).toBe("text");
    expect(propKind(null)).toBe("text");
  });

  it("parses a typed value", () => {
    expect(parseValue("")).toBeNull();
    expect(parseValue("true")).toBe(true);
    expect(parseValue("-2.5")).toBe(-2.5);
    expect(parseValue('["a", "b"]')).toEqual(["a", "b"]);
    expect(parseValue("[not json")).toBe("[not json");
    expect(parseValue("words")).toBe("words");
  });

  it("adds and removes list items", () => {
    expect(addItem(["a"], "  b ")).toEqual(["a", "b"]);
    expect(addItem(["a"], "a")).toEqual(["a"]);
    expect(addItem(["a"], "   ")).toEqual(["a"]);
    expect(removeItem(["a", "b", "c"], 1)).toEqual(["a", "c"]);
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd ui && pnpm test properties`
Expected: FAIL, cannot resolve `./properties`.

- [ ] **Step 3: `ui/src/lib/properties.ts`**

```ts
export type PropKind = "checkbox" | "number" | "date" | "datetime" | "list" | "text";

/** The editor a value gets; the same rule as `value_type` in the index. */
export function propKind(v: unknown): PropKind {
  if (typeof v === "boolean") return "checkbox";
  if (typeof v === "number") return "number";
  if (Array.isArray(v)) return "list";
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}$/.test(v)) return "date";
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(v)) return "datetime";
  return "text";
}

/** A value typed into the new-property row; empty means no value. */
export function parseValue(raw: string): unknown {
  if (raw === "") return null;
  if (raw === "true") return true;
  if (raw === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(raw)) return Number(raw);
  if (raw.startsWith("[")) {
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  return raw;
}

export function addItem(list: unknown[], raw: string): unknown[] {
  const item = raw.trim();
  return item && !list.includes(item) ? [...list, item] : list;
}

export function removeItem(list: unknown[], index: number): unknown[] {
  return list.filter((_, i) => i !== index);
}
```

Run: `cd ui && pnpm test properties`
Expected: PASS.

- [ ] **Step 4: `ui/src/components/Properties.svelte`**

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { addItem, parseValue, propKind, removeItem } from "../lib/properties";

  let props = $state<Record<string, unknown>>({});
  let newKey = $state("");
  $effect(() => {
    const p = app.activeDoc?.path;
    void app.files; // re-run whenever the index changed
    if (p) properties(p).then((r) => (props = r));
    else props = {};
  });

  // null removes the key.
  async function commit(key: string, value: unknown) {
    const doc = app.activeDoc;
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would otherwise conflict with the rewrite
      await setProperty(doc.path, key, value);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  const list = (v: unknown) => v as unknown[];
  const text = (v: unknown) => (v == null ? "" : typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if app.activeDoc}
  <div class="pane-title">Properties</div>
  <div class="props">
    {#each Object.entries(props) as [k, v] (k)}
      {@const kind = propKind(v)}
      <span class="key" title={k}>{k}</span>
      {#if kind === "checkbox"}
        <input type="checkbox" checked={v === true} onchange={(e) => commit(k, e.currentTarget.checked)} />
      {:else if kind === "number"}
        <input type="number" value={v as number} onchange={(e) => commit(k, e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
      {:else if kind === "date"}
        <input type="date" value={v as string} onchange={(e) => commit(k, e.currentTarget.value || null)} />
      {:else if kind === "datetime"}
        <input type="datetime-local" value={(v as string).slice(0, 16)} onchange={(e) => commit(k, e.currentTarget.value || null)} />
      {:else if kind === "list"}
        <div class="chips">
          {#each list(v) as item, i (i)}
            <span class="chip">{text(item)}<button title="Remove" onclick={() => commit(k, removeItem(list(v), i))}>×</button></span>
          {/each}
          <input
            class="chip-input"
            placeholder="Add"
            onkeydown={(e) => {
              if (e.key !== "Enter") return;
              void commit(k, addItem(list(v), e.currentTarget.value));
              e.currentTarget.value = "";
            }}
          />
        </div>
      {:else}
        <input value={text(v)} onchange={(e) => commit(k, e.currentTarget.value)} />
      {/if}
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    {/each}
    <input placeholder="New property" bind:value={newKey} />
    <input
      placeholder="Value"
      onchange={(e) => {
        if (!newKey) return;
        void commit(newKey, parseValue(e.currentTarget.value) ?? "");
        newKey = "";
        e.currentTarget.value = "";
      }}
    />
    <span></span>
  </div>
{/if}
```

`ui/src/app.css`, replace the two `.props` rules:

```css
.props { padding: 4px 12px; display: grid; grid-template-columns: minmax(60px, max-content) 1fr auto; gap: 4px 8px; font-size: 13px; align-items: center; }
.props .key { color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.props input { padding: 2px 6px; min-width: 0; }
.props input[type="checkbox"] { justify-self: start; accent-color: var(--accent); }
.props .remove { color: var(--fg-muted); opacity: .5; }
.props .remove:hover { opacity: 1; }
.chips { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; min-width: 0; }
.chip { display: inline-flex; align-items: center; background: var(--accent-bg); border-radius: 10px; padding: 0 2px 0 8px; font-size: 12px; }
.chip button { color: var(--fg-muted); padding: 0 4px; }
.chip-input { width: 64px; border: none !important; background: transparent !important; padding: 0 4px !important; }
```

A new property with an empty value is written as `""`, since `null` removes the key.

- [ ] **Step 5: Check**

Run: `cd ui && pnpm check && pnpm test`
Expected: 0 errors, all pass.

- [ ] **Step 6: Verify in the running app**

Scratch note `Props.md`:

```markdown
---
tags: [project, urgent]
done: true
count: 3
due: 2026-09-12
start: 2026-09-12T09:30
owner: sam
---
Body
```

Seed it as the only tab, launch, screenshot the right sidebar in the light and the dark theme (`"theme": "dark"` in `app.json`). Expected: two chips with ×, a checked box, a number field, a date picker, a date-time field, a text field, a × per row; nothing shows raw JSON.

- [ ] **Step 7: Commit**

```bash
git add ui
git commit -m "feat(ui): typed property editors with list chips"
```

### Task 11: Smoke checklist, handoff, full verification

Record what changed where a human or the next session looks for it.

**Files:**
- Modify: `docs/smoke.md`, `docs/superpowers/plans/2026-09-12-handoff.md`

- [ ] **Step 1: Smoke checklist**

In `docs/smoke.md`, insert before the last line and renumber so `cargo test …` stays last:

```markdown
17. Split right and split down from the palette: the note opens beside itself, typing in one pane shows in the other, dividers drag, and the layout survives a restart.
18. Closing the last tab of a pane closes the pane; the last pane stays.
19. Click an image and a PDF in the explorer: both preview; *Open in default app* opens them.
20. `[[Note#Heading]]` and `[[Note#^block]]` open the note scrolled to the line in live and reading mode.
21. List properties show as chips; adding and removing one rewrites the frontmatter.
22. Corrupt the index file while the app is closed: the next open says it rebuilt the index.
```

- [ ] **Step 2: Handoff**

In `docs/superpowers/plans/2026-09-12-handoff.md`, under *Against the spec*, replace the numbered gap list with what this plan closed, the measured open times from Task 3 (debug and release), anything a screenshot could not show (clicks, drags, the PDF on WebKitGTK, the watcher fallback), and move the gaps plan into *Plans*. Keep *Deferred by the foundation plan* as it is; those items belong to the graph-and-bases plan.

- [ ] **Step 3: Full verification**

Run:

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
cd ui && pnpm check && pnpm test
```

Expected: every command succeeds. Push the branch and confirm CI is green with `gh run watch`.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: smoke lines and handoff after the gaps plan"
```
