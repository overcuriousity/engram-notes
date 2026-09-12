# Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A working markdown note taker: open a vault folder, browse and edit notes with live preview, follow and autocomplete wikilinks, see backlinks, search full text, with a native window on Linux, Windows and macOS.

**Architecture:** A Cargo workspace. `core` is a plain library holding the parser, vault I/O, the SQLite index and config, fully tested without a window. `src-tauri` is a thin Tauri 2 shell exposing `core` as commands and pushing index and file-change events. `ui` is a Svelte 5 frontend with a CodeMirror 6 editor. Files are the truth; the index is derived.

**Tech Stack:** Rust 2024 (stable 1.95), rusqlite 0.40 (bundled), pulldown-cmark 0.13, serde_yaml_ng 0.10, notify-debouncer-full 0.7, Tauri 2.11, Svelte 5.57, Vite 8, TypeScript 5.9, CodeMirror 6, markdown-it 15, pnpm 11.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`. This plan implements the sections *The vault*, *The index*, *The editor and UI*, *Error handling*, *Testing* and *Repository*. Two further plans follow once this one has landed: `graph-and-bases` (graph view, `.base` files, folder rename and delete, image embeds through the asset protocol, inline rename in the explorer), then `semantic-search-and-memory` (embedding, fused search, activation, association, the Related pane, semantic edges, link suggestions). Those plans are written after this one is executed, because their tasks build on the types this one produces.

## Global Constraints

- Licence header not required per file; `LICENSE` is GPL-3.0-only and `Cargo.toml` says `license = "GPL-3.0-only"`.
- Rust edition 2024, stable toolchain. `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings` clean at every commit.
- `core` has no dependency on `tauri`. All logic and all tests live there.
- Comment policy from `AGENTS.md`: short, why not what. No comment restating a name.
- Relative vault paths always use forward slashes, are relative to the vault root, and never start with `/` or `./`.
- Hidden directories and files (name starting with `.`) are never indexed. `.engram-notes/` is the vault config folder.
- Every note write is atomic: temp file beside the target, then rename.
- Link resolution order: exact relative path, then basename anywhere in the vault; `.md` implied; case-insensitive; ambiguous basename resolves to the shortest path.
- Commit messages: conventional prefix, imperative subject under 72 characters.
- Frontend never touches the filesystem; it only calls Tauri commands.
- Package manager for `ui` is pnpm. TypeScript stays on 5.x (7.x is the new compiler and svelte-check does not accept it yet).

## Machine prerequisites

Tauri on Fedora needs system packages this machine does not have yet. Run once, as the user, before Task 9:

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel libappindicator-gtk3-devel
```

On Ubuntu CI runners the equivalent is installed in the workflow (Task 1). Tasks 1 to 8 need only Rust.

## File structure

```
Cargo.toml                      workspace: members core, src-tauri
rust-toolchain.toml             stable
rustfmt.toml                    defaults, edition 2024
.github/workflows/ci.yml        fmt, clippy, test, svelte-check, vitest
.github/workflows/release.yml   tauri-action on v* tags
core/Cargo.toml
core/src/lib.rs                 pub mod error, parse, vault, index, config, watch
core/src/error.rs               Error enum, Result alias
core/src/parse.rs               markdown -> ParsedNote (frontmatter, links, tags, headings)
core/src/vault.rs               Vault: walk, read, atomic write, create, delete, rename
core/src/rename.rs              link rewriting on rename (pure) + rename_note orchestration
core/src/index/mod.rs           Index struct, open, schema
core/src/index/schema.sql       DDL
core/src/index/rebuild.rs       rebuild from vault, update one file, remove
core/src/index/resolve.rs       link resolution
core/src/index/query.rs         notes, backlinks, outgoing, unresolved, tags, properties
core/src/index/fts.rs           full-text search
core/src/config.rs              AppConfig, workspace.json, load/save under .engram-notes/
core/src/watch.rs               notify wrapper -> Change events
core/tests/fixtures/vault/      a small vault used by integration-style tests
src-tauri/Cargo.toml
src-tauri/build.rs
src-tauri/tauri.conf.json
src-tauri/capabilities/default.json
src-tauri/src/main.rs
src-tauri/src/lib.rs            run(): builder, state, watcher thread
src-tauri/src/state.rs          AppState = Mutex<Option<Open>>; Open { vault, index, config }
src-tauri/src/commands.rs       all #[tauri::command] fns
src-tauri/src/error.rs          engram_core::Error -> CommandError {code, message}
ui/package.json
ui/vite.config.ts
ui/tsconfig.json
ui/svelte.config.js
ui/index.html
ui/src/main.ts
ui/src/App.svelte               layout: left sidebar, centre, right sidebar, status bar
ui/src/app.css                  CSS variables, light and dark
ui/src/lib/api.ts               typed invoke wrappers and event subscriptions
ui/src/lib/state.svelte.ts      open vault, tree, tabs, active tab
ui/src/lib/wikilink.ts          parseWikilink, findWikilinks (pure, tested)
ui/src/lib/render.ts            markdown-it with wikilink, tag, callout, checkbox plugins (tested)
ui/src/editor/livePreview.ts    CodeMirror decorations for live preview
ui/src/editor/completions.ts    [[ and # autocompletion
ui/src/editor/theme.ts          editor colours from CSS variables
ui/src/components/Explorer.svelte
ui/src/components/Tabs.svelte
ui/src/components/NoteView.svelte   editor / reading toggle, autosave, conflict bar
ui/src/components/Editor.svelte
ui/src/components/Reading.svelte
ui/src/components/Backlinks.svelte
ui/src/components/Outgoing.svelte
ui/src/components/Properties.svelte
ui/src/components/Search.svelte
ui/src/components/Palette.svelte    quick switcher and command palette (one component, two modes)
ui/src/components/StatusBar.svelte
ui/src/components/VaultPicker.svelte
ui/src/lib/wikilink.test.ts
ui/src/lib/render.test.ts
docs/smoke.md                   manual checklist
```

---

### Task 1: Workspace, core skeleton, CI

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `core/Cargo.toml`, `core/src/lib.rs`, `core/src/error.rs`, `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `engram_core::Error` enum, `core::Result<T>`.

- [ ] **Step 1: Workspace manifest**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["core", "src-tauri"]
default-members = ["core"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-only"
repository = "https://github.com/overcuriousity/engram-notes"
rust-version = "1.95"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
thiserror = "2"
```

`default-members = ["core"]` lets `cargo test` run without the Tauri system libraries until Task 9 exists. Since `src-tauri` does not exist yet, temporarily write `members = ["core"]` and add `src-tauri` in Task 9.

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`rustfmt.toml`:

```toml
edition = "2024"
```

- [ ] **Step 2: core crate**

`core/Cargo.toml`:

```toml
[package]
name = "engram-notes-core"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[lib]
name = "core"
path = "src/lib.rs"

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile = "3"
```

`core/src/lib.rs`:

```rust
//! engram-notes core: parser, vault, index, config. No window, no Tauri.

pub mod error;

pub use error::{Error, Result};
```

`core/src/error.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("index error: {0}")]
    Index(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    Exists(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }
}
```

- [ ] **Step 3: Verify it builds and lints**

Run: `cargo build && cargo clippy --all-targets -- -D warnings && cargo fmt --all --check`
Expected: success, no warnings.

- [ ] **Step 4: CI workflow**

`.github/workflows/ci.yml`:

```yaml
name: ci

on:
  push:
  pull_request:

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

permissions:
  contents: read

jobs:
  rust:
    name: fmt, clippy, tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Tauri system libraries
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets --locked -- -D warnings
      - run: cargo test --workspace --locked

  ui:
    name: svelte-check, vitest
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: ui
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          version: 11
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: ui/pnpm-lock.yaml
      - run: pnpm install --frozen-lockfile
      - run: pnpm check
      - run: pnpm test

  audit:
    name: security advisories
    runs-on: ubuntu-latest
    permissions:
      contents: read
      checks: write
    steps:
      - uses: actions/checkout@v5
      - uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

The `ui` job will fail until Task 10 creates `ui/`. That is acceptable for the commits in between; do not add `continue-on-error`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml core .github/workflows/ci.yml
git commit -m "chore: cargo workspace, core crate skeleton, ci"
```

---

### Task 2: Parser

**Files:**
- Create: `core/src/parse.rs`
- Modify: `core/src/lib.rs` (add `pub mod parse;`), `core/Cargo.toml` (deps)

**Interfaces:**
- Produces:

```rust
pub struct ParsedNote {
    pub title: Option<String>,          // frontmatter `title` if a string
    pub frontmatter: serde_json::Map<String, serde_json::Value>,
    pub body: String,                   // text after the frontmatter fence
    pub body_offset: usize,             // byte offset of body in the source
    pub links: Vec<Link>,
    pub tags: Vec<String>,              // lowercased, deduplicated, sorted
    pub headings: Vec<Heading>,
}
pub struct Link {
    pub target: String,                 // as written, trimmed; for markdown links percent-decoded
    pub heading: Option<String>,        // after '#'
    pub alias: Option<String>,          // after '|'
    pub kind: LinkKind,                 // Wiki | Markdown | Embed
    pub line: u32,                      // 1-based, in the source
    pub start: usize,                   // byte range in the source, whole link syntax
    pub end: usize,
}
pub struct Heading { pub level: u8, pub text: String, pub line: u32 }
pub fn parse(source: &str) -> ParsedNote
```

`parse` never fails: a bad frontmatter block becomes an empty map and the fence text stays in the body.

- [ ] **Step 1: Add dependencies**

In `core/Cargo.toml` `[dependencies]` add:

```toml
pulldown-cmark = { version = "0.13", default-features = false }
serde_yaml_ng = "0.10"
regex = "1"
```

- [ ] **Step 2: Write the failing tests**

Append to `core/src/parse.rs` (create the file with just the tests module for now):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_split_from_body() {
        let n = parse("---\ntitle: Hello\ntags: [a, B]\n---\n# H\nbody");
        assert_eq!(n.title.as_deref(), Some("Hello"));
        assert_eq!(n.frontmatter["tags"], serde_json::json!(["a", "B"]));
        assert_eq!(n.body, "# H\nbody");
        assert_eq!(&"---\ntitle: Hello\ntags: [a, B]\n---\n# H\nbody"[n.body_offset..], n.body);
    }

    #[test]
    fn broken_frontmatter_stays_in_body() {
        let n = parse("---\n: : not yaml [\n---\ntext");
        assert!(n.frontmatter.is_empty());
        assert!(n.body.starts_with("---"));
    }

    #[test]
    fn wikilink_forms() {
        let n = parse("a [[Note]] b [[Folder/Other|shown]] c [[Third#Sec]] d ![[img.png]]");
        let t: Vec<_> = n.links.iter().map(|l| (l.target.as_str(), l.alias.as_deref(), l.heading.as_deref(), l.kind)).collect();
        assert_eq!(t, vec![
            ("Note", None, None, LinkKind::Wiki),
            ("Folder/Other", Some("shown"), None, LinkKind::Wiki),
            ("Third", None, Some("Sec"), LinkKind::Wiki),
            ("img.png", None, None, LinkKind::Embed),
        ]);
        assert_eq!(&"a [[Note]] b"[n.links[0].start..n.links[0].end], "[[Note]]");
    }

    #[test]
    fn markdown_links_to_notes_only() {
        let n = parse("[x](Other.md) [y](https://example.com) [z](sub/My%20Note.md)");
        let t: Vec<_> = n.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(t, vec!["Other.md", "sub/My Note.md"]);
        assert_eq!(n.links[0].kind, LinkKind::Markdown);
    }

    #[test]
    fn links_in_code_are_not_links() {
        let n = parse("`[[inline]]`\n\n```\n[[fenced]]\n```\n[[real]]");
        let t: Vec<_> = n.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(t, vec!["real"]);
        assert_eq!(n.links[0].line, 6);
    }

    #[test]
    fn tags_from_body_and_frontmatter() {
        let n = parse("---\ntags: [Alpha, beta]\n---\nx #Gamma/sub y #beta `#notatag` z#nope\n```\n#code\n```");
        assert_eq!(n.tags, vec!["alpha", "beta", "gamma/sub"]);
    }

    #[test]
    fn headings_with_lines() {
        let n = parse("---\na: 1\n---\n# One\ntext\n## Two");
        let h: Vec<_> = n.headings.iter().map(|h| (h.level, h.text.as_str(), h.line)).collect();
        assert_eq!(h, vec![(1, "One", 4), (2, "Two", 6)]);
    }

    #[test]
    fn line_numbers_count_frontmatter() {
        let n = parse("---\na: 1\n---\n\n[[L]]");
        assert_eq!(n.links[0].line, 5);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Add `pub mod parse;` to `core/src/lib.rs`. Run: `cargo test -p engram-notes-core parse`
Expected: compile error, `parse` and the types are not defined.

- [ ] **Step 4: Implement**

Prepend to `core/src/parse.rs`:

```rust
//! Markdown to `ParsedNote`. Pure: no I/O, never fails.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Wiki,
    Markdown,
    Embed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub kind: LinkKind,
    pub line: u32,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ParsedNote {
    pub title: Option<String>,
    pub frontmatter: serde_json::Map<String, serde_json::Value>,
    pub body: String,
    pub body_offset: usize,
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
}

static WIKILINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(!?)\[\[([^\[\]\|#]+)(?:#([^\[\]\|]*))?(?:\|([^\[\]]*))?\]\]").unwrap());
// A tag starts a word: preceded by start, whitespace or '('. Letters, digits,
// '_', '-', '/' as Obsidian allows; a tag that is only digits is not a tag.
static TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s(])#([\p{L}\p{N}_/\-]*[\p{L}_/\-][\p{L}\p{N}_/\-]*)").unwrap());

pub fn parse(source: &str) -> ParsedNote {
    let (frontmatter, body_offset) = split_frontmatter(source);
    let body = &source[body_offset..];
    let title = frontmatter.get("title").and_then(|v| v.as_str()).map(str::to_owned);

    let (code_ranges, headings, md_links) = walk_markdown(body, body_offset, source);

    let mut links = Vec::new();
    for c in WIKILINK.captures_iter(body) {
        let m = c.get(0).unwrap();
        let start = body_offset + m.start();
        if in_ranges(start, &code_ranges) {
            continue;
        }
        links.push(Link {
            target: c[2].trim().to_owned(),
            heading: c.get(3).map(|h| h.as_str().trim().to_owned()).filter(|h| !h.is_empty()),
            alias: c.get(4).map(|a| a.as_str().trim().to_owned()).filter(|a| !a.is_empty()),
            kind: if &c[1] == "!" { LinkKind::Embed } else { LinkKind::Wiki },
            line: line_of(source, start),
            start,
            end: body_offset + m.end(),
        });
    }
    links.extend(md_links);
    links.sort_by_key(|l| l.start);

    let mut tags: BTreeSet<String> = BTreeSet::new();
    for c in TAG.captures_iter(body) {
        let m = c.get(1).unwrap();
        if !in_ranges(body_offset + m.start(), &code_ranges) {
            tags.insert(m.as_str().to_lowercase());
        }
    }
    if let Some(fm_tags) = frontmatter.get("tags") {
        match fm_tags {
            serde_json::Value::Array(a) => {
                for v in a {
                    if let Some(s) = v.as_str() {
                        tags.insert(s.trim_start_matches('#').to_lowercase());
                    }
                }
            }
            serde_json::Value::String(s) => {
                for t in s.split([',', ' ']).filter(|t| !t.is_empty()) {
                    tags.insert(t.trim_start_matches('#').to_lowercase());
                }
            }
            _ => {}
        }
    }

    ParsedNote {
        title,
        frontmatter,
        body: body.to_owned(),
        body_offset,
        links,
        tags: tags.into_iter().collect(),
        headings,
    }
}

fn split_frontmatter(source: &str) -> (serde_json::Map<String, serde_json::Value>, usize) {
    let empty = serde_json::Map::new();
    let Some(rest) = source.strip_prefix("---\n").or_else(|| source.strip_prefix("---\r\n")) else {
        return (empty, 0);
    };
    let Some(end) = find_fence_end(rest) else {
        return (empty, 0);
    };
    let yaml = &rest[..end.0];
    let body_offset = source.len() - rest.len() + end.1;
    match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml) {
        Ok(v) => match serde_json::to_value(v) {
            Ok(serde_json::Value::Object(m)) => (m, body_offset),
            _ => (empty, 0),
        },
        Err(_) => (empty, 0),
    }
}

/// Byte offset of the closing fence line in `rest`, and the offset just past
/// it (past its newline), or `None` when there is no closing fence.
fn find_fence_end(rest: &str) -> Option<(usize, usize)> {
    let mut pos = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            return Some((pos, pos + line.len()));
        }
        pos += line.len();
    }
    None
}

/// Code spans and blocks as absolute byte ranges, headings, and markdown
/// links pointing at vault files.
fn walk_markdown(body: &str, body_offset: usize, source: &str) -> (Vec<(usize, usize)>, Vec<Heading>, Vec<Link>) {
    let mut code = Vec::new();
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut heading: Option<(u8, usize, String)> = None;
    let mut link: Option<(String, usize)> = None;
    let parser = Parser::new_ext(body, Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH);
    for (event, range) in parser.into_offset_iter() {
        let abs = (body_offset + range.start, body_offset + range.end);
        match event {
            Event::Code(ref t) => {
                code.push(abs);
                if let Some(h) = heading.as_mut() {
                    h.2.push_str(t);
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code.push(abs),
            Event::Start(Tag::Heading { level, .. }) => heading = Some((level as u8, abs.0, String::new())),
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, text)) = heading.take() {
                    headings.push(Heading { level, text: text.trim().to_owned(), line: line_of(source, start) });
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => link = Some((dest_url.to_string(), abs.0)),
            Event::End(TagEnd::Link) => {
                if let Some((dest, start)) = link.take()
                    && is_vault_target(&dest)
                {
                    links.push(Link {
                        target: percent_decode(&dest),
                        heading: None,
                        alias: None,
                        kind: LinkKind::Markdown,
                        line: line_of(source, start),
                        start,
                        end: abs.1,
                    });
                }
            }
            Event::Text(t) if heading.is_some() => {
                heading.as_mut().unwrap().2.push_str(&t);
            }
            _ => {}
        }
    }
    (code, headings, links)
}

fn is_vault_target(dest: &str) -> bool {
    !dest.contains("://") && !dest.starts_with("mailto:") && !dest.starts_with('#') && dest.to_lowercase().ends_with(".md")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn in_ranges(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| pos >= *s && pos < *e)
}

fn line_of(source: &str, offset: usize) -> u32 {
    source[..offset].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p engram-notes-core parse`
Expected: 8 passed. If `tags_from_body_and_frontmatter` fails on `z#nope`, the regex's leading `(?:^|[\s(])` handles it; check the inline-code range covers the backtick span from `Event::Code`.

- [ ] **Step 6: Lint and commit**

Run: `cargo clippy --all-targets -- -D warnings && cargo fmt --all`

```bash
git add core
git commit -m "feat(core): markdown parser for frontmatter, links, tags and headings"
```

---

### Task 3: Vault I/O

**Files:**
- Create: `core/src/vault.rs`
- Modify: `core/src/lib.rs`, `core/Cargo.toml`

**Interfaces:**
- Produces:

```rust
pub struct Vault { root: PathBuf }
pub struct FileEntry { pub path: String, pub mtime_ms: i64, pub size: u64, pub is_markdown: bool }
impl Vault {
    pub fn open(root: impl AsRef<Path>) -> Result<Vault>;   // must be an existing dir; creates .engram-notes/
    pub fn root(&self) -> &Path;
    pub fn config_dir(&self) -> PathBuf;                    // root/.engram-notes
    pub fn abs(&self, rel: &str) -> PathBuf;
    pub fn walk(&self) -> Result<Vec<FileEntry>>;           // sorted by path, hidden skipped
    pub fn stat(&self, rel: &str) -> Result<Option<FileEntry>>;
    pub fn read(&self, rel: &str) -> Result<String>;
    pub fn write(&self, rel: &str, text: &str) -> Result<()>;   // atomic, creates parents
    pub fn create(&self, rel: &str, text: &str) -> Result<()>;  // Error::Exists if present
    pub fn delete(&self, rel: &str) -> Result<()>;              // to the OS trash
    pub fn rename(&self, from: &str, to: &str) -> Result<()>;   // Error::Exists if target present
}
pub fn normalize_rel(path: &str) -> String;   // backslashes to '/', strip leading "./" and "/"
```

- [ ] **Step 1: Dependencies**

`core/Cargo.toml` `[dependencies]` add:

```toml
walkdir = "2"
trash = "5"
```

- [ ] **Step 2: Failing tests**

`core/src/vault.rs` tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> (tempfile::TempDir, Vault) {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        (d, v)
    }

    #[test]
    fn open_creates_config_dir_and_rejects_missing() {
        let (d, v) = tmp();
        assert!(d.path().join(".engram-notes").is_dir());
        assert_eq!(v.root(), d.path());
        assert!(Vault::open(d.path().join("nope")).is_err());
    }

    #[test]
    fn walk_skips_hidden_and_sorts() {
        let (d, v) = tmp();
        fs::create_dir_all(d.path().join("b/.git")).unwrap();
        fs::write(d.path().join("b/.git/x.md"), "").unwrap();
        fs::write(d.path().join("b/two.md"), "2").unwrap();
        fs::write(d.path().join("a.md"), "1").unwrap();
        fs::write(d.path().join("pic.png"), "p").unwrap();
        fs::write(d.path().join(".hidden.md"), "").unwrap();
        let e: Vec<_> = v.walk().unwrap().into_iter().map(|e| (e.path, e.is_markdown)).collect();
        assert_eq!(e, vec![("a.md".into(), true), ("b/two.md".into(), true), ("pic.png".into(), false)]);
    }

    #[test]
    fn write_is_atomic_and_creates_parents() {
        let (d, v) = tmp();
        v.write("deep/er/note.md", "hi").unwrap();
        assert_eq!(v.read("deep/er/note.md").unwrap(), "hi");
        let leftovers: Vec<_> = fs::read_dir(d.path().join("deep/er")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 1);
    }

    #[test]
    fn create_refuses_existing() {
        let (_d, v) = tmp();
        v.create("n.md", "").unwrap();
        assert!(matches!(v.create("n.md", ""), Err(Error::Exists(_))));
    }

    #[test]
    fn rename_moves_and_refuses_overwrite() {
        let (_d, v) = tmp();
        v.create("a.md", "x").unwrap();
        v.create("c.md", "y").unwrap();
        v.rename("a.md", "sub/b.md").unwrap();
        assert_eq!(v.read("sub/b.md").unwrap(), "x");
        assert!(v.stat("a.md").unwrap().is_none());
        assert!(matches!(v.rename("sub/b.md", "c.md"), Err(Error::Exists(_))));
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_rel("./a\\b/c.md"), "a/b/c.md");
        assert_eq!(normalize_rel("/x.md"), "x.md");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Add `pub mod vault;` to `lib.rs`. Run: `cargo test -p engram-notes-core vault`
Expected: compile error.

- [ ] **Step 4: Implement**

Prepend to `core/src/vault.rs`:

```rust
//! The folder of files. Paths in and out are vault-relative with '/'.

use crate::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const CONFIG_DIR: &str = ".engram-notes";

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileEntry {
    pub path: String,
    pub mtime_ms: i64,
    pub size: u64,
    pub is_markdown: bool,
}

pub fn normalize_rel(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.trim_start_matches("./").trim_start_matches('/');
    p.to_owned()
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

impl Vault {
    pub fn open(root: impl AsRef<Path>) -> Result<Vault> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(Error::NotFound(root.display().to_string()));
        }
        let root = root.canonicalize().map_err(|e| Error::io(root, e))?;
        let cfg = root.join(CONFIG_DIR);
        fs::create_dir_all(&cfg).map_err(|e| Error::io(&cfg, e))?;
        Ok(Vault { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join(CONFIG_DIR)
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in normalize_rel(rel).split('/').filter(|s| !s.is_empty() && *s != "..") {
            p.push(part);
        }
        p
    }

    pub fn walk(&self) -> Result<Vec<FileEntry>> {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));
        for entry in walker {
            let entry = entry.map_err(|e| Error::io(&self.root, e.into()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry.path().strip_prefix(&self.root).unwrap();
            let path = rel.to_string_lossy().replace('\\', "/");
            let md = entry.metadata().map_err(|e| Error::io(entry.path(), e.into()))?;
            out.push(FileEntry { path: path.clone(), mtime_ms: mtime_ms(&md), size: md.len(), is_markdown: path.to_lowercase().ends_with(".md") });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    pub fn stat(&self, rel: &str) -> Result<Option<FileEntry>> {
        let abs = self.abs(rel);
        match fs::metadata(&abs) {
            Ok(md) if md.is_file() => Ok(Some(FileEntry {
                path: normalize_rel(rel),
                mtime_ms: mtime_ms(&md),
                size: md.len(),
                is_markdown: rel.to_lowercase().ends_with(".md"),
            })),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io(abs, e)),
        }
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        let abs = self.abs(rel);
        fs::read_to_string(&abs).map_err(|e| Error::io(abs, e))
    }

    pub fn write(&self, rel: &str, text: &str) -> Result<()> {
        let abs = self.abs(rel);
        let dir = abs.parent().ok_or_else(|| Error::NotFound(rel.into()))?;
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        let tmp = dir.join(format!(".{}.tmp", abs.file_name().unwrap().to_string_lossy()));
        fs::write(&tmp, text).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &abs).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::io(&abs, e)
        })
    }

    pub fn create(&self, rel: &str, text: &str) -> Result<()> {
        if self.abs(rel).exists() {
            return Err(Error::Exists(normalize_rel(rel)));
        }
        self.write(rel, text)
    }

    pub fn delete(&self, rel: &str) -> Result<()> {
        let abs = self.abs(rel);
        trash::delete(&abs).map_err(|e| Error::io(&abs, std::io::Error::other(e)))
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let src = self.abs(from);
        let dst = self.abs(to);
        if dst.exists() {
            return Err(Error::Exists(normalize_rel(to)));
        }
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        fs::rename(&src, &dst).map_err(|e| Error::io(src, e))
    }
}

fn mtime_ms(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
```

`rename` on Windows fails when moving across volumes; the vault is one volume, so `fs::rename` is right.

- [ ] **Step 5: Verify, lint, commit**

Run: `cargo test -p engram-notes-core vault && cargo clippy --all-targets -- -D warnings && cargo fmt --all`
Expected: 6 passed.

```bash
git add core
git commit -m "feat(core): vault walking, atomic writes, create, rename, trash"
```

---

### Task 4: Index schema, rebuild and update

**Files:**
- Create: `core/src/index/mod.rs`, `core/src/index/schema.sql`, `core/src/index/rebuild.rs`
- Modify: `core/src/lib.rs`, `core/Cargo.toml`

**Interfaces:**
- Consumes: `Vault::walk/read/stat`, `parse::parse`.
- Produces:

```rust
pub struct Index { conn: rusqlite::Connection }
pub struct RebuildStats { pub added: usize, pub updated: usize, pub removed: usize, pub unchanged: usize }
impl Index {
    pub fn open(path: &Path) -> Result<Index>;         // creates parents; wrong schema version -> drop and recreate
    pub fn open_in_memory() -> Result<Index>;
    pub fn rebuild(&mut self, vault: &Vault) -> Result<RebuildStats>;
    pub fn update_file(&mut self, vault: &Vault, rel: &str) -> Result<bool>;  // true if the note changed; removes when gone
    pub fn remove_file(&mut self, rel: &str) -> Result<()>;
    pub fn conn(&self) -> &rusqlite::Connection;       // for the query modules in this crate
}
```

- [ ] **Step 1: Dependencies**

`core/Cargo.toml` add:

```toml
rusqlite = { version = "0.40", features = ["bundled"] }
sha2 = "0.11"
hex = "0.4"
```

- [ ] **Step 2: Schema**

`core/src/index/schema.sql`:

```sql
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  path TEXT PRIMARY KEY,
  path_lower TEXT NOT NULL,
  stem_lower TEXT NOT NULL,
  title TEXT NOT NULL,
  mtime_ms INTEGER NOT NULL,
  size INTEGER NOT NULL,
  hash TEXT NOT NULL,
  frontmatter TEXT NOT NULL,
  body TEXT NOT NULL,
  body_line INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS notes_stem ON notes(stem_lower);
CREATE INDEX IF NOT EXISTS notes_path_lower ON notes(path_lower);

CREATE TABLE IF NOT EXISTS links (
  src_path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  target_raw TEXT NOT NULL,
  target_path TEXT,
  kind TEXT NOT NULL,
  heading TEXT,
  alias TEXT,
  line INTEGER NOT NULL,
  start INTEGER NOT NULL,
  "end" INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS links_src ON links(src_path);
CREATE INDEX IF NOT EXISTS links_target ON links(target_path);

CREATE TABLE IF NOT EXISTS tags (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  PRIMARY KEY (path, tag)
);
CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);

CREATE TABLE IF NOT EXISTS properties (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  value_type TEXT NOT NULL,
  PRIMARY KEY (path, key)
);
CREATE INDEX IF NOT EXISTS properties_key ON properties(key);

CREATE TABLE IF NOT EXISTS headings (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  level INTEGER NOT NULL,
  text TEXT NOT NULL,
  line INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  title, body, path UNINDEXED,
  content='notes', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts(rowid, title, body, path) VALUES (new.rowid, new.title, new.body, new.path);
END;
CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body, path) VALUES ('delete', old.rowid, old.title, old.body, old.path);
END;
CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body, path) VALUES ('delete', old.rowid, old.title, old.body, old.path);
  INSERT INTO notes_fts(rowid, title, body, path) VALUES (new.rowid, new.title, new.body, new.path);
END;
```

- [ ] **Step 3: Failing tests**

`core/src/index/rebuild.rs` tests module:

```rust
#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn vault() -> (tempfile::TempDir, Vault) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("A.md"), "---\ntitle: Alpha\nstatus: open\n---\n# A\nlinks [[B]] and [[Missing]] #t1").unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::write(d.path().join("sub/B.md"), "back to [[A]]").unwrap();
        fs::write(d.path().join("pic.png"), "x").unwrap();
        let v = Vault::open(d.path()).unwrap();
        (d, v)
    }

    fn count(ix: &Index, sql: &str) -> i64 {
        ix.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn rebuild_indexes_markdown_only() {
        let (_d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (2, 0, 0, 0));
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 2);
        assert_eq!(count(&ix, "SELECT count(*) FROM links"), 3);
        assert_eq!(count(&ix, "SELECT count(*) FROM tags WHERE tag='t1'"), 1);
        assert_eq!(count(&ix, "SELECT count(*) FROM properties WHERE key='status' AND value_json='\"open\"'"), 1);
        let title: String = ix.conn().query_row("SELECT title FROM notes WHERE path='A.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "Alpha");
        let title: String = ix.conn().query_row("SELECT title FROM notes WHERE path='sub/B.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "B");
    }

    #[test]
    fn second_rebuild_is_unchanged_and_detects_edits_and_removals() {
        let (d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (0, 0, 0, 2));
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(d.path().join("sub/B.md"), "changed, no links").unwrap();
        fs::remove_file(d.path().join("A.md")).unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (0, 1, 1, 0));
        assert_eq!(count(&ix, "SELECT count(*) FROM links"), 0);
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 1);
    }

    #[test]
    fn update_file_handles_change_and_disappearance() {
        let (d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        assert!(!ix.update_file(&v, "A.md").unwrap());
        fs::write(d.path().join("A.md"), "new body").unwrap();
        assert!(ix.update_file(&v, "A.md").unwrap());
        assert_eq!(count(&ix, "SELECT count(*) FROM links WHERE src_path='A.md'"), 0);
        fs::remove_file(d.path().join("A.md")).unwrap();
        assert!(ix.update_file(&v, "A.md").unwrap());
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 1);
    }

    #[test]
    fn schema_version_mismatch_rebuilds_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ix.db");
        {
            let ix = Index::open(&p).unwrap();
            ix.conn().execute("UPDATE meta SET value='0' WHERE key='schema'", []).unwrap();
            ix.conn().execute("INSERT INTO notes VALUES ('x.md','x.md','x','x',0,0,'','{}','',1)", []).unwrap();
        }
        let ix = Index::open(&p).unwrap();
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 0);
    }
}
```

- [ ] **Step 4: Run to verify failure**

Add `pub mod index;` to `lib.rs`. Run: `cargo test -p engram-notes-core index`
Expected: compile error.

- [ ] **Step 5: Implement `index/mod.rs`**

```rust
//! The derived view of a vault. Rebuildable from the files at any time.

pub mod rebuild;

use crate::{Error, Result};
use rusqlite::Connection;
use std::path::Path;

pub const SCHEMA_VERSION: &str = "1";
const SCHEMA: &str = include_str!("schema.sql");

pub struct Index {
    conn: Connection,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RebuildStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

impl Index {
    pub fn open(path: &Path) -> Result<Index> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        let conn = Connection::open(path)?;
        let mut ix = Index { conn };
        ix.prepare()?;
        Ok(ix)
    }

    pub fn open_in_memory() -> Result<Index> {
        let mut ix = Index { conn: Connection::open_in_memory()? };
        ix.prepare()?;
        Ok(ix)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn prepare(&mut self) -> Result<()> {
        self.conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
        let has_meta: bool = self
            .conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='meta'", [], |r| r.get::<_, i64>(0))?
            > 0;
        if has_meta {
            let version: Option<String> = self
                .conn
                .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| r.get(0))
                .ok();
            if version.as_deref() != Some(SCHEMA_VERSION) {
                self.drop_all()?;
            }
        }
        self.conn.execute_batch(SCHEMA)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('schema', ?1)",
            [SCHEMA_VERSION],
        )?;
        Ok(())
    }

    // Files are the truth: a schema change throws the derived state away.
    fn drop_all(&mut self) -> Result<()> {
        let names: Vec<(String, String)> = self
            .conn
            .prepare("SELECT type, name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%'")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        self.conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        for (kind, name) in names {
            self.conn.execute_batch(&format!("DROP {kind} IF EXISTS \"{name}\";"))?;
        }
        self.conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(())
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Index(e.to_string())
    }
}
```

Dropping the FTS virtual table also drops its shadow tables, and the triggers go with `notes`.

- [ ] **Step 6: Implement `index/rebuild.rs`**

```rust
use super::{Index, RebuildStats};
use crate::parse::{ParsedNote, parse};
use crate::vault::{FileEntry, Vault};
use crate::Result;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").or_else(|| name.strip_suffix(".MD")).unwrap_or(name)
}

fn hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

// The source line the body starts on, so a link's line maps into `body`.
fn line_of_body(text: &str, body_offset: usize) -> i64 {
    text[..body_offset].bytes().filter(|b| *b == b'\n').count() as i64 + 1
}

fn value_type(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "checkbox",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(s) => {
            if s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
                "date"
            } else if s.len() >= 16 && s.as_bytes()[4] == b'-' && s.as_bytes()[10] == b'T' {
                "datetime"
            } else {
                "text"
            }
        }
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "object",
    }
}

impl Index {
    pub fn rebuild(&mut self, vault: &Vault) -> Result<RebuildStats> {
        let mut stats = RebuildStats::default();
        let known: HashMap<String, (i64, i64)> = self
            .conn
            .prepare("SELECT path, mtime_ms, size FROM notes")?
            .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<std::result::Result<_, _>>()?;
        let mut seen = std::collections::HashSet::new();
        let tx = self.conn.transaction()?;
        for entry in vault.walk()?.into_iter().filter(|e| e.is_markdown) {
            seen.insert(entry.path.clone());
            match known.get(&entry.path) {
                Some(&(m, s)) if m == entry.mtime_ms && s == entry.size as i64 => stats.unchanged += 1,
                Some(_) => {
                    let text = vault.read(&entry.path)?;
                    write_note(&tx, &entry, &text, &parse(&text))?;
                    stats.updated += 1;
                }
                None => {
                    let text = vault.read(&entry.path)?;
                    write_note(&tx, &entry, &text, &parse(&text))?;
                    stats.added += 1;
                }
            }
        }
        for path in known.keys().filter(|p| !seen.contains(*p)) {
            tx.execute("DELETE FROM notes WHERE path=?1", [path])?;
            stats.removed += 1;
        }
        tx.commit()?;
        self.resolve_all()?;
        Ok(stats)
    }

    pub fn update_file(&mut self, vault: &Vault, rel: &str) -> Result<bool> {
        let Some(entry) = vault.stat(rel)? else {
            let existed = self.conn.execute("DELETE FROM notes WHERE path=?1", [rel])? > 0;
            if existed {
                self.resolve_all()?;
            }
            return Ok(existed);
        };
        if !entry.is_markdown {
            return Ok(false);
        }
        let text = vault.read(rel)?;
        let new_hash = hash(&text);
        let old_hash: Option<String> = self
            .conn
            .query_row("SELECT hash FROM notes WHERE path=?1", [rel], |r| r.get(0))
            .ok();
        if old_hash.as_deref() == Some(new_hash.as_str()) {
            self.conn.execute(
                "UPDATE notes SET mtime_ms=?2, size=?3 WHERE path=?1",
                params![rel, entry.mtime_ms, entry.size as i64],
            )?;
            return Ok(false);
        }
        let tx = self.conn.transaction()?;
        write_note(&tx, &entry, &text, &parse(&text))?;
        tx.commit()?;
        self.resolve_all()?;
        Ok(true)
    }

    pub fn remove_file(&mut self, rel: &str) -> Result<()> {
        self.conn.execute("DELETE FROM notes WHERE path=?1", [rel])?;
        self.resolve_all()
    }
}

fn write_note(tx: &rusqlite::Transaction, entry: &FileEntry, text: &str, note: &ParsedNote) -> Result<()> {
    let title = note.title.clone().unwrap_or_else(|| stem(&entry.path).to_owned());
    tx.execute("DELETE FROM notes WHERE path=?1", [&entry.path])?;
    tx.execute(
        "INSERT INTO notes(path, path_lower, stem_lower, title, mtime_ms, size, hash, frontmatter, body, body_line)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            entry.path,
            entry.path.to_lowercase(),
            stem(&entry.path).to_lowercase(),
            title,
            entry.mtime_ms,
            entry.size as i64,
            hash(text),
            serde_json::Value::Object(note.frontmatter.clone()).to_string(),
            note.body,
            line_of_body(text, note.body_offset),
        ],
    )?;
    let mut ins = tx.prepare_cached(
        "INSERT INTO links(src_path, target_raw, target_path, kind, heading, alias, line, start, \"end\")
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for l in &note.links {
        let kind = match l.kind {
            crate::parse::LinkKind::Wiki => "wiki",
            crate::parse::LinkKind::Markdown => "markdown",
            crate::parse::LinkKind::Embed => "embed",
        };
        ins.execute(params![entry.path, l.target, kind, l.heading, l.alias, l.line, l.start as i64, l.end as i64])?;
    }
    let mut ins = tx.prepare_cached("INSERT INTO tags(path, tag) VALUES (?1, ?2)")?;
    for t in &note.tags {
        ins.execute(params![entry.path, t])?;
    }
    let mut ins = tx.prepare_cached("INSERT INTO properties(path, key, value_json, value_type) VALUES (?1, ?2, ?3, ?4)")?;
    for (k, v) in &note.frontmatter {
        ins.execute(params![entry.path, k, v.to_string(), value_type(v)])?;
    }
    let mut ins = tx.prepare_cached("INSERT INTO headings(path, level, text, line) VALUES (?1, ?2, ?3, ?4)")?;
    for h in &note.headings {
        ins.execute(params![entry.path, h.level, h.text, h.line])?;
    }
    Ok(())
}
```

`resolve_all` comes in Task 5. Until then add a stub in `index/mod.rs` so this compiles:

```rust
impl Index {
    pub fn resolve_all(&mut self) -> Result<()> {
        Ok(())
    }
}
```

- [ ] **Step 7: Verify, lint, commit**

Run: `cargo test -p engram-notes-core index && cargo clippy --all-targets -- -D warnings && cargo fmt --all`
Expected: 4 passed.

```bash
git add core
git commit -m "feat(core): sqlite index with rebuild and single-file update"
```

---

### Task 5: Link resolution and queries

**Files:**
- Create: `core/src/index/resolve.rs`, `core/src/index/query.rs`
- Modify: `core/src/index/mod.rs` (remove the `resolve_all` stub, add `pub mod resolve; pub mod query;`)

**Interfaces:**
- Produces:

```rust
impl Index {
    pub fn resolve_all(&mut self) -> Result<()>;
    pub fn resolve_target(&self, target: &str) -> Result<Option<String>>;  // path of the note a link target names
    pub fn notes(&self) -> Result<Vec<NoteSummary>>;
    pub fn note(&self, path: &str) -> Result<Option<NoteSummary>>;
    pub fn backlinks(&self, path: &str) -> Result<Vec<LinkRow>>;
    pub fn outgoing(&self, path: &str) -> Result<Vec<LinkRow>>;
    pub fn unresolved(&self) -> Result<Vec<Unresolved>>;
    pub fn tags(&self) -> Result<Vec<TagCount>>;
    pub fn properties(&self, path: &str) -> Result<serde_json::Map<String, serde_json::Value>>;
    pub fn titles(&self) -> Result<Vec<(String, String)>>;   // (path, title) for autocompletion
}
pub struct NoteSummary { pub path: String, pub title: String, pub mtime_ms: i64, pub size: i64 }
pub struct LinkRow { pub src_path: String, pub target_raw: String, pub target_path: Option<String>, pub kind: String, pub heading: Option<String>, pub alias: Option<String>, pub line: u32, pub context: String }
pub struct Unresolved { pub target: String, pub count: i64 }
pub struct TagCount { pub tag: String, pub count: i64 }
```

`context` on `LinkRow` is the source line containing the link, trimmed.

- [ ] **Step 1: Failing tests**

`core/src/index/resolve.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("deep/er")).unwrap();
        fs::create_dir_all(d.path().join("other")).unwrap();
        fs::write(d.path().join("Home.md"), "[[Note]] [[deep/Note]] [[other/note]] [[home]] [[Ghost]] [x](deep/er/Note.md) [[Note#Sec|alias]]").unwrap();
        fs::write(d.path().join("deep/Note.md"), "").unwrap();
        fs::write(d.path().join("deep/er/Note.md"), "").unwrap();
        fs::write(d.path().join("other/Note.md"), "[[Home]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    #[test]
    fn resolution_order_and_case() {
        let (_d, ix) = ix();
        assert_eq!(ix.resolve_target("deep/Note").unwrap().as_deref(), Some("deep/Note.md"));
        assert_eq!(ix.resolve_target("other/note").unwrap().as_deref(), Some("other/Note.md"));
        assert_eq!(ix.resolve_target("home").unwrap().as_deref(), Some("Home.md"));
        assert_eq!(ix.resolve_target("deep/er/Note.md").unwrap().as_deref(), Some("deep/er/Note.md"));
        assert_eq!(ix.resolve_target("Ghost").unwrap(), None);
    }

    #[test]
    fn ambiguous_basename_takes_shortest_path() {
        let (_d, ix) = ix();
        assert_eq!(ix.resolve_target("Note").unwrap().as_deref(), Some("deep/Note.md"));
    }

    #[test]
    fn links_are_resolved_after_rebuild() {
        let (_d, ix) = ix();
        let out = ix.outgoing("Home.md").unwrap();
        let t: Vec<_> = out.iter().map(|l| l.target_path.as_deref()).collect();
        assert_eq!(t, vec![Some("deep/Note.md"), Some("deep/Note.md"), Some("other/Note.md"), Some("Home.md"), None, Some("deep/er/Note.md"), Some("deep/Note.md")]);
        assert_eq!(out[6].alias.as_deref(), Some("alias"));
        assert_eq!(out[6].heading.as_deref(), Some("Sec"));
    }

    #[test]
    fn backlinks_and_unresolved() {
        let (_d, ix) = ix();
        let b = ix.backlinks("Home.md").unwrap();
        let src: Vec<_> = b.iter().map(|l| l.src_path.as_str()).collect();
        assert_eq!(src, vec!["Home.md", "other/Note.md"]);
        assert_eq!(b[1].context, "[[Home]]");
        let u = ix.unresolved().unwrap();
        assert_eq!(u.len(), 1);
        assert_eq!((u[0].target.as_str(), u[0].count), ("Ghost", 1));
    }

    #[test]
    fn creating_the_missing_note_resolves_the_link() {
        let (d, mut ix) = ix();
        fs::write(d.path().join("Ghost.md"), "").unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.update_file(&v, "Ghost.md").unwrap();
        assert!(ix.unresolved().unwrap().is_empty());
    }

    #[test]
    fn summaries_tags_properties_titles() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("P.md"), "---\ntitle: Pretty\nn: 3\n---\n#x #y").unwrap();
        fs::write(d.path().join("Q.md"), "#x").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let notes = ix.notes().unwrap();
        assert_eq!(notes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), vec!["Pretty", "Q"]);
        assert_eq!(ix.note("P.md").unwrap().unwrap().title, "Pretty");
        assert!(ix.note("nope.md").unwrap().is_none());
        let tags = ix.tags().unwrap();
        assert_eq!(tags.iter().map(|t| (t.tag.as_str(), t.count)).collect::<Vec<_>>(), vec![("x", 2), ("y", 1)]);
        assert_eq!(ix.properties("P.md").unwrap()["n"], serde_json::json!(3));
        assert_eq!(ix.titles().unwrap(), vec![("P.md".to_string(), "Pretty".to_string()), ("Q.md".to_string(), "Q".to_string())]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engram-notes-core index::resolve`
Expected: compile error on the missing methods and types.

- [ ] **Step 3: Implement `index/resolve.rs`**

```rust
//! Link targets to note paths: exact path, then basename anywhere; `.md`
//! implied; case-insensitive; the shortest path wins a tie.

use super::Index;
use crate::Result;
use rusqlite::OptionalExtension;

impl Index {
    pub fn resolve_target(&self, target: &str) -> Result<Option<String>> {
        let t = crate::vault::normalize_rel(target).to_lowercase();
        let t = t.trim_end_matches('/');
        if t.is_empty() {
            return Ok(None);
        }
        let with_md = if t.ends_with(".md") { t.to_owned() } else { format!("{t}.md") };
        if let Some(p) = self
            .conn
            .query_row("SELECT path FROM notes WHERE path_lower=?1", [&with_md], |r| r.get::<_, String>(0))
            .optional()?
        {
            return Ok(Some(p));
        }
        let stem = with_md.rsplit('/').next().unwrap().trim_end_matches(".md");
        Ok(self
            .conn
            .query_row(
                "SELECT path FROM notes WHERE stem_lower=?1 ORDER BY length(path), path LIMIT 1",
                [stem],
                |r| r.get::<_, String>(0),
            )
            .optional()?)
    }

    pub fn resolve_all(&mut self) -> Result<()> {
        let targets: Vec<String> = self
            .conn
            .prepare("SELECT DISTINCT target_raw FROM links")?
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let tx = self.conn.transaction()?;
        {
            let mut upd = tx.prepare_cached("UPDATE links SET target_path=?2 WHERE target_raw=?1")?;
            for raw in targets {
                let resolved = resolve_in(&tx, &raw)?;
                upd.execute(rusqlite::params![raw, resolved])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn resolve_in(conn: &rusqlite::Connection, target: &str) -> Result<Option<String>> {
    let t = crate::vault::normalize_rel(target).to_lowercase();
    let t = t.trim_end_matches('/');
    if t.is_empty() {
        return Ok(None);
    }
    let with_md = if t.ends_with(".md") { t.to_owned() } else { format!("{t}.md") };
    if let Some(p) = conn
        .query_row("SELECT path FROM notes WHERE path_lower=?1", [&with_md], |r| r.get::<_, String>(0))
        .optional()?
    {
        return Ok(Some(p));
    }
    let stem = with_md.rsplit('/').next().unwrap().trim_end_matches(".md");
    Ok(conn
        .query_row("SELECT path FROM notes WHERE stem_lower=?1 ORDER BY length(path), path LIMIT 1", [stem], |r| r.get::<_, String>(0))
        .optional()?)
}
```

Then make `resolve_target` a one-liner: `resolve_in(&self.conn, target)`. Resolving every distinct target after each change is O(targets) cheap SQLite lookups; a ten-thousand-note vault has a few thousand distinct targets, which is milliseconds.

- [ ] **Step 4: Implement `index/query.rs`**

```rust
use super::Index;
use crate::Result;
use rusqlite::OptionalExtension;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub mtime_ms: i64,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LinkRow {
    pub src_path: String,
    pub target_raw: String,
    pub target_path: Option<String>,
    pub kind: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub line: u32,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unresolved {
    pub target: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

const LINK_COLUMNS: &str = "l.src_path, l.target_raw, l.target_path, l.kind, l.heading, l.alias, l.line, n.body, n.body_line";

// The index stores the body without its frontmatter and the line the body
// starts on, so a link's source line maps onto a body line by subtraction.
fn context_line(body: &str, line: u32, body_line: i64) -> String {
    let idx = (line as i64 - body_line).max(0) as usize;
    body.lines().nth(idx).unwrap_or("").trim().to_owned()
}

fn link_row(r: &rusqlite::Row) -> rusqlite::Result<LinkRow> {
    let body: String = r.get(7)?;
    let line: u32 = r.get(6)?;
    let body_line: i64 = r.get(8)?;
    Ok(LinkRow {
        src_path: r.get(0)?,
        target_raw: r.get(1)?,
        target_path: r.get(2)?,
        kind: r.get(3)?,
        heading: r.get(4)?,
        alias: r.get(5)?,
        line,
        context: context_line(&body, line, body_line),
    })
}
```

Continue `query.rs`:

```rust
impl Index {
    pub fn notes(&self) -> Result<Vec<NoteSummary>> {
        let rows = self
            .conn
            .prepare("SELECT path, title, mtime_ms, size FROM notes ORDER BY path")?
            .query_map([], |r| Ok(NoteSummary { path: r.get(0)?, title: r.get(1)?, mtime_ms: r.get(2)?, size: r.get(3)? }))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn note(&self, path: &str) -> Result<Option<NoteSummary>> {
        Ok(self
            .conn
            .query_row("SELECT path, title, mtime_ms, size FROM notes WHERE path=?1", [path], |r| {
                Ok(NoteSummary { path: r.get(0)?, title: r.get(1)?, mtime_ms: r.get(2)?, size: r.get(3)? })
            })
            .optional()?)
    }

    pub fn backlinks(&self, path: &str) -> Result<Vec<LinkRow>> {
        let sql = format!("SELECT {LINK_COLUMNS} FROM links l JOIN notes n ON n.path=l.src_path WHERE l.target_path=?1 ORDER BY l.src_path, l.line");
        Ok(self.conn.prepare(&sql)?.query_map([path], link_row)?.collect::<std::result::Result<_, _>>()?)
    }

    pub fn outgoing(&self, path: &str) -> Result<Vec<LinkRow>> {
        let sql = format!("SELECT {LINK_COLUMNS} FROM links l JOIN notes n ON n.path=l.src_path WHERE l.src_path=?1 ORDER BY l.start");
        Ok(self.conn.prepare(&sql)?.query_map([path], link_row)?.collect::<std::result::Result<_, _>>()?)
    }

    pub fn unresolved(&self) -> Result<Vec<Unresolved>> {
        Ok(self
            .conn
            .prepare("SELECT target_raw, count(*) FROM links WHERE target_path IS NULL AND kind != 'embed' GROUP BY target_raw ORDER BY count(*) DESC, target_raw")?
            .query_map([], |r| Ok(Unresolved { target: r.get(0)?, count: r.get(1)? }))?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        Ok(self
            .conn
            .prepare("SELECT tag, count(*) FROM tags GROUP BY tag ORDER BY tag")?
            .query_map([], |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? }))?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn properties(&self, path: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
        let fm: Option<String> = self.conn.query_row("SELECT frontmatter FROM notes WHERE path=?1", [path], |r| r.get(0)).optional()?;
        Ok(fm
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default())
    }

    pub fn titles(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .conn
            .prepare("SELECT path, title FROM notes ORDER BY path")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?)
    }
}
```

Embeds of images stay unresolved by design and are excluded from `unresolved()`; a `![[note]]` embed of a markdown note resolves like any wikilink since `resolve_all` does not look at `kind`.

- [ ] **Step 5: Verify, lint, commit**

Run: `cargo test -p engram-notes-core && cargo clippy --all-targets -- -D warnings && cargo fmt --all`
Expected: all pass.

```bash
git add core
git commit -m "feat(core): link resolution, backlinks, outgoing, tags, properties"
```

---

### Task 6: Full-text search

**Files:**
- Create: `core/src/index/fts.rs`
- Modify: `core/src/index/mod.rs` (`pub mod fts;`)

**Interfaces:**
- Produces:

```rust
pub struct FtsHit { pub path: String, pub title: String, pub snippet: String, pub score: f64 }
impl Index { pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>>; }
```

`snippet` contains `<mark>` and `</mark>` around matches, with the rest HTML-escaped. The query is user text: each whitespace-separated word becomes a quoted prefix term, so FTS5 syntax characters never reach the engine.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Rust.md"), "Rust ownership rules keep memory safe. <b>bold</b>").unwrap();
        fs::write(d.path().join("Go.md"), "Garbage collection in Go").unwrap();
        fs::write(d.path().join("Umlaut.md"), "Übung macht den Meister").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    #[test]
    fn prefix_match_with_snippet() {
        let (_d, ix) = ix();
        let hits = ix.search_fts("owner", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "Rust.md");
        assert!(hits[0].snippet.contains("<mark>ownership</mark>"));
        assert!(hits[0].snippet.contains("&lt;b&gt;"));
    }

    #[test]
    fn title_matches_rank_first_and_syntax_is_inert() {
        let (_d, ix) = ix();
        let hits = ix.search_fts("go", 10).unwrap();
        assert_eq!(hits[0].path, "Go.md");
        assert!(ix.search_fts("\"(", 10).is_ok());
        assert!(ix.search_fts("   ", 10).unwrap().is_empty());
    }

    #[test]
    fn diacritics_are_folded() {
        let (_d, ix) = ix();
        assert_eq!(ix.search_fts("ubung", 10).unwrap()[0].path, "Umlaut.md");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engram-notes-core index::fts`
Expected: compile error.

- [ ] **Step 3: Implement**

```rust
use super::Index;
use crate::Result;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FtsHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

pub fn fts_query(user: &str) -> Option<String> {
    let terms: Vec<String> = user
        .split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() { None } else { Some(terms.join(" ")) }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl Index {
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>> {
        let Some(q) = fts_query(query) else { return Ok(vec![]) };
        // Markers that survive escaping, swapped for tags afterwards.
        let sql = "SELECT n.path, n.title,
                          snippet(notes_fts, 1, '\u{1}', '\u{2}', '…', 24),
                          bm25(notes_fts, 10.0, 1.0)
                   FROM notes_fts JOIN notes n ON n.rowid = notes_fts.rowid
                   WHERE notes_fts MATCH ?1
                   ORDER BY bm25(notes_fts, 10.0, 1.0)
                   LIMIT ?2";
        let rows = self
            .conn
            .prepare(sql)?
            .query_map(rusqlite::params![q, limit as i64], |r| {
                let raw: String = r.get(2)?;
                let snippet = escape_html(&raw).replace('\u{1}', "<mark>").replace('\u{2}', "</mark>");
                Ok(FtsHit { path: r.get(0)?, title: r.get(1)?, snippet, score: -r.get::<_, f64>(3)? })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}
```

`bm25` returns lower-is-better negatives; the title column weight of 10 makes a title hit outrank a body hit. `score` is negated so higher is better for callers.

- [ ] **Step 4: Verify, lint, commit**

Run: `cargo test -p engram-notes-core && cargo clippy --all-targets -- -D warnings && cargo fmt --all`

```bash
git add core
git commit -m "feat(core): full-text search over titles and bodies"
```

---

### Task 7: Rename with link rewriting

**Files:**
- Create: `core/src/rename.rs`
- Modify: `core/src/lib.rs` (`pub mod rename;`)

**Interfaces:**
- Consumes: `Index::backlinks`, `Index::resolve_target`, `Index::titles`, `Vault::read/write/rename`, `Index::update_file`, `parse::parse`.
- Produces:

```rust
pub struct RenamePlan { pub from: String, pub to: String, pub affected: Vec<String> }  // affected: paths whose text will change
pub fn plan_rename(index: &Index, from: &str, to: &str) -> Result<RenamePlan>;
pub fn apply_rename(vault: &Vault, index: &mut Index, plan: &RenamePlan) -> Result<()>;
pub fn rewrite_links(text: &str, old_path: &str, new_path: &str, new_is_unique_stem: bool) -> String;  // pure
```

Rewrite rule: a wikilink or embed whose target resolves to `old_path` gets its target replaced by the new stem when the new stem is unique in the vault, else by the new path without `.md`. Heading and alias parts are kept. A markdown link gets the new path, percent-encoding spaces. Everything else in the file is untouched, byte for byte.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    #[test]
    fn rewrite_keeps_alias_heading_and_other_text() {
        let text = "see [[Old|shown]] and [[Old#Sec]] and ![[Old]] and [[Older]] and [m](sub/Old.md) `[[Old]]`";
        let out = rewrite_links(text, "sub/Old.md", "New Name.md", true);
        assert_eq!(out, "see [[New Name|shown]] and [[New Name#Sec]] and ![[New Name]] and [[Older]] and [m](New%20Name.md) `[[Old]]`");
    }

    #[test]
    fn rewrite_uses_full_path_when_stem_is_ambiguous() {
        let out = rewrite_links("[[Old]]", "Old.md", "a/Note.md", false);
        assert_eq!(out, "[[a/Note]]");
    }

    #[test]
    fn plan_and_apply_move_the_file_and_fix_referrers() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Old.md"), "I am old").unwrap();
        fs::write(d.path().join("Ref.md"), "link [[Old]]").unwrap();
        fs::write(d.path().join("Unrelated.md"), "[[Ref]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let plan = plan_rename(&ix, "Old.md", "sub/New.md").unwrap();
        assert_eq!(plan.affected, vec!["Ref.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("Ref.md").unwrap(), "link [[New]]");
        assert_eq!(v.read("sub/New.md").unwrap(), "I am old");
        assert!(ix.note("Old.md").unwrap().is_none());
        assert_eq!(ix.backlinks("sub/New.md").unwrap().len(), 1);
        assert!(ix.unresolved().unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engram-notes-core rename`
Expected: compile error.

- [ ] **Step 3: Implement**

```rust
//! Renaming a note means rewriting every link that pointed at it.

use crate::index::Index;
use crate::parse::{LinkKind, parse};
use crate::vault::{Vault, normalize_rel};
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RenamePlan {
    pub from: String,
    pub to: String,
    pub affected: Vec<String>,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

pub fn plan_rename(index: &Index, from: &str, to: &str) -> Result<RenamePlan> {
    let from = normalize_rel(from);
    let to = normalize_rel(to);
    let mut affected: Vec<String> = index.backlinks(&from)?.into_iter().map(|l| l.src_path).filter(|p| *p != from).collect();
    affected.dedup();
    Ok(RenamePlan { from, to, affected })
}

pub fn apply_rename(vault: &Vault, index: &mut Index, plan: &RenamePlan) -> Result<()> {
    let new_stem = stem(&plan.to).to_lowercase();
    let unique = !index
        .titles()?
        .iter()
        .any(|(p, _)| *p != plan.from && stem(p).to_lowercase() == new_stem);
    vault.rename(&plan.from, &plan.to)?;
    for path in &plan.affected {
        let text = vault.read(path)?;
        let out = rewrite_links(&text, &plan.from, &plan.to, unique);
        if out != text {
            vault.write(path, &out)?;
        }
    }
    // Self-links inside the moved note point at its old name.
    let own = vault.read(&plan.to)?;
    let own_out = rewrite_links(&own, &plan.from, &plan.to, unique);
    if own_out != own {
        vault.write(&plan.to, &own_out)?;
    }
    index.remove_file(&plan.from)?;
    index.update_file(vault, &plan.to)?;
    for path in &plan.affected {
        index.update_file(vault, path)?;
    }
    Ok(())
}

/// Does `target`, as written in a link, name `old_path`? Same rule as the
/// index's resolver, applied to one known note: exact path or bare stem.
fn names(target: &str, old_path: &str) -> bool {
    let t = normalize_rel(target).to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t).to_owned();
    let old = old_path.to_lowercase();
    let old = old.strip_suffix(".md").unwrap_or(&old);
    t == old || t == stem(old)
}

pub fn rewrite_links(text: &str, old_path: &str, new_path: &str, new_is_unique_stem: bool) -> String {
    let note = parse(text);
    let new_wiki = if new_is_unique_stem { stem(new_path).to_owned() } else { new_path.strip_suffix(".md").unwrap_or(new_path).to_owned() };
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for l in &note.links {
        if !names(&l.target, old_path) {
            continue;
        }
        out.push_str(&text[pos..l.start]);
        let original = &text[l.start..l.end];
        match l.kind {
            LinkKind::Wiki | LinkKind::Embed => {
                let bang = if l.kind == LinkKind::Embed { "!" } else { "" };
                let heading = l.heading.as_ref().map(|h| format!("#{h}")).unwrap_or_default();
                let alias = l.alias.as_ref().map(|a| format!("|{a}")).unwrap_or_default();
                out.push_str(&format!("{bang}[[{new_wiki}{heading}{alias}]]"));
            }
            LinkKind::Markdown => {
                let close = original.rfind('(').unwrap_or(0);
                out.push_str(&original[..=close]);
                out.push_str(&new_path.replace(' ', "%20"));
                out.push(')');
            }
        }
        pos = l.end;
    }
    out.push_str(&text[pos..]);
    out
}
```

Note `names` must not match `[[Older]]` against `Old.md`: it compares whole stems, so it does not. Links in code spans are never in `note.links`, so they are untouched.

- [ ] **Step 4: Verify, lint, commit**

Run: `cargo test -p engram-notes-core && cargo clippy --all-targets -- -D warnings && cargo fmt --all`

```bash
git add core
git commit -m "feat(core): rename a note and rewrite the links that pointed at it"
```

---

### Task 8: Config, workspace state and the file watcher

**Files:**
- Create: `core/src/config.rs`, `core/src/watch.rs`
- Modify: `core/src/lib.rs`, `core/Cargo.toml`

**Interfaces:**
- Produces:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub editor: EditorConfig,      // { default_mode: "live" | "source" | "reading" }
    pub daily_notes: DailyNotes,   // { folder: "Daily", template: Option<String>, format: "%Y-%m-%d" }
    pub hotkeys: BTreeMap<String, String>,   // command id -> key chord, empty means defaults
    pub theme: String,             // "system" | "light" | "dark"
}
pub fn load_config(vault: &Vault) -> Result<AppConfig>;       // missing file -> defaults, written
pub fn save_config(vault: &Vault, cfg: &AppConfig) -> Result<()>;
pub fn load_workspace(vault: &Vault) -> Result<serde_json::Value>;   // {} when missing
pub fn save_workspace(vault: &Vault, ws: &serde_json::Value) -> Result<()>;
pub fn daily_note_path(cfg: &AppConfig, today: chrono::NaiveDate) -> String;   // "Daily/2026-09-12.md"
pub fn data_dir() -> Result<PathBuf>;                               // OS data dir /engram-notes
pub fn index_path(vault: &Vault) -> Result<PathBuf>;                // data_dir/vaults/<sha256 of root>/index.db

pub enum ChangeKind { Changed, Removed }
pub struct Change { pub path: String, pub kind: ChangeKind }
pub struct Watcher { .. }   // dropping it stops watching
pub fn watch(vault: &Vault, on_change: impl Fn(Vec<Change>) + Send + 'static) -> Result<Watcher>;
```

Changes under `.engram-notes/` and other hidden paths are filtered out. Non-markdown files pass through with their path so the explorer can refresh.

- [ ] **Step 1: Dependencies**

```toml
dirs = "7"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
notify = "8"
notify-debouncer-full = "0.7"
```

- [ ] **Step 2: Failing tests (config)**

`core/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_written_on_first_load_and_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg, AppConfig::default());
        assert!(d.path().join(".engram-notes/app.json").is_file());
        let mut changed = cfg.clone();
        changed.theme = "dark".into();
        changed.daily_notes.folder = "Journal".into();
        save_config(&v, &changed).unwrap();
        assert_eq!(load_config(&v).unwrap(), changed);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(d.path().join(".engram-notes/app.json"), r#"{"theme":"light"}"#).unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg.theme, "light");
        assert_eq!(cfg.daily_notes.folder, "Daily");
    }

    #[test]
    fn workspace_is_opaque_json() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert_eq!(load_workspace(&v).unwrap(), serde_json::json!({}));
        save_workspace(&v, &serde_json::json!({"tabs": ["a.md"]})).unwrap();
        assert_eq!(load_workspace(&v).unwrap()["tabs"][0], "a.md");
    }

    #[test]
    fn daily_path_and_index_path() {
        let cfg = AppConfig::default();
        assert_eq!(daily_note_path(&cfg, chrono::NaiveDate::from_ymd_opt(2026, 9, 12).unwrap()), "Daily/2026-09-12.md");
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let p = index_path(&v).unwrap();
        assert!(p.ends_with("index.db"));
        assert!(p.to_string_lossy().contains("engram-notes"));
    }
}
```

- [ ] **Step 3: Implement config**

```rust
//! Vault-level settings under `.engram-notes/`, and where derived state goes.

use crate::vault::Vault;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorConfig {
    pub default_mode: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        EditorConfig { default_mode: "live".into() }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct DailyNotes {
    pub folder: String,
    pub template: Option<String>,
    pub format: String,
}

impl Default for DailyNotes {
    fn default() -> Self {
        DailyNotes { folder: "Daily".into(), template: None, format: "%Y-%m-%d".into() }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub editor: EditorConfig,
    pub daily_notes: DailyNotes,
    pub hotkeys: BTreeMap<String, String>,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { editor: EditorConfig::default(), daily_notes: DailyNotes::default(), hotkeys: BTreeMap::new(), theme: "system".into() }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<Option<T>> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map(Some).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| Error::Config(e.to_string()))?;
    std::fs::write(path, text).map_err(|e| Error::io(path, e))
}

pub fn load_config(vault: &Vault) -> Result<AppConfig> {
    let path = vault.config_dir().join("app.json");
    match read_json::<AppConfig>(&path)? {
        Some(cfg) => Ok(cfg),
        None => {
            let cfg = AppConfig::default();
            write_json(&path, &cfg)?;
            Ok(cfg)
        }
    }
}

pub fn save_config(vault: &Vault, cfg: &AppConfig) -> Result<()> {
    write_json(&vault.config_dir().join("app.json"), cfg)
}

pub fn load_workspace(vault: &Vault) -> Result<serde_json::Value> {
    Ok(read_json(&vault.config_dir().join("workspace.json"))?.unwrap_or_else(|| serde_json::json!({})))
}

pub fn save_workspace(vault: &Vault, ws: &serde_json::Value) -> Result<()> {
    write_json(&vault.config_dir().join("workspace.json"), ws)
}

pub fn daily_note_path(cfg: &AppConfig, today: chrono::NaiveDate) -> String {
    let name = today.format(&cfg.daily_notes.format).to_string();
    let folder = cfg.daily_notes.folder.trim_matches('/');
    if folder.is_empty() { format!("{name}.md") } else { format!("{folder}/{name}.md") }
}

pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir().map(|d| d.join("engram-notes")).ok_or_else(|| Error::Config("no data directory".into()))
}

pub fn index_path(vault: &Vault) -> Result<PathBuf> {
    use sha2::Digest;
    let key = hex::encode(sha2::Sha256::digest(vault.root().to_string_lossy().as_bytes()));
    Ok(data_dir()?.join("vaults").join(&key[..16]).join("index.db"))
}
```

- [ ] **Step 4: Failing test (watch)**

`core/src/watch.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn reports_markdown_changes_and_ignores_config_dir() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let seen: Arc<Mutex<Vec<Change>>> = Arc::default();
        let sink = seen.clone();
        let _w = watch(&v, move |c| sink.lock().unwrap().extend(c)).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(d.path().join("n.md"), "x").unwrap();
        std::fs::write(d.path().join(".engram-notes/app.json"), "{}").unwrap();
        let start = Instant::now();
        loop {
            if seen.lock().unwrap().iter().any(|c| c.path == "n.md") || start.elapsed() > Duration::from_secs(5) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let got = seen.lock().unwrap();
        assert!(got.iter().any(|c| c.path == "n.md" && c.kind == ChangeKind::Changed), "{got:?}");
        assert!(!got.iter().any(|c| c.path.starts_with(".engram-notes")));
    }
}
```

- [ ] **Step 5: Implement watch**

```rust
//! File changes from outside the app, debounced, as vault-relative paths.

use crate::vault::Vault;
use crate::{Error, Result};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Changed,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

pub struct Watcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

fn rel(root: &Path, p: &Path) -> Option<String> {
    let r = p.strip_prefix(root).ok()?;
    if r.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
        return None;
    }
    Some(r.to_string_lossy().replace('\\', "/"))
}

pub fn watch(vault: &Vault, on_change: impl Fn(Vec<Change>) + Send + 'static) -> Result<Watcher> {
    let root: PathBuf = vault.root().to_path_buf();
    let handler_root = root.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(300), None, move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let mut out: Vec<Change> = Vec::new();
        for ev in events {
            let kind = if ev.kind.is_remove() { ChangeKind::Removed } else { ChangeKind::Changed };
            for p in &ev.paths {
                if let Some(path) = rel(&handler_root, p) {
                    // A rename arrives as one event with two paths: old then new.
                    let k = if ev.kind.is_modify() && ev.paths.len() == 2 && p == &ev.paths[0] { ChangeKind::Removed } else { kind.clone() };
                    if !out.iter().any(|c| c.path == path && c.kind == k) {
                        out.push(Change { path, kind: k });
                    }
                }
            }
        }
        if !out.is_empty() {
            on_change(out);
        }
    })
    .map_err(|e| Error::io(&root, std::io::Error::other(e)))?;
    debouncer
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| Error::io(&root, std::io::Error::other(e)))?;
    Ok(Watcher { _debouncer: debouncer })
}
```

The consumer treats a `Changed` for a path that no longer exists as a removal anyway (`Index::update_file` does), so a missed rename half is harmless.

- [ ] **Step 6: Verify, lint, commit**

Add `pub mod config; pub mod watch;` to `lib.rs`. Run: `cargo test -p engram-notes-core && cargo clippy --all-targets -- -D warnings && cargo fmt --all`

```bash
git add core
git commit -m "feat(core): vault config, workspace state, index location, file watcher"
```

---

### Task 9: Tauri shell and commands

Requires the system packages from *Machine prerequisites*.

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`, `src-tauri/src/error.rs`, `src-tauri/src/commands.rs`, `src-tauri/icons/*` (generated), `assets/icon.svg`, `core/src/frontmatter.rs`
- Modify: `Cargo.toml` (add `src-tauri` to members), `.gitignore` (add `/src-tauri/gen`)

**Interfaces:**
- Consumes: everything `core` exports.
- Produces the command surface the UI calls (all names exact; arguments are camelCase on the JS side, Tauri converts):

| command | args | returns |
| --- | --- | --- |
| `open_vault` | `path: String` | `VaultInfo { root, config, stats: RebuildStats }` |
| `recent_vaults` | | `Vec<String>` |
| `list_files` | | `Vec<FileEntry>` |
| `read_note` | `path` | `NoteText { path, text, mtime_ms }` |
| `write_note` | `path, text` | `i64` new mtime_ms |
| `create_note` | `path, text` | `()` |
| `delete_file` | `path` | `()` |
| `plan_rename` | `from, to` | `RenamePlan` |
| `apply_rename` | `plan: RenamePlan` | `()` |
| `create_folder` | `path` | `()` |
| `backlinks` | `path` | `Vec<LinkRow>` |
| `outgoing` | `path` | `Vec<LinkRow>` |
| `unresolved` | | `Vec<Unresolved>` |
| `resolve_link` | `target` | `Option<String>` |
| `titles` | | `Vec<(String, String)>` |
| `tags` | | `Vec<TagCount>` |
| `properties` | `path` | JSON object |
| `set_property` | `path, key, value: Value` | `()` rewrites frontmatter |
| `search` | `query, limit` | `Vec<FtsHit>` |
| `get_config` / `set_config` | / `config: AppConfig` | `AppConfig` / `()` |
| `get_workspace` / `set_workspace` | / `workspace: Value` | `Value` / `()` |
| `daily_note` | | `String` path, created if missing |

Events emitted to the window: `index-changed` with `Vec<Change>` after the watcher's changes were applied; `file-changed` with `Change` for every change (the UI decides whether an open tab is affected).

- [ ] **Step 1: Crate manifest and Tauri config**

`src-tauri/Cargo.toml`:

```toml
[package]
name = "engram-notes"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[lib]
name = "engram_notes_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
engram_core = { package = "engram-notes-core", path = "../core" }
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-opener = "2"
serde.workspace = true
serde_json.workspace = true
chrono = { version = "0.4", default-features = false, features = ["clock"] }
```

`src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "engram-notes",
  "version": "../Cargo.toml",
  "identifier": "org.overcuriousity.engram-notes",
  "build": {
    "frontendDist": "../ui/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm --dir ../ui dev",
    "beforeBuildCommand": "pnpm --dir ../ui build"
  },
  "app": {
    "windows": [
      { "title": "engram-notes", "width": 1280, "height": 820, "minWidth": 720, "minHeight": 480 }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

`"version": "../Cargo.toml"` reads the workspace version; if the Tauri CLI rejects it, put `"0.1.0"` and keep it in step with `Cargo.toml` at release time.

`src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "main window",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default", "opener:default"]
}
```

Icon: write `assets/icon.svg` as a 512×512 rounded square in `#3b6ea5` with a white `e` glyph (any simple SVG with a transparent background works). Generate the set with:

```bash
cd ui && pnpm dlx @tauri-apps/cli@2 icon ../assets/icon.svg -o ../src-tauri/icons
```

(`ui` and its `package.json` come from Task 10; if doing Task 9 first, run `pnpm dlx` from the repo root instead.)

- [ ] **Step 2: State and error mapping**

`src-tauri/src/state.rs`:

```rust
use engram_core::config::AppConfig;
use engram_core::index::Index;
use engram_core::vault::Vault;
use engram_core::watch::Watcher;
use std::sync::Mutex;

pub struct Open {
    pub vault: Vault,
    pub index: Index,
    pub config: AppConfig,
    pub watcher: Option<Watcher>,
}

#[derive(Default)]
pub struct AppState {
    pub open: Mutex<Option<Open>>,
}
```

`src-tauri/src/error.rs`:

```rust
#[derive(serde::Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<engram_core::Error> for CommandError {
    fn from(e: engram_core::Error) -> Self {
        let code = match &e {
            engram_core::Error::Io { .. } => "io",
            engram_core::Error::Parse { .. } => "parse",
            engram_core::Error::Index(_) => "index",
            engram_core::Error::Config(_) => "config",
            engram_core::Error::NotFound(_) => "not_found",
            engram_core::Error::Exists(_) => "exists",
        };
        CommandError { code, message: e.to_string() }
    }
}

impl CommandError {
    pub fn closed() -> Self {
        CommandError { code: "no_vault", message: "no vault is open".into() }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;
```

- [ ] **Step 3: Commands**

`src-tauri/src/commands.rs`:

```rust
use crate::error::{CmdResult, CommandError};
use crate::state::{AppState, Open};
use engram_core::config::{self, AppConfig};
use engram_core::index::fts::FtsHit;
use engram_core::index::query::{LinkRow, TagCount, Unresolved};
use engram_core::index::{Index, RebuildStats};
use engram_engram_core::rename::RenamePlan;
use engram_core::vault::{FileEntry, Vault};
use engram_core::watch::{Change, ChangeKind};
use std::sync::MutexGuard;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(serde::Serialize)]
pub struct VaultInfo {
    pub root: String,
    pub config: AppConfig,
    pub stats: RebuildStats,
}

#[derive(serde::Serialize)]
pub struct NoteText {
    pub path: String,
    pub text: String,
    pub mtime_ms: i64,
}

fn with_open<T>(state: &State<AppState>, f: impl FnOnce(&mut Open) -> CmdResult<T>) -> CmdResult<T> {
    let mut guard: MutexGuard<Option<Open>> = state.open.lock().unwrap();
    let open = guard.as_mut().ok_or_else(CommandError::closed)?;
    f(open)
}

fn recent_file() -> Option<std::path::PathBuf> {
    config::data_dir().ok().map(|d| d.join("recent.json"))
}

fn remember(root: &str) {
    let Some(p) = recent_file() else { return };
    let mut list: Vec<String> = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    list.retain(|r| r != root);
    list.insert(0, root.to_owned());
    list.truncate(10);
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    let _ = std::fs::write(&p, serde_json::to_string(&list).unwrap());
}

#[tauri::command]
pub fn recent_vaults() -> Vec<String> {
    recent_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<VaultInfo> {
    let vault = Vault::open(&path)?;
    let mut index = Index::open(&config::index_path(&vault)?)?;
    let stats = index.rebuild(&vault)?;
    let cfg = config::load_config(&vault)?;
    let root = vault.root().to_string_lossy().into_owned();
    remember(&root);

    let handle = app.clone();
    let watcher = engram_core::watch::watch(&vault, move |changes| apply_changes(&handle, changes)).ok();

    *state.open.lock().unwrap() = Some(Open { vault, index, config: cfg.clone(), watcher });
    Ok(VaultInfo { root, config: cfg, stats })
}

// Runs on the watcher thread: re-index what changed, then tell the window.
fn apply_changes(app: &AppHandle, changes: Vec<Change>) {
    let state = app.state::<AppState>();
    let mut guard = state.open.lock().unwrap();
    let Some(open) = guard.as_mut() else { return };
    for c in &changes {
        let _ = match c.kind {
            ChangeKind::Removed => open.index.remove_file(&c.path),
            ChangeKind::Changed => open.index.update_file(&open.vault, &c.path).map(|_| ()),
        };
        let _ = app.emit("file-changed", c);
    }
    let _ = app.emit("index-changed", &changes);
}

#[tauri::command]
pub fn list_files(state: State<AppState>) -> CmdResult<Vec<FileEntry>> {
    with_open(&state, |o| Ok(o.vault.walk()?))
}

#[tauri::command]
pub fn read_note(state: State<AppState>, path: String) -> CmdResult<NoteText> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let mtime_ms = o.vault.stat(&path)?.map(|e| e.mtime_ms).unwrap_or(0);
        Ok(NoteText { path, text, mtime_ms })
    })
}

#[tauri::command]
pub fn write_note(state: State<AppState>, path: String, text: String) -> CmdResult<i64> {
    with_open(&state, |o| {
        o.vault.write(&path, &text)?;
        o.index.update_file(&o.vault, &path)?;
        Ok(o.vault.stat(&path)?.map(|e| e.mtime_ms).unwrap_or(0))
    })
}

#[tauri::command]
pub fn create_note(state: State<AppState>, path: String, text: String) -> CmdResult<()> {
    with_open(&state, |o| {
        o.vault.create(&path, &text)?;
        o.index.update_file(&o.vault, &path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn create_folder(state: State<AppState>, path: String) -> CmdResult<()> {
    with_open(&state, |o| {
        let abs = o.vault.abs(&path);
        std::fs::create_dir_all(&abs).map_err(|e| engram_core::Error::io(abs, e))?;
        Ok(())
    })
}

#[tauri::command]
pub fn delete_file(state: State<AppState>, path: String) -> CmdResult<()> {
    with_open(&state, |o| {
        o.vault.delete(&path)?;
        o.index.remove_file(&path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn plan_rename(state: State<AppState>, from: String, to: String) -> CmdResult<RenamePlan> {
    with_open(&state, |o| Ok(engram_core::rename::plan_rename(&o.index, &from, &to)?))
}

#[tauri::command]
pub fn apply_rename(state: State<AppState>, plan: RenamePlan) -> CmdResult<()> {
    with_open(&state, |o| Ok(engram_core::rename::apply_rename(&o.vault, &mut o.index, &plan)?))
}

#[tauri::command]
pub fn backlinks(state: State<AppState>, path: String) -> CmdResult<Vec<LinkRow>> {
    with_open(&state, |o| Ok(o.index.backlinks(&path)?))
}

#[tauri::command]
pub fn outgoing(state: State<AppState>, path: String) -> CmdResult<Vec<LinkRow>> {
    with_open(&state, |o| Ok(o.index.outgoing(&path)?))
}

#[tauri::command]
pub fn unresolved(state: State<AppState>) -> CmdResult<Vec<Unresolved>> {
    with_open(&state, |o| Ok(o.index.unresolved()?))
}

#[tauri::command]
pub fn resolve_link(state: State<AppState>, target: String) -> CmdResult<Option<String>> {
    with_open(&state, |o| Ok(o.index.resolve_target(&target)?))
}

#[tauri::command]
pub fn titles(state: State<AppState>) -> CmdResult<Vec<(String, String)>> {
    with_open(&state, |o| Ok(o.index.titles()?))
}

#[tauri::command]
pub fn tags(state: State<AppState>) -> CmdResult<Vec<TagCount>> {
    with_open(&state, |o| Ok(o.index.tags()?))
}

#[tauri::command]
pub fn properties(state: State<AppState>, path: String) -> CmdResult<serde_json::Map<String, serde_json::Value>> {
    with_open(&state, |o| Ok(o.index.properties(&path)?))
}

#[tauri::command]
pub fn set_property(state: State<AppState>, path: String, key: String, value: serde_json::Value) -> CmdResult<()> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let out = engram_core::frontmatter::set_property(&text, &key, value);
        o.vault.write(&path, &out)?;
        o.index.update_file(&o.vault, &path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn search(state: State<AppState>, query: String, limit: Option<usize>) -> CmdResult<Vec<FtsHit>> {
    with_open(&state, |o| Ok(o.index.search_fts(&query, limit.unwrap_or(50))?))
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> CmdResult<AppConfig> {
    with_open(&state, |o| Ok(o.config.clone()))
}

#[tauri::command]
pub fn set_config(state: State<AppState>, config: AppConfig) -> CmdResult<()> {
    with_open(&state, |o| {
        config::save_config(&o.vault, &config)?;
        o.config = config;
        Ok(())
    })
}

#[tauri::command]
pub fn get_workspace(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_open(&state, |o| Ok(config::load_workspace(&o.vault)?))
}

#[tauri::command]
pub fn set_workspace(state: State<AppState>, workspace: serde_json::Value) -> CmdResult<()> {
    with_open(&state, |o| Ok(config::save_workspace(&o.vault, &workspace)?))
}

#[tauri::command]
pub fn daily_note(state: State<AppState>) -> CmdResult<String> {
    with_open(&state, |o| {
        let path = config::daily_note_path(&o.config, chrono::Local::now().date_naive());
        if o.vault.stat(&path)?.is_none() {
            let template = match &o.config.daily_notes.template {
                Some(t) => o.vault.read(t).unwrap_or_default(),
                None => String::new(),
            };
            o.vault.create(&path, &template)?;
            o.index.update_file(&o.vault, &path)?;
        }
        Ok(path)
    })
}
```

`set_property` needs one more core function. Add `core/src/frontmatter.rs` with tests and `pub mod frontmatter;` in `lib.rs`:

```rust
//! Edit one frontmatter key in a note's text, keeping everything else.

pub fn set_property(text: &str, key: &str, value: serde_json::Value) -> String {
    let parsed = crate::parse::parse(text);
    let mut map = parsed.frontmatter.clone();
    if value.is_null() {
        map.remove(key);
    } else {
        map.insert(key.to_owned(), value);
    }
    let body = if parsed.body_offset == 0 { text } else { &text[parsed.body_offset..] };
    if map.is_empty() {
        return body.to_owned();
    }
    let yaml = serde_yaml_ng::to_string(&map).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_updates_and_removes() {
        let t = set_property("body", "a", serde_json::json!(1));
        assert_eq!(t, "---\na: 1\n---\nbody");
        let t = set_property(&t, "a", serde_json::json!("x"));
        assert_eq!(t, "---\na: x\n---\nbody");
        let t = set_property(&t, "a", serde_json::Value::Null);
        assert_eq!(t, "body");
    }

    #[test]
    fn keeps_other_keys_and_body() {
        let t = set_property("---\nk: v\n---\n# H", "n", serde_json::json!([1, 2]));
        assert_eq!(t, "---\nk: v\nn:\n- 1\n- 2\n---\n# H");
    }
}
```

`serde_json::Map` keeps insertion order because the workspace enables `preserve_order` (Task 1), so keys keep their file order.

- [ ] **Step 4: Entry points**

`src-tauri/src/lib.rs`:

```rust
mod commands;
mod error;
mod state;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::recent_vaults,
            commands::open_vault,
            commands::list_files,
            commands::read_note,
            commands::write_note,
            commands::create_note,
            commands::create_folder,
            commands::delete_file,
            commands::plan_rename,
            commands::apply_rename,
            commands::backlinks,
            commands::outgoing,
            commands::unresolved,
            commands::resolve_link,
            commands::titles,
            commands::tags,
            commands::properties,
            commands::set_property,
            commands::search,
            commands::get_config,
            commands::set_config,
            commands::get_workspace,
            commands::set_workspace,
            commands::daily_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running engram-notes");
}
```

`src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    engram_notes_lib::run()
}
```

- [ ] **Step 5: Build**

Add `src-tauri` to `members` in the root `Cargo.toml`. A `ui/dist/index.html` must exist for `generate_context!` to embed; create `ui/dist/index.html` containing `<!doctype html><title>engram-notes</title>` for now (Task 10 replaces `ui/` wholesale and `.gitignore` already excludes `ui/dist`).

Run: `cargo build -p engram-notes && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: builds; the `set_property` tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src-tauri assets core .gitignore
git commit -m "feat(app): tauri shell with vault, note, link, search and config commands"
```

---

### Task 10: Frontend scaffold, vault picker, explorer, tabs, source editor

**Files:**
- Create: `ui/package.json`, `ui/vite.config.ts`, `ui/tsconfig.json`, `ui/svelte.config.js`, `ui/index.html`, `ui/src/main.ts`, `ui/src/app.css`, `ui/src/App.svelte`, `ui/src/lib/api.ts`, `ui/src/lib/state.svelte.ts`, `ui/src/lib/wikilink.ts`, `ui/src/lib/wikilink.test.ts`, `ui/src/components/VaultPicker.svelte`, `ui/src/components/Explorer.svelte`, `ui/src/components/Tabs.svelte`, `ui/src/components/NoteView.svelte`, `ui/src/components/Editor.svelte`, `ui/src/editor/theme.ts`, `ui/src/components/StatusBar.svelte`
- Delete: the placeholder `ui/dist/index.html` from Task 9 (it is gitignored; just let `pnpm build` overwrite it)

**Interfaces:**
- Consumes: the Task 9 commands.
- Produces: `api.ts` functions named exactly as the commands, `state.svelte.ts` with `app` (vault, files, tabs, active), `openNote(path)`.

- [ ] **Step 1: Package and tool config**

`ui/package.json`:

```json
{
  "name": "engram-notes-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "tauri": "tauri"
  },
  "dependencies": {
    "@codemirror/autocomplete": "^6.18.0",
    "@codemirror/commands": "^6.8.0",
    "@codemirror/lang-markdown": "^6.5.2",
    "@codemirror/language": "^6.11.0",
    "@codemirror/language-data": "^6.5.2",
    "@codemirror/search": "^6.5.0",
    "@codemirror/state": "^6.7.4",
    "@codemirror/view": "^6.43.11",
    "@lezer/highlight": "^1.2.0",
    "@tauri-apps/api": "^2.11.1",
    "@tauri-apps/plugin-dialog": "^2.7.3",
    "@tauri-apps/plugin-opener": "^2.5.5",
    "markdown-it": "^15.0.2"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^7.3.0",
    "@tauri-apps/cli": "^2.11.4",
    "@types/markdown-it": "^14.2.0",
    "svelte": "^5.57.0",
    "svelte-check": "^4.7.6",
    "typescript": "^5.9.3",
    "vite": "^8.3.0",
    "vitest": "^5.0.0"
  }
}
```

`ui/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022" },
  test: { include: ["src/**/*.test.ts"] },
});
```

If TypeScript rejects the `test` key, add `/// <reference types="vitest/config" />` at the top of the file.

`ui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "vite.config.ts"]
}
```

`ui/svelte.config.js`:

```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default { preprocess: vitePreprocess() };
```

`ui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>engram-notes</title>
    <link rel="stylesheet" href="/src/app.css" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Run: `cd ui && pnpm install`
Expected: a lockfile, no peer warnings that name svelte or vite.

- [ ] **Step 2: Wikilink helper with tests (pure TypeScript)**

`ui/src/lib/wikilink.ts`:

```ts
export interface WikiLink {
  target: string;
  heading?: string;
  alias?: string;
  embed: boolean;
  from: number;
  to: number;
}

const RE = /(!?)\[\[([^[\]|#]+)(?:#([^[\]|]*))?(?:\|([^[\]]*))?\]\]/g;

export function findWikilinks(text: string): WikiLink[] {
  const out: WikiLink[] = [];
  for (const m of text.matchAll(RE)) {
    out.push({
      target: m[2].trim(),
      heading: m[3]?.trim() || undefined,
      alias: m[4]?.trim() || undefined,
      embed: m[1] === "!",
      from: m.index!,
      to: m.index! + m[0].length,
    });
  }
  return out;
}

export function displayText(l: WikiLink): string {
  if (l.alias) return l.alias;
  return l.heading ? `${l.target} › ${l.heading}` : l.target;
}
```

`ui/src/lib/wikilink.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { displayText, findWikilinks } from "./wikilink";

describe("findWikilinks", () => {
  it("parses every form with positions", () => {
    const t = "a [[Note]] b ![[img.png]] c [[X#H|Y]]";
    const l = findWikilinks(t);
    expect(l.map((x) => [x.target, x.heading, x.alias, x.embed])).toEqual([
      ["Note", undefined, undefined, false],
      ["img.png", undefined, undefined, true],
      ["X", "H", "Y", false],
    ]);
    expect(t.slice(l[2].from, l[2].to)).toBe("[[X#H|Y]]");
  });
  it("chooses display text", () => {
    const [a, b, c] = findWikilinks("[[N]] [[N#H]] [[N|A]]");
    expect([displayText(a), displayText(b), displayText(c)]).toEqual(["N", "N › H", "A"]);
  });
});
```

Run: `pnpm test` Expected: 2 passed.

- [ ] **Step 3: API layer**

`ui/src/lib/api.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FileEntry { path: string; mtime_ms: number; size: number; is_markdown: boolean }
export interface RebuildStats { added: number; updated: number; removed: number; unchanged: number }
export interface AppConfig {
  editor: { default_mode: "live" | "source" | "reading" };
  daily_notes: { folder: string; template: string | null; format: string };
  hotkeys: Record<string, string>;
  theme: "system" | "light" | "dark";
}
export interface VaultInfo { root: string; config: AppConfig; stats: RebuildStats }
export interface NoteText { path: string; text: string; mtime_ms: number }
export interface LinkRow {
  src_path: string; target_raw: string; target_path: string | null; kind: string;
  heading: string | null; alias: string | null; line: number; context: string;
}
export interface Unresolved { target: string; count: number }
export interface TagCount { tag: string; count: number }
export interface FtsHit { path: string; title: string; snippet: string; score: number }
export interface RenamePlan { from: string; to: string; affected: string[] }
export interface Change { path: string; kind: "changed" | "removed" }
export interface CommandError { code: string; message: string }

export const recentVaults = () => invoke<string[]>("recent_vaults");
export const openVault = (path: string) => invoke<VaultInfo>("open_vault", { path });
export const listFiles = () => invoke<FileEntry[]>("list_files");
export const readNote = (path: string) => invoke<NoteText>("read_note", { path });
export const writeNote = (path: string, text: string) => invoke<number>("write_note", { path, text });
export const createNote = (path: string, text = "") => invoke<void>("create_note", { path, text });
export const createFolder = (path: string) => invoke<void>("create_folder", { path });
export const deleteFile = (path: string) => invoke<void>("delete_file", { path });
export const planRename = (from: string, to: string) => invoke<RenamePlan>("plan_rename", { from, to });
export const applyRename = (plan: RenamePlan) => invoke<void>("apply_rename", { plan });
export const backlinks = (path: string) => invoke<LinkRow[]>("backlinks", { path });
export const outgoing = (path: string) => invoke<LinkRow[]>("outgoing", { path });
export const unresolved = () => invoke<Unresolved[]>("unresolved");
export const resolveLink = (target: string) => invoke<string | null>("resolve_link", { target });
export const titles = () => invoke<[string, string][]>("titles");
export const tags = () => invoke<TagCount[]>("tags");
export const properties = (path: string) => invoke<Record<string, unknown>>("properties", { path });
export const setProperty = (path: string, key: string, value: unknown) => invoke<void>("set_property", { path, key, value });
export const search = (query: string, limit = 50) => invoke<FtsHit[]>("search", { query, limit });
export const getConfig = () => invoke<AppConfig>("get_config");
export const setConfig = (config: AppConfig) => invoke<void>("set_config", { config });
export const getWorkspace = () => invoke<Record<string, unknown>>("get_workspace");
export const setWorkspace = (workspace: Record<string, unknown>) => invoke<void>("set_workspace", { workspace });
export const dailyNote = () => invoke<string>("daily_note");

export const onIndexChanged = (f: (c: Change[]) => void): Promise<UnlistenFn> => listen<Change[]>("index-changed", (e) => f(e.payload));
export const onFileChanged = (f: (c: Change) => void): Promise<UnlistenFn> => listen<Change>("file-changed", (e) => f(e.payload));

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CommandError).message);
  return String(e);
}
```

- [ ] **Step 4: State**

`ui/src/lib/state.svelte.ts`:

```ts
import * as api from "./api";

export interface Tab {
  path: string;
  text: string;
  savedText: string;
  mtime_ms: number;
  mode: "live" | "source" | "reading";
  conflict: boolean;
}

class AppStateStore {
  root = $state<string | null>(null);
  config = $state<api.AppConfig | null>(null);
  files = $state<api.FileEntry[]>([]);
  tabs = $state<Tab[]>([]);
  active = $state<number>(-1);
  toast = $state<string | null>(null);
  titles = $state<[string, string][]>([]);

  get activeTab(): Tab | null {
    return this.active >= 0 ? this.tabs[this.active] : null;
  }

  async open(root: string) {
    const info = await api.openVault(root);
    this.root = info.root;
    this.config = info.config;
    await this.refresh();
    const ws = await api.getWorkspace();
    const open = (ws.tabs as string[] | undefined) ?? [];
    for (const p of open) await this.openNote(p, false);
    this.active = Math.min(Number(ws.active ?? 0), this.tabs.length - 1);
    await api.onIndexChanged(() => this.refresh());
    await api.onFileChanged((c) => this.externalChange(c));
  }

  async refresh() {
    this.files = await api.listFiles();
    this.titles = await api.titles();
  }

  async openNote(path: string, activate = true) {
    const i = this.tabs.findIndex((t) => t.path === path);
    if (i >= 0) {
      if (activate) this.active = i;
      return;
    }
    const n = await api.readNote(path);
    this.tabs.push({ path, text: n.text, savedText: n.text, mtime_ms: n.mtime_ms, mode: this.config?.editor.default_mode ?? "live", conflict: false });
    if (activate) this.active = this.tabs.length - 1;
    this.persist();
  }

  closeTab(i: number) {
    this.tabs.splice(i, 1);
    if (this.active >= this.tabs.length) this.active = this.tabs.length - 1;
    this.persist();
  }

  async save(tab: Tab) {
    if (tab.text === tab.savedText) return;
    try {
      tab.mtime_ms = await api.writeNote(tab.path, tab.text);
      tab.savedText = tab.text;
    } catch (e) {
      this.say(api.errorMessage(e));
    }
  }

  async externalChange(c: api.Change) {
    const tab = this.tabs.find((t) => t.path === c.path);
    if (!tab) return;
    if (c.kind === "removed") {
      tab.conflict = tab.text !== tab.savedText;
      return;
    }
    const n = await api.readNote(c.path);
    if (n.text === tab.savedText) return;
    if (tab.text === tab.savedText) {
      tab.text = n.text;
      tab.savedText = n.text;
      tab.mtime_ms = n.mtime_ms;
    } else {
      tab.conflict = true;
    }
  }

  async resolveConflict(tab: Tab, keepMine: boolean) {
    if (keepMine) {
      tab.savedText = "";
      await this.save(tab);
    } else {
      const n = await api.readNote(tab.path);
      tab.text = n.text;
      tab.savedText = n.text;
      tab.mtime_ms = n.mtime_ms;
    }
    tab.conflict = false;
  }

  persist() {
    if (!this.root) return;
    void api.setWorkspace({ tabs: this.tabs.map((t) => t.path), active: this.active });
  }

  say(msg: string) {
    this.toast = msg;
    setTimeout(() => (this.toast = null), 4000);
  }
}

export const app = new AppStateStore();
```

- [ ] **Step 5: Styles**

`ui/src/app.css`:

```css
:root {
  --bg: #ffffff;
  --bg-2: #f5f6f8;
  --bg-3: #e9ebef;
  --fg: #1f2328;
  --fg-muted: #6b7280;
  --accent: #3b6ea5;
  --accent-bg: #e3edf8;
  --border: #d9dde3;
  --mark: #fff3a3;
  --font-ui: system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-text: "Inter", system-ui, sans-serif;
  --font-mono: ui-monospace, "JetBrains Mono", Menlo, Consolas, monospace;
  --radius: 6px;
}
:root[data-theme="dark"] {
  --bg: #1e1f22;
  --bg-2: #242629;
  --bg-3: #2c2f33;
  --fg: #dcdfe4;
  --fg-muted: #8b919a;
  --accent: #7aa2f7;
  --accent-bg: #263042;
  --border: #35393f;
  --mark: #5a4b12;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #1e1f22; --bg-2: #242629; --bg-3: #2c2f33; --fg: #dcdfe4; --fg-muted: #8b919a;
    --accent: #7aa2f7; --accent-bg: #263042; --border: #35393f; --mark: #5a4b12;
  }
}
* { box-sizing: border-box; }
html, body, #app { height: 100%; margin: 0; }
body { background: var(--bg); color: var(--fg); font: 14px/1.5 var(--font-ui); overflow: hidden; }
button { font: inherit; color: inherit; background: none; border: none; cursor: pointer; }
input { font: inherit; color: var(--fg); background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); padding: 4px 8px; }
.layout { display: grid; grid-template-columns: var(--left, 260px) 1fr var(--right, 300px); grid-template-rows: 1fr 24px; height: 100%; }
.layout.no-left { --left: 0px; }
.layout.no-right { --right: 0px; }
.sidebar { background: var(--bg-2); border-right: 1px solid var(--border); overflow: auto; }
.sidebar.right { border-right: none; border-left: 1px solid var(--border); }
.centre { display: flex; flex-direction: column; min-width: 0; }
.statusbar { grid-column: 1 / -1; background: var(--bg-2); border-top: 1px solid var(--border); font-size: 12px; color: var(--fg-muted); padding: 0 10px; display: flex; gap: 16px; align-items: center; }
.pane-title { font-size: 11px; text-transform: uppercase; letter-spacing: .06em; color: var(--fg-muted); padding: 8px 12px 4px; }
.tree button { display: block; width: 100%; text-align: left; padding: 2px 8px; border-radius: var(--radius); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tree button:hover { background: var(--bg-3); }
.tree button.active { background: var(--accent-bg); }
.tabs { display: flex; background: var(--bg-2); border-bottom: 1px solid var(--border); overflow-x: auto; }
.tabs button { padding: 6px 12px; border-right: 1px solid var(--border); color: var(--fg-muted); white-space: nowrap; }
.tabs button.active { color: var(--fg); background: var(--bg); }
.tabs button .x { margin-left: 8px; opacity: .6; }
.note { flex: 1; overflow: auto; }
.toast { position: fixed; bottom: 36px; right: 16px; background: var(--fg); color: var(--bg); padding: 8px 12px; border-radius: var(--radius); }
.conflict { background: var(--mark); padding: 6px 12px; display: flex; gap: 12px; align-items: center; }
mark { background: var(--mark); color: inherit; }
.cm-editor { height: 100%; font-family: var(--font-text); font-size: 16px; }
.cm-editor .cm-content { max-width: 760px; margin: 0 auto; padding: 24px 32px; }
.cm-editor .cm-line { padding: 0; }
.cm-editor.cm-focused { outline: none; }
```

- [ ] **Step 6: Components**

`ui/src/main.ts`:

```ts
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
```

`ui/src/App.svelte`:

```svelte
<script lang="ts">
  import { app } from "./lib/state.svelte";
  import VaultPicker from "./components/VaultPicker.svelte";
  import Explorer from "./components/Explorer.svelte";
  import Tabs from "./components/Tabs.svelte";
  import NoteView from "./components/NoteView.svelte";
  import StatusBar from "./components/StatusBar.svelte";

  let showLeft = $state(true);
  let showRight = $state(true);

  $effect(() => {
    const t = app.config?.theme ?? "system";
    if (t === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  });

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === "s") { e.preventDefault(); const t = app.activeTab; if (t) void app.save(t); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if !app.root}
  <VaultPicker />
{:else}
  <div class="layout" class:no-left={!showLeft} class:no-right={!showRight}>
    <aside class="sidebar">{#if showLeft}<Explorer />{/if}</aside>
    <main class="centre">
      <Tabs />
      {#if app.activeTab}
        {#key app.activeTab.path}<NoteView tab={app.activeTab} />{/key}
      {:else}
        <div class="note" style="display:grid;place-items:center;color:var(--fg-muted)">No note open</div>
      {/if}
    </main>
    <aside class="sidebar right">{#if showRight}<div class="pane-title">Backlinks</div>{/if}</aside>
    <StatusBar bind:showLeft bind:showRight />
  </div>
{/if}
{#if app.toast}<div class="toast">{app.toast}</div>{/if}
```

`ui/src/components/VaultPicker.svelte`:

```svelte
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { recentVaults, errorMessage } from "../lib/api";
  import { app } from "../lib/state.svelte";

  let recent = $state<string[]>([]);
  onMount(async () => { recent = await recentVaults(); });

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: "Open vault folder" });
    if (typeof dir === "string") await openIt(dir);
  }
  async function openIt(dir: string) {
    try { await app.open(dir); } catch (e) { app.say(errorMessage(e)); }
  }
</script>

<div style="display:grid;place-items:center;height:100%">
  <div style="width:420px">
    <h1 style="font-weight:600">engram-notes</h1>
    <button onclick={pick} style="padding:8px 14px;background:var(--accent);color:white;border-radius:var(--radius)">Open folder as vault</button>
    {#if recent.length}
      <div class="pane-title" style="margin-top:24px;padding-left:0">Recent</div>
      <div class="tree">{#each recent as r}<button onclick={() => openIt(r)}>{r}</button>{/each}</div>
    {/if}
  </div>
</div>
```

`ui/src/components/Explorer.svelte` (a folder tree built from the flat file list):

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { createNote, errorMessage } from "../lib/api";

  interface Node { name: string; path: string; dirs: Node[]; files: { name: string; path: string }[] }

  let collapsed = $state<Record<string, boolean>>({});

  const tree = $derived.by((): Node => {
    const root: Node = { name: "", path: "", dirs: [], files: [] };
    for (const f of app.files) {
      const parts = f.path.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        let d = node.dirs.find((x) => x.name === parts[i]);
        if (!d) { d = { name: parts[i], path: parts.slice(0, i + 1).join("/"), dirs: [], files: [] }; node.dirs.push(d); }
        node = d;
      }
      const name = parts[parts.length - 1];
      node.files.push({ name: f.is_markdown ? name.replace(/\.md$/i, "") : name, path: f.path });
    }
    const sort = (n: Node) => { n.dirs.sort((a, b) => a.name.localeCompare(b.name)); n.files.sort((a, b) => a.name.localeCompare(b.name)); n.dirs.forEach(sort); };
    sort(root);
    return root;
  });

  async function newNote() {
    const base = "Untitled";
    let name = `${base}.md`;
    for (let i = 1; app.files.some((f) => f.path === name); i++) name = `${base} ${i}.md`;
    try { await createNote(name); await app.refresh(); await app.openNote(name); } catch (e) { app.say(errorMessage(e)); }
  }
</script>

{#snippet dir(node: Node, depth: number)}
  {#each node.dirs as d (d.path)}
    <button style="padding-left:{8 + depth * 12}px;font-weight:500" onclick={() => (collapsed[d.path] = !collapsed[d.path])}>
      {collapsed[d.path] ? "▸" : "▾"} {d.name}
    </button>
    {#if !collapsed[d.path]}{@render dir(d, depth + 1)}{/if}
  {/each}
  {#each node.files as f (f.path)}
    <button style="padding-left:{20 + depth * 12}px" class:active={app.activeTab?.path === f.path} onclick={() => app.openNote(f.path)}>{f.name}</button>
  {/each}
{/snippet}

<div class="pane-title" style="display:flex;justify-content:space-between;align-items:center">
  <span>Files</span>
  <button title="New note" onclick={newNote}>＋</button>
</div>
<div class="tree">{@render dir(tree, 0)}</div>
```

`ui/src/components/Tabs.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  const title = (p: string) => p.split("/").pop()!.replace(/\.md$/i, "");
</script>

<div class="tabs">
  {#each app.tabs as t, i (t.path)}
    <button class:active={i === app.active} onclick={() => (app.active = i)}>
      {title(t.path)}{t.text !== t.savedText ? " •" : ""}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.closeTab(i); }}>×</span>
    </button>
  {/each}
</div>
```

`ui/src/components/NoteView.svelte`:

```svelte
<script lang="ts">
  import { app, type Tab } from "../lib/state.svelte";
  import Editor from "./Editor.svelte";

  let { tab }: { tab: Tab } = $props();
  let timer: ReturnType<typeof setTimeout> | undefined;

  function onChange(text: string) {
    tab.text = text;
    clearTimeout(timer);
    timer = setTimeout(() => app.save(tab), 500);
  }
</script>

{#if tab.conflict}
  <div class="conflict">
    This file changed on disk while you had unsaved edits.
    <button onclick={() => app.resolveConflict(tab, false)}>Reload from disk</button>
    <button onclick={() => app.resolveConflict(tab, true)}>Keep mine</button>
  </div>
{/if}
<div class="note">
  <Editor text={tab.text} onchange={onChange} onblur={() => app.save(tab)} />
</div>
```

`ui/src/editor/theme.ts`:

```ts
import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

export const editorTheme = EditorView.theme({
  "&": { color: "var(--fg)", backgroundColor: "var(--bg)" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "var(--accent-bg) !important" },
  ".cm-activeLine": { backgroundColor: "transparent" },
  ".cm-gutters": { display: "none" },
});

export const markdownHighlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.heading1, fontSize: "1.8em", fontWeight: "700" },
    { tag: tags.heading2, fontSize: "1.5em", fontWeight: "700" },
    { tag: tags.heading3, fontSize: "1.25em", fontWeight: "600" },
    { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "600" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.strikethrough, textDecoration: "line-through" },
    { tag: tags.monospace, fontFamily: "var(--font-mono)", fontSize: "0.9em", background: "var(--bg-3)", borderRadius: "3px" },
    { tag: tags.link, color: "var(--accent)" },
    { tag: tags.url, color: "var(--fg-muted)" },
    { tag: tags.quote, color: "var(--fg-muted)", fontStyle: "italic" },
    { tag: tags.processingInstruction, color: "var(--fg-muted)" },
    { tag: tags.meta, color: "var(--fg-muted)" },
  ]),
);
```

`ui/src/components/Editor.svelte` (source mode only in this task; live preview arrives in Task 11):

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap, drawSelection, highlightActiveLine } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { markdown } from "@codemirror/lang-markdown";
  import { languages } from "@codemirror/language-data";
  import { editorTheme, markdownHighlight } from "../editor/theme";

  let { text, onchange, onblur }: { text: string; onchange: (t: string) => void; onblur: () => void } = $props();
  let host: HTMLDivElement;
  let view: EditorView;

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          markdown({ codeLanguages: languages }),
          editorTheme,
          markdownHighlight,
          EditorView.lineWrapping,
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => { if (u.docChanged) onchange(u.state.doc.toString()); }),
          EditorView.domEventHandlers({ blur: () => { onblur(); return false; } }),
        ],
      }),
    });
    return () => view.destroy();
  });

  // An external reload replaces the document; typing does not round-trip here
  // because `text` then already equals the editor's own content.
  $effect(() => {
    if (view && text !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
  });
</script>

<div bind:this={host} style="height:100%"></div>
```

`ui/src/components/StatusBar.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  let { showLeft = $bindable(), showRight = $bindable() }: { showLeft: boolean; showRight: boolean } = $props();
  const words = $derived(app.activeTab ? app.activeTab.text.split(/\s+/).filter(Boolean).length : 0);
</script>

<div class="statusbar">
  <button onclick={() => (showLeft = !showLeft)}>☰</button>
  <span>{app.files.filter((f) => f.is_markdown).length} notes</span>
  {#if app.activeTab}<span>{words} words</span>{/if}
  <span style="flex:1"></span>
  <button onclick={() => (showRight = !showRight)}>☰</button>
</div>
```

- [ ] **Step 7: Verify**

Run: `cd ui && pnpm check && pnpm test && pnpm build`
Expected: no errors. Then `cd .. && cargo tauri dev` (or `cd ui && pnpm tauri dev`): the window opens, a folder can be chosen, notes list and open, edits autosave, the tab shows a dot while unsaved. Verify by editing a note, waiting a second, and `cat`ing it in a terminal.

- [ ] **Step 8: Commit**

```bash
git add ui
git commit -m "feat(ui): svelte shell with vault picker, explorer, tabs and source editor"
```

---

### Task 11: Live preview, reading mode, link following, completions

**Files:**
- Create: `ui/src/editor/livePreview.ts`, `ui/src/editor/completions.ts`, `ui/src/lib/render.ts`, `ui/src/lib/render.test.ts`, `ui/src/components/Reading.svelte`
- Modify: `ui/src/components/Editor.svelte`, `ui/src/components/NoteView.svelte`, `ui/src/lib/state.svelte.ts`, `ui/src/app.css`

**Interfaces:**
- Consumes: `findWikilinks`, `displayText`, `api.titles/tags/resolveLink/createNote`, `app.openNote`.
- Produces: `livePreview(opts: { onFollow(target: string): void })` extension, `completions(titles, tags)` extension, `renderMarkdown(text: string): string`.

- [ ] **Step 1: Renderer with tests**

`ui/src/lib/render.ts`. An inline rule registered before markdown-it's own `link` rule consumes `[[...]]` and emits an anchor, so markdown-it never escapes it or mistakes it for a reference link.

```ts
import MarkdownIt from "markdown-it";
import type StateCore from "markdown-it/lib/rules_core/state_core.mjs";
import { findWikilinks, displayText } from "./wikilink";

const md = new MarkdownIt({ html: false, linkify: true });

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escapeAttr = (s: string) => escapeHtml(s).replace(/"/g, "&quot;");


function wikilinkHtml(raw: string): string {
  const l = findWikilinks(raw)[0];
  if (!l) return escapeHtml(raw);
  const target = l.heading ? `${l.target}#${l.heading}` : l.target;
  if (l.embed && /\.(png|jpe?g|gif|svg|webp)$/i.test(l.target)) {
    return `<img data-embed="${escapeAttr(l.target)}" alt="${escapeAttr(l.target)}">`;
  }
  return `<a class="wikilink" data-target="${escapeAttr(target)}">${escapeHtml(displayText(l))}</a>`;
}

md.inline.ruler.before("link", "wikilink", (state, silent) => {
  const src = state.src;
  if (src.charAt(state.pos) !== "[" && src.slice(state.pos, state.pos + 2) !== "![") return false;
  const start = src.charAt(state.pos) === "!" ? state.pos + 1 : state.pos;
  if (src.slice(start, start + 2) !== "[[") return false;
  const end = src.indexOf("]]", start + 2);
  if (end < 0) return false;
  if (!silent) {
    const tok = state.push("html_inline", "", 0);
    tok.content = wikilinkHtml(src.slice(state.pos, end + 2));
  }
  state.pos = end + 2;
  return true;
});

// #tags in text become spans.
const TAG = /(^|[\s(])#([\p{L}\p{N}_/-]*[\p{L}_/-][\p{L}\p{N}_/-]*)/gu;
md.core.ruler.push("tags", (state: StateCore) => {
  for (const block of state.tokens) {
    if (block.type !== "inline" || !block.children) continue;
    for (const t of block.children) {
      if (t.type !== "text" || !t.content.includes("#")) continue;
      const html = escapeHtml(t.content).replace(TAG, '$1<span class="tag" data-tag="$2">#$2</span>');
      if (html !== escapeHtml(t.content)) { t.type = "html_inline"; t.content = html; }
    }
  }
});

// Task items become checkboxes that carry their source line.
md.core.ruler.push("tasks", (state: StateCore) => {
  let itemLine = -1;
  for (const tok of state.tokens) {
    if (tok.type === "list_item_open") itemLine = tok.map?.[0] ?? -1;
    if (tok.type === "inline" && itemLine >= 0 && tok.children?.[0]?.type === "text") {
      const first = tok.children[0];
      const m = /^\[([ xX])\]\s/.exec(first.content);
      if (m) {
        first.content = first.content.slice(m[0].length);
        const cb = new state.Token("html_inline", "", 0);
        cb.content = `<input type="checkbox" data-line="${itemLine}"${m[1] === " " ? "" : " checked"}> `;
        tok.children.unshift(cb);
      }
      itemLine = -1;
    }
  }
});

// Callouts: a blockquote whose paragraph starts with `[!type] title`.
// Runs before inline parsing so the stripped marker never reaches it.
md.core.ruler.before("inline", "callouts", (state: StateCore) => {
  for (let i = 0; i < state.tokens.length - 2; i++) {
    const open = state.tokens[i];
    const inline = state.tokens[i + 2];
    if (open.type !== "blockquote_open" || inline.type !== "inline") continue;
    const m = /^\[!(\w+)\]\s*([^\n]*)\n?/.exec(inline.content);
    if (!m) continue;
    open.attrSet("class", `callout callout-${m[1].toLowerCase()}`);
    open.attrSet("data-title", m[2] || m[1]);
    inline.content = inline.content.slice(m[0].length);
  }
});

export function renderMarkdown(text: string): string {
  return md.render(text);
}
```

`ui/src/lib/render.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./render";

describe("renderMarkdown", () => {
  it("renders wikilinks as anchors with targets", () => {
    const h = renderMarkdown("see [[Note#Sec|shown]] and ![[pic.png]]");
    expect(h).toContain('<a class="wikilink" data-target="Note#Sec">shown</a>');
    expect(h).toContain('<img data-embed="pic.png"');
  });
  it("renders tags and checkboxes", () => {
    const h = renderMarkdown("- [x] done #work\n- [ ] todo");
    expect(h).toContain('<input type="checkbox" data-line="0" checked>');
    expect(h).toContain('<input type="checkbox" data-line="1">');
    expect(h).toContain('<span class="tag" data-tag="work">#work</span>');
  });
  it("renders callouts", () => {
    const h = renderMarkdown("> [!warning] Careful\n> body");
    expect(h).toContain('class="callout callout-warning"');
    expect(h).toContain('data-title="Careful"');
    expect(h).not.toContain("[!warning]");
  });
  it("escapes html", () => {
    expect(renderMarkdown("<script>x</script>")).not.toContain("<script>");
  });
});
```

Run: `cd ui && pnpm test` Expected: 6 passed (2 from Task 10, 4 here). If the `tasks` rule finds markdown-it already turned `[x]` into nothing, check that `MarkdownIt` was created without a task-list plugin; it has none by default.

- [ ] **Step 2: Live preview extension**

`ui/src/editor/livePreview.ts`:

```ts
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";
import { findWikilinks, displayText } from "../lib/wikilink";

// Lezer-markdown node names whose text is hidden on lines the cursor is not on.
const HIDDEN = new Set(["HeaderMark", "EmphasisMark", "CodeMark", "StrikethroughMark", "QuoteMark", "LinkMark", "URL"]);
const hide = Decoration.replace({});

class WikiWidget extends WidgetType {
  constructor(readonly text: string, readonly target: string) { super(); }
  eq(o: WikiWidget) { return o.text === this.text && o.target === this.target; }
  toDOM() {
    const a = document.createElement("a");
    a.className = "cm-wikilink";
    a.textContent = this.text;
    a.dataset.target = this.target;
    return a;
  }
  ignoreEvent() { return false; }
}

function activeLines(view: EditorView): Set<number> {
  const s = new Set<number>();
  for (const r of view.state.selection.ranges) {
    const a = view.state.doc.lineAt(r.from).number;
    const b = view.state.doc.lineAt(r.to).number;
    for (let i = a; i <= b; i++) s.add(i);
  }
  return s;
}

function build(view: EditorView): DecorationSet {
  const active = activeLines(view);
  const marks: { from: number; to: number; deco: Decoration }[] = [];
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from, to,
      enter(node) {
        if (node.name === "FencedCode" || node.name === "CodeBlock") return false;
        const line = view.state.doc.lineAt(node.from).number;
        if (HIDDEN.has(node.name) && !active.has(line)) {
          const trailingSpace = node.name === "HeaderMark" && view.state.sliceDoc(node.to, node.to + 1) === " ";
          marks.push({ from: node.from, to: trailingSpace ? node.to + 1 : node.to, deco: hide });
        }
      },
    });
    const text = view.state.sliceDoc(from, to);
    for (const l of findWikilinks(text)) {
      const a = from + l.from;
      const b = from + l.to;
      const line = view.state.doc.lineAt(a).number;
      const target = l.heading ? `${l.target}#${l.heading}` : l.target;
      marks.push(
        active.has(line)
          ? { from: a, to: b, deco: Decoration.mark({ class: "cm-wikilink-src" }) }
          : { from: a, to: b, deco: Decoration.replace({ widget: new WikiWidget(displayText(l), target) }) },
      );
    }
  }
  marks.sort((x, y) => x.from - y.from || x.to - y.to);
  const b = new RangeSetBuilder<Decoration>();
  let last = -1;
  for (const m of marks) {
    if (m.from < last) continue; // overlapping marks are dropped, never nested
    b.add(m.from, m.to, m.deco);
    last = m.to;
  }
  return b.finish();
}

export function livePreview(opts: { onFollow: (target: string) => void }) {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) { this.decorations = build(view); }
      update(u: ViewUpdate) {
        if (u.docChanged || u.viewportChanged || u.selectionSet) this.decorations = build(u.view);
      }
    },
    { decorations: (v) => v.decorations },
  );
  const clicks = EditorView.domEventHandlers({
    mousedown(e, view) {
      const el = (e.target as HTMLElement).closest?.("a.cm-wikilink") as HTMLElement | null;
      if (el?.dataset.target) { e.preventDefault(); opts.onFollow(el.dataset.target); return true; }
      if (!(e.ctrlKey || e.metaKey)) return false;
      const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
      if (pos == null) return false;
      const line = view.state.doc.lineAt(pos);
      const hit = findWikilinks(line.text).find((l) => pos >= line.from + l.from && pos <= line.from + l.to);
      if (!hit) return false;
      e.preventDefault();
      opts.onFollow(hit.heading ? `${hit.target}#${hit.heading}` : hit.target);
      return true;
    },
  });
  const style = EditorView.baseTheme({
    ".cm-wikilink, .cm-wikilink-src": { color: "var(--accent)", cursor: "pointer" },
    ".cm-wikilink:hover": { textDecoration: "underline" },
  });
  return [plugin, clicks, style];
}
```

- [ ] **Step 3: Completions**

`ui/src/editor/completions.ts`:

```ts
import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";

export function wikiCompletion(titles: () => [string, string][]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const m = ctx.matchBefore(/\[\[([^\]|#]*)$/);
    if (!m) return null;
    const q = m.text.slice(2).toLowerCase();
    const options = titles()
      .filter(([p, t]) => !q || t.toLowerCase().includes(q) || p.toLowerCase().includes(q))
      .slice(0, 50)
      .map(([p, t]) => {
        const stem = p.split("/").pop()!.replace(/\.md$/i, "");
        return { label: t, detail: p, apply: `[[${stem}]]` };
      });
    return { from: m.from, options, filter: false };
  };
}

export function tagCompletion(tags: () => string[]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const m = ctx.matchBefore(/(^|\s)#([\w/-]*)$/);
    if (!m) return null;
    const from = m.from + m.text.indexOf("#");
    return { from, options: tags().map((t) => ({ label: `#${t}` })), validFor: /^#[\w/-]*$/ };
  };
}

export const completions = (titles: () => [string, string][], tags: () => string[]) =>
  autocompletion({ override: [wikiCompletion(titles), tagCompletion(tags)], icons: false });
```

- [ ] **Step 4: Reading view**

`ui/src/components/Reading.svelte`:

```svelte
<script lang="ts">
  import { renderMarkdown } from "../lib/render";
  let { text, onFollow, onToggleTask }: { text: string; onFollow: (t: string) => void; onToggleTask: (line: number) => void } = $props();
  const html = $derived(renderMarkdown(text));

  function onClick(e: MouseEvent) {
    const el = e.target as HTMLElement;
    const a = el.closest("a.wikilink") as HTMLElement | null;
    if (a?.dataset.target) { e.preventDefault(); onFollow(a.dataset.target); return; }
    if (el instanceof HTMLInputElement && el.type === "checkbox") onToggleTask(Number(el.dataset.line));
  }
</script>

<!-- markdown-it runs with html disabled; only our own tags reach here -->
<div class="reading" onclick={onClick} role="presentation">{@html html}</div>

<style>
  .reading { max-width: 760px; margin: 0 auto; padding: 24px 32px; font-family: var(--font-text); font-size: 16px; }
  .reading :global(a.wikilink) { color: var(--accent); cursor: pointer; }
  .reading :global(.tag) { color: var(--accent); background: var(--accent-bg); border-radius: 10px; padding: 0 6px; font-size: .9em; }
  .reading :global(pre) { background: var(--bg-2); padding: 12px; border-radius: var(--radius); overflow: auto; font-family: var(--font-mono); font-size: .9em; }
  .reading :global(code) { font-family: var(--font-mono); font-size: .9em; background: var(--bg-3); border-radius: 3px; padding: 0 3px; }
  .reading :global(blockquote) { border-left: 3px solid var(--border); margin: 0; padding-left: 12px; color: var(--fg-muted); }
  .reading :global(.callout) { border-left-color: var(--accent); background: var(--accent-bg); padding: 8px 12px; border-radius: var(--radius); color: var(--fg); }
  .reading :global(.callout)::before { content: attr(data-title); display: block; font-weight: 600; margin-bottom: 4px; }
  .reading :global(table) { border-collapse: collapse; }
  .reading :global(td), .reading :global(th) { border: 1px solid var(--border); padding: 4px 8px; }
</style>
```

- [ ] **Step 5: Wire into Editor and NoteView**

`Editor.svelte` gains props and a compartment. Full replacement of its script:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, keymap, drawSelection, highlightActiveLine } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { markdown } from "@codemirror/lang-markdown";
  import { languages } from "@codemirror/language-data";
  import { editorTheme, markdownHighlight } from "../editor/theme";
  import { livePreview } from "../editor/livePreview";
  import { completions } from "../editor/completions";

  interface Props {
    text: string;
    mode: "live" | "source";
    onchange: (t: string) => void;
    onblur: () => void;
    onFollow: (target: string) => void;
    titles: () => [string, string][];
    tags: () => string[];
  }
  let { text, mode, onchange, onblur, onFollow, titles, tags }: Props = $props();
  let host: HTMLDivElement;
  let view: EditorView | undefined;
  const modeComp = new Compartment();
  const forMode = (m: string) => (m === "live" ? livePreview({ onFollow }) : []);

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          markdown({ codeLanguages: languages }),
          editorTheme,
          markdownHighlight,
          EditorView.lineWrapping,
          modeComp.of(forMode(mode)),
          completions(titles, tags),
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => { if (u.docChanged) onchange(u.state.doc.toString()); }),
          EditorView.domEventHandlers({ blur: () => { onblur(); return false; } }),
        ],
      }),
    });
    view.focus();
    return () => view?.destroy();
  });

  $effect(() => { view?.dispatch({ effects: modeComp.reconfigure(forMode(mode)) }); });

  // An external reload replaces the document. Typing does not loop through
  // here because `text` then already equals the editor's content.
  $effect(() => {
    if (view && text !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
  });
</script>

<div bind:this={host} style="height:100%"></div>
```

`NoteView.svelte` full replacement:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { app, type Tab } from "../lib/state.svelte";
  import { resolveLink, createNote, tags as apiTags } from "../lib/api";
  import Editor from "./Editor.svelte";
  import Reading from "./Reading.svelte";

  let { tab }: { tab: Tab } = $props();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let tagList = $state<string[]>([]);
  onMount(async () => { tagList = (await apiTags()).map((t) => t.tag); });

  function onChange(text: string) {
    tab.text = text;
    clearTimeout(timer);
    timer = setTimeout(() => app.save(tab), 500);
  }

  async function follow(target: string) {
    const [name] = target.split("#");
    let path = await resolveLink(name);
    if (!path) { path = `${name}.md`; await createNote(path); await app.refresh(); }
    await app.openNote(path);
  }

  function toggleTask(line: number) {
    const lines = tab.text.split("\n");
    if (line < 0 || line >= lines.length) return;
    lines[line] = lines[line].replace(/^(\s*[-*+]\s+)\[( |x|X)\]/, (_, p, c) => `${p}[${c === " " ? "x" : " "}]`);
    onChange(lines.join("\n"));
  }
</script>

{#if tab.conflict}
  <div class="conflict">
    This file changed on disk while you had unsaved edits.
    <button onclick={() => app.resolveConflict(tab, false)}>Reload from disk</button>
    <button onclick={() => app.resolveConflict(tab, true)}>Keep mine</button>
  </div>
{/if}
<div class="modes">
  {#each ["live", "source", "reading"] as m}
    <button class:active={tab.mode === m} onclick={() => (tab.mode = m as Tab["mode"])}>{m}</button>
  {/each}
</div>
<div class="note">
  {#if tab.mode === "reading"}
    <Reading text={tab.text} onFollow={follow} onToggleTask={toggleTask} />
  {:else}
    <Editor text={tab.text} mode={tab.mode} onchange={onChange} onblur={() => app.save(tab)} onFollow={follow} titles={() => app.titles} tags={() => tagList} />
  {/if}
</div>
```

Add to `app.css`:

```css
.modes { display: flex; gap: 4px; padding: 4px 8px; border-bottom: 1px solid var(--border); font-size: 12px; }
.modes button { padding: 2px 8px; border-radius: var(--radius); color: var(--fg-muted); text-transform: capitalize; }
.modes button.active { background: var(--bg-3); color: var(--fg); }
```

- [ ] **Step 6: Verify**

Run: `cd ui && pnpm check && pnpm test`, then `pnpm tauri dev`. In live mode `# Title` shows as a large heading with the hash hidden until the cursor enters the line; `[[Note]]` shows as a link and a click opens it; a link to a missing note creates it; typing `[[` offers titles; typing `#` offers tags; reading mode renders a callout and a checkbox click writes `[x]` to disk.

- [ ] **Step 7: Commit**

```bash
git add ui
git commit -m "feat(ui): live preview, reading mode, link following, completions"
```

---

### Task 12: Right sidebar, search, quick switcher, command palette, context menu

**Files:**
- Create: `ui/src/components/Backlinks.svelte`, `ui/src/components/Outgoing.svelte`, `ui/src/components/Properties.svelte`, `ui/src/components/Search.svelte`, `ui/src/components/Palette.svelte`, `ui/src/lib/commands.ts`
- Modify: `ui/src/App.svelte`, `ui/src/lib/state.svelte.ts`, `ui/src/components/Explorer.svelte`, `ui/src/components/StatusBar.svelte`, `ui/src/app.css`

**Interfaces:**
- Consumes: Task 9 commands through `api.ts`.
- Produces: `commands.ts` exporting `Command { id, name, hotkey, run }`, `allCommands()`, `chord(e)`; new `app` fields `showLeft`, `showRight`, `palette`, `leftPane`.

- [ ] **Step 1: State additions**

In `state.svelte.ts` add to the class:

```ts
  showLeft = $state(true);
  showRight = $state(true);
  palette = $state<"none" | "files" | "commands">("none");
  leftPane = $state<"files" | "search">("files");
```

and in `App.svelte` and `StatusBar.svelte` use `app.showLeft` / `app.showRight` instead of local state and bindable props.

- [ ] **Step 2: Commands**

`ui/src/lib/commands.ts`:

```ts
import { app } from "./state.svelte";
import { dailyNote, createNote } from "./api";

export interface Command { id: string; name: string; hotkey: string; run: () => void | Promise<void> }

async function newNote() {
  let n = "Untitled.md";
  for (let i = 1; app.files.some((f) => f.path === n); i++) n = `Untitled ${i}.md`;
  await createNote(n);
  await app.refresh();
  await app.openNote(n);
}

export const defaults: Command[] = [
  { id: "palette", name: "Open command palette", hotkey: "Ctrl+P", run: () => (app.palette = "commands") },
  { id: "switcher", name: "Quick switcher", hotkey: "Ctrl+O", run: () => (app.palette = "files") },
  { id: "search", name: "Search in all files", hotkey: "Ctrl+Shift+F", run: () => { app.leftPane = "search"; app.showLeft = true; } },
  { id: "new-note", name: "New note", hotkey: "Ctrl+N", run: newNote },
  { id: "daily", name: "Open today's daily note", hotkey: "Ctrl+D", run: async () => { const p = await dailyNote(); await app.refresh(); await app.openNote(p); } },
  { id: "close-tab", name: "Close current tab", hotkey: "Ctrl+W", run: () => { if (app.active >= 0) app.closeTab(app.active); } },
  { id: "toggle-mode", name: "Toggle live preview / source", hotkey: "Ctrl+E", run: () => { const t = app.activeTab; if (t) t.mode = t.mode === "source" ? "live" : "source"; } },
  { id: "toggle-reading", name: "Toggle reading view", hotkey: "Ctrl+Shift+E", run: () => { const t = app.activeTab; if (t) t.mode = t.mode === "reading" ? "live" : "reading"; } },
  { id: "toggle-left", name: "Toggle left sidebar", hotkey: "Ctrl+Shift+L", run: () => (app.showLeft = !app.showLeft) },
  { id: "toggle-right", name: "Toggle right sidebar", hotkey: "Ctrl+Shift+R", run: () => (app.showRight = !app.showRight) },
  { id: "save", name: "Save", hotkey: "Ctrl+S", run: () => { const t = app.activeTab; if (t) return app.save(t); } },
];

export function allCommands(): Command[] {
  const over = app.config?.hotkeys ?? {};
  return defaults.map((c) => ({ ...c, hotkey: over[c.id] ?? c.hotkey }));
}

export function chord(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return parts.join("+");
}
```

`App.svelte`'s `onKey` becomes:

```ts
import { allCommands, chord } from "./lib/commands";
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") { app.palette = "none"; return; }
  const cmd = allCommands().find((x) => x.hotkey === chord(e));
  if (cmd) { e.preventDefault(); void cmd.run(); }
}
```

- [ ] **Step 3: Palette**

`ui/src/components/Palette.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { createNote } from "../lib/api";

  interface Item { label: string; detail: string; run: () => void | Promise<void> }

  let q = $state("");
  let sel = $state(0);
  let input = $state<HTMLInputElement>();

  function score(t: string, n: string): number {
    const tl = t.toLowerCase();
    return tl === n ? 3 : tl.startsWith(n) ? 2 : tl.includes(n) ? 1 : 0;
  }

  const items = $derived.by((): Item[] => {
    const needle = q.toLowerCase().trim();
    if (app.palette === "commands") {
      return allCommands()
        .filter((c) => c.name.toLowerCase().includes(needle))
        .map((c) => ({ label: c.name, detail: c.hotkey, run: c.run }));
    }
    const notes: Item[] = app.titles
      .filter(([p, t]) => !needle || t.toLowerCase().includes(needle) || p.toLowerCase().includes(needle))
      .sort((a, b) => score(b[1], needle) - score(a[1], needle))
      .slice(0, 30)
      .map(([p, t]) => ({ label: t, detail: p, run: () => app.openNote(p) }));
    if (needle && !notes.some((n) => n.label.toLowerCase() === needle)) {
      const name = q.trim();
      notes.push({ label: `Create "${name}"`, detail: `${name}.md`, run: async () => { await createNote(`${name}.md`); await app.refresh(); await app.openNote(`${name}.md`); } });
    }
    return notes;
  });

  $effect(() => {
    if (app.palette !== "none") { q = ""; sel = 0; setTimeout(() => input?.focus()); }
  });

  async function choose(i: number) {
    const it = items[i];
    app.palette = "none";
    if (it) await it.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, items.length - 1); e.preventDefault(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter") { e.preventDefault(); void choose(sel); }
    else if (e.key === "Escape") { app.palette = "none"; }
    e.stopPropagation();
  }
</script>

{#if app.palette !== "none"}
  <div class="scrim" onclick={() => (app.palette = "none")} role="presentation">
    <div class="palette" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder={app.palette === "files" ? "Open note…" : "Run command…"} />
      <div class="items">
        {#each items as it, i (it.detail)}
          <button class:active={i === sel} onclick={() => choose(i)}><span>{it.label}</span><span class="detail">{it.detail}</span></button>
        {/each}
      </div>
    </div>
  </div>
{/if}
```

- [ ] **Step 4: Sidebar panes**

`ui/src/components/Backlinks.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { backlinks, type LinkRow } from "../lib/api";
  let rows = $state<LinkRow[]>([]);
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files; // re-run whenever the index changed
    if (p) backlinks(p).then((r) => (rows = r)); else rows = [];
  });
</script>

<div class="pane-title">Backlinks ({rows.length})</div>
{#each rows as r (r.src_path + r.line)}
  <button class="linkrow" onclick={() => app.openNote(r.src_path)}>
    <div class="src">{r.src_path.replace(/\.md$/i, "")}</div>
    <div class="ctx">{r.context}</div>
  </button>
{/each}
```

`ui/src/components/Outgoing.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { outgoing, createNote, type LinkRow } from "../lib/api";
  let rows = $state<LinkRow[]>([]);
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files;
    if (p) outgoing(p).then((r) => (rows = r.filter((l) => l.kind !== "embed"))); else rows = [];
  });
  async function open(r: LinkRow) {
    if (r.target_path) return app.openNote(r.target_path);
    const path = `${r.target_raw}.md`;
    await createNote(path);
    await app.refresh();
    await app.openNote(path);
  }
</script>

<div class="pane-title">Outgoing links ({rows.length})</div>
{#each rows as r (r.line + r.target_raw)}
  <button class="linkrow" onclick={() => open(r)}>
    <div class="src">{r.target_raw}{r.target_path ? "" : " (not created)"}</div>
    <div class="ctx">{r.context}</div>
  </button>
{/each}
```

`ui/src/components/Properties.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  let props = $state<Record<string, unknown>>({});
  let newKey = $state("");
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files;
    if (p) properties(p).then((r) => (props = r)); else props = {};
  });
  function parse(v: string): unknown {
    if (v === "") return null;
    if (v === "true") return true;
    if (v === "false") return false;
    if (/^-?\d+(\.\d+)?$/.test(v)) return Number(v);
    if (v.startsWith("[")) { try { return JSON.parse(v); } catch { return v; } }
    return v;
  }
  async function commit(key: string, raw: string) {
    const tab = app.activeTab;
    if (!tab) return;
    try {
      await app.save(tab); // a dirty buffer would otherwise conflict with the rewrite
      await setProperty(tab.path, key, parse(raw));
    } catch (e) { app.say(errorMessage(e)); }
  }
  const show = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));
</script>

<div class="pane-title">Properties</div>
<div class="props">
  {#each Object.entries(props) as [k, v] (k)}
    <span>{k}</span>
    <input value={show(v)} onchange={(e) => commit(k, (e.currentTarget as HTMLInputElement).value)} />
  {/each}
  <input placeholder="new key" bind:value={newKey} />
  <input placeholder="value" onchange={(e) => { if (newKey) { void commit(newKey, (e.currentTarget as HTMLInputElement).value); newKey = ""; } }} />
</div>
```

`ui/src/components/Search.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { search, type FtsHit } from "../lib/api";
  let q = $state("");
  let hits = $state<FtsHit[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => { hits = query.trim() ? await search(query) : []; }, 150);
  });
</script>

<div class="pane-title">Search</div>
<div style="padding:0 12px 8px"><input style="width:100%" placeholder="Search…" bind:value={q} /></div>
{#each hits as h (h.path)}
  <button class="linkrow" onclick={() => app.openNote(h.path)}>
    <div class="src">{h.title}</div>
    <!-- escaped in core; only <mark> survives -->
    <div class="ctx">{@html h.snippet}</div>
  </button>
{/each}
```

CSS to add to `app.css`:

```css
.scrim { position: fixed; inset: 0; background: rgba(0,0,0,.25); display: flex; justify-content: center; align-items: flex-start; padding-top: 12vh; z-index: 10; }
.palette { width: 560px; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 12px 40px rgba(0,0,0,.3); overflow: hidden; }
.palette input { width: 100%; border: none; border-bottom: 1px solid var(--border); border-radius: 0; padding: 10px 14px; font-size: 15px; }
.palette .items { max-height: 50vh; overflow: auto; }
.palette .items button { display: flex; justify-content: space-between; width: 100%; padding: 6px 14px; text-align: left; }
.palette .items button.active, .palette .items button:hover { background: var(--accent-bg); }
.palette .detail { color: var(--fg-muted); font-size: 12px; }
.linkrow { display: block; width: 100%; text-align: left; padding: 6px 12px; border-bottom: 1px solid var(--border); }
.linkrow:hover { background: var(--bg-3); }
.linkrow .src { font-weight: 500; }
.linkrow .ctx { color: var(--fg-muted); font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.props { padding: 4px 12px; display: grid; grid-template-columns: 1fr 2fr; gap: 4px 8px; font-size: 13px; }
.props input { padding: 2px 6px; min-width: 0; }
.panestrip { display: flex; border-bottom: 1px solid var(--border); }
.panestrip button { flex: 1; padding: 6px; color: var(--fg-muted); }
.panestrip button.active { color: var(--fg); border-bottom: 2px solid var(--accent); }
.menu { position: fixed; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: 0 6px 20px rgba(0,0,0,.2); z-index: 20; min-width: 160px; }
.menu button { display: block; width: 100%; text-align: left; padding: 6px 12px; }
.menu button:hover { background: var(--accent-bg); }
```

- [ ] **Step 5: Explorer context menu**

In `Explorer.svelte` add a right-click menu. State: `menu = $state<{ x: number; y: number; path: string; isDir: boolean } | null>(null)`. On `oncontextmenu` of a file or folder button: `e.preventDefault(); menu = { x: e.clientX, y: e.clientY, path, isDir }`. A `<svelte:window onclick={() => (menu = null)} />` closes it. Menu items:

```ts
import { ask, confirm } from "@tauri-apps/plugin-dialog";
import { createNote, createFolder, deleteFile, planRename, applyRename, errorMessage } from "../lib/api";

async function rename(path: string) {
  const to = window.prompt("New path", path);
  if (!to || to === path) return;
  try {
    const plan = await planRename(path, to);
    const ok = plan.affected.length === 0
      || (await ask(`Update links in ${plan.affected.length} file(s)?\n\n${plan.affected.join("\n")}`, { title: "Rename", kind: "info" }));
    if (!ok) return;
    await applyRename(plan);
    const tab = app.tabs.find((t) => t.path === path);
    if (tab) tab.path = plan.to;
    await app.refresh();
  } catch (e) { app.say(errorMessage(e)); }
}

async function remove(path: string) {
  if (!(await confirm(`Move "${path}" to the trash?`, { title: "Delete", kind: "warning" }))) return;
  try {
    await deleteFile(path);
    const i = app.tabs.findIndex((t) => t.path === path);
    if (i >= 0) app.closeTab(i);
    await app.refresh();
  } catch (e) { app.say(errorMessage(e)); }
}

async function newNoteIn(dir: string) {
  const name = window.prompt("Note name", "Untitled");
  if (!name) return;
  const path = dir ? `${dir}/${name}.md` : `${name}.md`;
  try { await createNote(path); await app.refresh(); await app.openNote(path); } catch (e) { app.say(errorMessage(e)); }
}

async function newFolderIn(dir: string) {
  const name = window.prompt("Folder name", "New folder");
  if (!name) return;
  try { await createFolder(dir ? `${dir}/${name}` : name); await app.refresh(); } catch (e) { app.say(errorMessage(e)); }
}
```

Folder rename and delete are out of scope for this task: the menu for a folder offers only *New note* and *New folder*. `window.prompt` works in the Tauri webview on all three platforms; a proper inline rename comes with the graph-and-bases plan's polish task.

- [ ] **Step 6: Assemble `App.svelte`**

```svelte
<script lang="ts">
  import { app } from "./lib/state.svelte";
  import { allCommands, chord } from "./lib/commands";
  import VaultPicker from "./components/VaultPicker.svelte";
  import Explorer from "./components/Explorer.svelte";
  import Search from "./components/Search.svelte";
  import Tabs from "./components/Tabs.svelte";
  import NoteView from "./components/NoteView.svelte";
  import Backlinks from "./components/Backlinks.svelte";
  import Outgoing from "./components/Outgoing.svelte";
  import Properties from "./components/Properties.svelte";
  import Palette from "./components/Palette.svelte";
  import StatusBar from "./components/StatusBar.svelte";

  $effect(() => {
    const t = app.config?.theme ?? "system";
    if (t === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") { app.palette = "none"; return; }
    const cmd = allCommands().find((x) => x.hotkey === chord(e));
    if (cmd) { e.preventDefault(); void cmd.run(); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if !app.root}
  <VaultPicker />
{:else}
  <div class="layout" class:no-left={!app.showLeft} class:no-right={!app.showRight}>
    <aside class="sidebar">
      {#if app.showLeft}
        <div class="panestrip">
          <button class:active={app.leftPane === "files"} onclick={() => (app.leftPane = "files")}>Files</button>
          <button class:active={app.leftPane === "search"} onclick={() => (app.leftPane = "search")}>Search</button>
        </div>
        {#if app.leftPane === "files"}<Explorer />{:else}<Search />{/if}
      {/if}
    </aside>
    <main class="centre">
      <Tabs />
      {#if app.activeTab}
        {#key app.activeTab.path}<NoteView tab={app.activeTab} />{/key}
      {:else}
        <div class="note" style="display:grid;place-items:center;color:var(--fg-muted)">No note open</div>
      {/if}
    </main>
    <aside class="sidebar right">
      {#if app.showRight}<Backlinks /><Outgoing /><Properties />{/if}
    </aside>
    <StatusBar />
  </div>
  <Palette />
{/if}
{#if app.toast}<div class="toast">{app.toast}</div>{/if}
```

- [ ] **Step 7: Verify**

`cd ui && pnpm check && pnpm test`, then `pnpm tauri dev`: Ctrl+O opens the switcher and Enter opens a note, a new name offers Create; Ctrl+P lists commands and runs them; Ctrl+Shift+F searches with highlighted snippets; the right sidebar shows backlinks with context and clicking one opens it; outgoing links open or create; editing a property rewrites the frontmatter and the editor shows it; right-click rename lists the affected files and rewrites them; right-click delete moves the file to the trash; Ctrl+D opens today's note under `Daily/`.

- [ ] **Step 8: Commit**

```bash
git add ui
git commit -m "feat(ui): backlinks, outgoing, properties, search, palettes, context menu"
```

---

### Task 13: Release workflow, smoke checklist, README

**Files:**
- Create: `.github/workflows/release.yml`, `docs/smoke.md`
- Modify: `README.md`

- [ ] **Step 1: Release workflow**

`.github/workflows/release.yml`:

```yaml
name: release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-22.04
            args: ""
          - platform: windows-latest
            args: ""
          - platform: macos-latest
            args: "--target universal-apple-darwin"
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          version: 11
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: ui/pnpm-lock.yaml
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - uses: Swatinem/rust-cache@v2
      - name: Linux system libraries
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
      - run: pnpm install --frozen-lockfile
        working-directory: ui
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: src-tauri
          tagName: ${{ github.ref_name }}
          releaseName: engram-notes ${{ github.ref_name }}
          releaseDraft: true
          prerelease: true
          args: ${{ matrix.args }}
```

The release is created as a draft so a failed platform can be retried before anything is public. Publishing is one click on GitHub. Tagging: bump `version` in the root `Cargo.toml` and `ui/package.json`, commit, `git tag v0.1.0`, push the tag.

- [ ] **Step 2: Smoke checklist**

`docs/smoke.md`:

```markdown
# Smoke checklist

Run on a real vault before tagging a release. Every line is a yes or the tag waits.

1. Open a vault of at least 500 notes. The second open takes under two seconds.
2. The explorer shows folders collapsed and expanded, notes without `.md`.
3. Open a note. Live preview hides `#` on headings not under the cursor.
4. Type `[[`, pick a note; the link renders and a click opens it.
5. Click a link to a missing note: the note is created and opened.
6. Edit, wait one second, read the file in a terminal: the edit is there.
7. Edit the file in a terminal while the tab is clean: the tab reloads.
8. Edit the file in a terminal while the tab is dirty: the conflict bar appears and both choices work.
9. The backlinks pane lists referrers with line context; clicking opens.
10. Rename a note with three referrers: the dialog lists them, all three are rewritten.
11. Ctrl+Shift+F finds a word in a body and a word in a title; the title hit is first.
12. Ctrl+O, part of a title, Enter opens it; an unknown name offers Create.
13. Ctrl+D creates and opens today's daily note.
14. Edit a property in the right pane: the frontmatter in the editor updates.
15. Reading mode renders a callout; a checkbox toggles and writes to disk.
16. Dark and light themes are both readable in editor, reading view and sidebars.
17. `cargo test --workspace` and `pnpm check && pnpm test` pass.
```

- [ ] **Step 3: README build section**

Add to `README.md` after the status paragraph:

```markdown
## Build

Rust stable, Node 22, pnpm 11, and on Linux the webkit2gtk development
packages: `webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel` on
Fedora, `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev` on Debian and
Ubuntu.

    cd ui && pnpm install && pnpm tauri dev      # run
    cd ui && pnpm tauri build                    # installers under target/release/bundle
    cargo test --workspace                       # the core tests need no window
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml docs/smoke.md README.md
git commit -m "ci: release installers on tags; docs: smoke checklist and build notes"
```
