# Semantic search and memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hybrid full-text and semantic search with a relevance divider, a memory
that decays, and semantic edges in the graph.

**Architecture:** `core` gains three modules: `embed` (an `Embedder` trait, a
deterministic fake, and a fastembed implementation behind a cargo feature),
`search` (passages, a brute-force vector scan, reciprocal rank fusion and
engram's cliff divider) and `memory` (events, sittings, decayed activation and
associations, priming and spread). `src-tauri` runs one background thread that
embeds passages in batches and reports its state. The frontend draws the
divider, an associated band, a Related pane, the queue in the status bar and
dashed semantic edges in the graph.

**Tech Stack:** Rust 2024, rusqlite (bundled SQLite), fastembed 6.1 with
`multilingual-e5-small`, Tauri 2, Svelte 5, TypeScript, CodeMirror 6, d3-force.

**Spec:** `docs/superpowers/specs/2026-09-12-engram-notes-design.md`, sections
*Search*, *Memory*, and *The graph* → *Semantic edges*.

**Handoff this plan continues:** `docs/superpowers/plans/2026-09-13-handoff.md`.

## Global Constraints

- Rust 2024 edition, stable toolchain. `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` stay clean.
- `core` has no Tauri dependency. `src-tauri` is a thin shell. The frontend
  never touches the filesystem.
- **Tests run without a model, a window or the network.** `cargo test
  --workspace` must never load an ONNX file or reach the internet: fastembed
  lives behind the non-default cargo feature `fastembed`, and every core test
  uses `FakeEmbedder`.
- Files are the truth. A schema bump rebuilds rather than migrates.
- Comments short: why, not what. One or two lines is the norm.
- Where a feature comes from engram, match engram's concepts. Where it comes
  from Obsidian, match Obsidian's behaviour and look.
- Errors are typed in `core::Error`; the UI shows them, never swallows them.
- Commit messages: conventional prefix, imperative subject under 72 characters.
- Every UI change is verified with `node ui/scripts/shot.mjs <fixture> <out.png>`
  before it is called done (handoff, *Checking the app*).

## Departures from the spec, decided before writing

1. **The divider is engram's cliff, not a fraction of the top score.** Over
   cosine similarities sorted descending, at least three of them, the largest
   gap must exceed `3.0 ×` the mean of the other gaps and `0.01 ×` the top
   score; everything from there on is past the divider. It reads similarity,
   never the fused RRF score, whose first gap is structurally the largest.
   AGENTS.md: engram is the reference for concepts it inspired.
2. **The model is not quantized.** fastembed 6.1 has no quantized e5-small, so
   the default is `EmbeddingModel::MultilingualE5Small`. A quantized ONNX loads
   through the *model directory* setting, which exists anyway for air-gapped
   machines.
3. **`vectors` is keyed by the passage text hash**, not by a passage id, so an
   unchanged passage in a renamed or re-saved note keeps its vector. The model
   id lives in `meta`; changing it clears `vectors`.
4. **No `sittings` table.** A sitting is a gap, so `events.sitting` is an
   integer that increments when more than `sitting_gap_secs` passed.
5. **`activation` and `assoc` have no foreign key to `notes`.** A note deleted
   and restored keeps its memory; queries join `notes`, so stale rows never
   surface. A schema bump still drops them with the rest of the index — the
   spec accepts that derived state is rebuildable, and memory is the one part
   that is not; `docs/memory.md` says so.

---

## File Structure

**Created in `core`:**

- `core/src/search/mod.rs` — `Hit`, `SearchResults`, `Associated`, `hybrid()`.
- `core/src/search/passages.rs` — `Passage`, `split()`. Pure, no I/O.
- `core/src/search/vector.rs` — blob encoding, `Index` methods for vectors.
- `core/src/search/fuse.rs` — `rrf()`, `cliff()`, `mark_past_divider()`. Pure.
- `core/src/embed/mod.rs` — `Embedder` trait, `FakeEmbedder`.
- `core/src/embed/fastembed.rs` — `FastEmbedder`, behind feature `fastembed`.
- `core/src/memory/mod.rs` — `EventKind`, `AssocRow`, `Index` methods.
- `core/src/memory/decay.rs` — `decayed()`. Pure.
- `core/src/memory/prime.rs` — `prime()`. Pure.
- `core/src/memory/spread.rs` — `spread()`.
- `core/src/memory/related.rs` — `Related`, `SimilarNote`, `related()`.

**Modified in `core`:** `index/schema.sql` (five tables), `index/mod.rs`
(`SCHEMA_VERSION` to `"3"`), `index/rebuild.rs` (write passages),
`index/fts.rs` (`FtsHit` gains `heading` and `line`), `config.rs` (three config
structs), `error.rs` (`Embed`), `graph.rs` (`semantic_edges()`),
`rename.rs` (move memory rows), `lib.rs` (three `pub mod`), `Cargo.toml`.

**Created in `src-tauri`:** `src/embed.rs` — the embed thread and its status.
**Modified:** `src/commands.rs`, `src/lib.rs`, `src/state.rs`, `Cargo.toml`.

**Created in `ui`:** `src/components/Related.svelte`,
`src/lib/semantic.ts` + `src/lib/semantic.test.ts` (pure edge merging).
**Modified:** `src/lib/api.ts`, `src/lib/state.svelte.ts`,
`src/lib/graph.ts` (+ its test), `src/lib/commands.ts`,
`src/components/Search.svelte`, `src/components/StatusBar.svelte`,
`src/components/GraphView.svelte`, `src/components/Editor.svelte`,
`src/components/NoteView.svelte`, `src/App.svelte`, `src/app.css`.

**Created in `docs`:** `docs/memory.md`. **Modified:** `docs/smoke.md`.

---

### Task 1: Schema version 3 and passages

**Files:**
- Create: `core/src/search/passages.rs`, `core/src/search/mod.rs`
- Modify: `core/src/index/schema.sql`, `core/src/index/mod.rs:19`,
  `core/src/index/rebuild.rs`, `core/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` in `passages.rs` and `rebuild.rs`

**Interfaces:**
- Consumes: `crate::index::Index`, `crate::parse::parse`.
- Produces: `search::passages::{Passage, split, MAX_CHARS}`; `Passage` has
  `ordinal: usize`, `heading: String`, `line: u32`, `text: String`, and methods
  `embed_text(&self) -> String` and `hash(&self) -> String`. Table `passages`
  is written by `write_note`.

- [ ] **Step 1: Write the failing test for splitting**

Create `core/src/search/passages.rs` with only this test module at the bottom
(the code comes in step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_group_under_their_heading_path() {
        let body = "intro line\n\n# One\nalpha\n\n## Two\nbeta\n\ngamma\n";
        let p = split(body);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].heading, "");
        assert_eq!(p[0].text, "intro line");
        assert_eq!(p[0].line, 1);
        assert_eq!(p[1].heading, "One");
        assert_eq!(p[1].text, "alpha");
        assert_eq!(p[1].line, 4);
        assert_eq!(p[2].heading, "One > Two");
        assert_eq!(p[2].text, "beta\n\ngamma");
        assert_eq!(p[2].ordinal, 2);
    }

    #[test]
    fn a_hash_inside_a_fence_is_not_a_heading() {
        let body = "# H\n```\n# not a heading\n```\ntail\n";
        let p = split(body);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "H");
        assert!(p[0].text.contains("# not a heading"));
    }

    #[test]
    fn long_text_is_cut_at_whitespace_below_the_limit() {
        let word = "word ".repeat(400); // 2000 chars
        let p = split(&word);
        assert!(p.len() >= 2);
        assert!(p.iter().all(|x| x.text.chars().count() <= MAX_CHARS));
        assert_eq!(p[1].ordinal, 1);
    }

    #[test]
    fn the_heading_path_is_embedded_and_hashed_with_the_text() {
        let a = split("# H\nbody\n");
        let b = split("# Other\nbody\n");
        assert_eq!(a[0].embed_text(), "H\n\nbody");
        assert_ne!(a[0].hash(), b[0].hash());
        assert_eq!(a[0].hash(), split("# H\nbody\n")[0].hash());
    }

    #[test]
    fn an_empty_body_has_no_passages() {
        assert!(split("   \n\n").is_empty());
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Add `pub mod search;` to `core/src/lib.rs` (alphabetically after `rename`) and
create `core/src/search/mod.rs` containing `pub mod passages;`.

Run: `cargo test -p engram-notes-core passages`
Expected: FAIL — `cannot find function split in this scope`.

- [ ] **Step 3: Write the splitter**

Put this above the test module in `core/src/search/passages.rs`:

```rust
//! A note's body cut into the pieces that get embedded.

use sha2::{Digest, Sha256};

/// Roughly the model's window in characters; e5-small takes 512 tokens.
pub const MAX_CHARS: usize = 1200;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Passage {
    pub ordinal: usize,
    /// The headings above it, joined with ` > `; empty above the first.
    pub heading: String,
    /// 1-based line in the body, not in the file.
    pub line: u32,
    pub text: String,
}

impl Passage {
    /// What is embedded: the heading path gives the passage its context.
    pub fn embed_text(&self) -> String {
        if self.heading.is_empty() {
            self.text.clone()
        } else {
            format!("{}\n\n{}", self.heading, self.text)
        }
    }

    pub fn hash(&self) -> String {
        hex::encode(Sha256::digest(self.embed_text().as_bytes()))
    }
}

fn heading_of(line: &str) -> Option<(u8, String)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    rest.starts_with(' ')
        .then(|| (hashes as u8, rest.trim().to_owned()))
}

fn path_of(stack: &[(u8, String)]) -> String {
    stack
        .iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join(" > ")
}

fn flush(buf: &mut String, line: u32, heading: &str, out: &mut Vec<Passage>) {
    let text = buf.trim();
    if !text.is_empty() {
        for piece in cut(text) {
            out.push(Passage {
                ordinal: out.len(),
                heading: heading.to_owned(),
                line,
                text: piece.to_owned(),
            });
        }
    }
    buf.clear();
}

/// Cut `text` so no piece is longer than `MAX_CHARS`, preferring whitespace.
fn cut(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.chars().count() > MAX_CHARS {
        let hard = rest
            .char_indices()
            .nth(MAX_CHARS)
            .map_or(rest.len(), |(i, _)| i);
        let at = rest[..hard].rfind(char::is_whitespace).unwrap_or(hard);
        let (head, tail) = rest.split_at(at);
        out.push(head.trim_end());
        rest = tail.trim_start();
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

/// Split on headings, then on paragraphs, into pieces of at most `MAX_CHARS`.
pub fn split(body: &str) -> Vec<Passage> {
    let mut out: Vec<Passage> = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut buf = String::new();
    let mut buf_line = 0u32;
    let mut fence: Option<u8> = None;

    for (i, line) in body.lines().enumerate() {
        let no = i as u32 + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let f = trimmed.as_bytes()[0];
            fence = match fence {
                Some(open) if open == f => None,
                other => other.or(Some(f)),
            };
        }
        if fence.is_none() {
            if let Some((level, text)) = heading_of(trimmed) {
                flush(&mut buf, buf_line, &path_of(&stack), &mut out);
                stack.retain(|(l, _)| *l < level);
                stack.push((level, text));
                buf_line = 0;
                continue;
            }
            // A blank line closes a paragraph; a full buffer closes it early.
            if line.trim().is_empty() {
                if buf.chars().count() >= MAX_CHARS / 2 {
                    flush(&mut buf, buf_line, &path_of(&stack), &mut out);
                    buf_line = 0;
                }
                if !buf.is_empty() {
                    buf.push('\n');
                }
                continue;
            }
        }
        if buf_line == 0 {
            buf_line = no;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    flush(&mut buf, buf_line, &path_of(&stack), &mut out);
    out
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core passages`
Expected: PASS, 5 tests.

- [ ] **Step 5: Add the five tables to the schema**

Append to `core/src/index/schema.sql`:

```sql
-- Semantic search and memory. `vectors` is keyed by the passage text hash, so
-- an unchanged passage in a renamed or re-saved note keeps its embedding.
CREATE TABLE IF NOT EXISTS passages (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL,
  heading TEXT NOT NULL,
  line INTEGER NOT NULL,
  text TEXT NOT NULL,
  hash TEXT NOT NULL,
  PRIMARY KEY (path, ordinal)
);
CREATE INDEX IF NOT EXISTS passages_hash ON passages(hash);

CREATE TABLE IF NOT EXISTS vectors (
  hash TEXT PRIMARY KEY,
  dim INTEGER NOT NULL,
  embedding BLOB NOT NULL
);

-- Memory outlives a deleted note on purpose: no foreign key, and queries join
-- `notes`, so a note deleted by accident keeps what it learned.
CREATE TABLE IF NOT EXISTS activation (
  path TEXT PRIMARY KEY,
  value REAL NOT NULL,
  stamped_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS assoc (
  a_path TEXT NOT NULL,
  b_path TEXT NOT NULL,
  value REAL NOT NULL,
  stamped_at INTEGER NOT NULL,
  queries TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (a_path, b_path)
);
CREATE INDEX IF NOT EXISTS assoc_b ON assoc(b_path);

CREATE TABLE IF NOT EXISTS events (
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  path TEXT,
  query TEXT,
  sitting INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS events_at ON events(at);
```

In `core/src/index/mod.rs` change `pub const SCHEMA_VERSION: &str = "2";` to
`"3"`.

- [ ] **Step 6: Write the failing test for passages in the index**

Add to the test module in `core/src/index/rebuild.rs`:

```rust
    #[test]
    fn rebuild_writes_passages_and_an_edit_replaces_them() {
        let (d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        assert_eq!(count(&ix, "SELECT count(*) FROM passages WHERE path='A.md'"), 1);
        let heading: String = ix
            .conn()
            .query_row("SELECT heading FROM passages WHERE path='A.md'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(heading, "A");
        fs::write(d.path().join("A.md"), "# X\none\n\n# Y\ntwo").unwrap();
        ix.update_file(&v, "A.md").unwrap();
        let rows: Vec<String> = ix
            .conn()
            .prepare("SELECT heading FROM passages WHERE path='A.md' ORDER BY ordinal")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec!["X".to_string(), "Y".to_string()]);
    }
```

- [ ] **Step 7: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core rebuild_writes_passages`
Expected: FAIL — `no such table: passages` is gone after step 5, so it fails on
the count being 0.

- [ ] **Step 8: Write passages in `write_note`**

In `core/src/index/rebuild.rs`, at the end of `write_note` (after the `blocks`
loop, before `Ok(())`):

```rust
    let mut ins = tx.prepare_cached(
        "INSERT INTO passages(path, ordinal, heading, line, text, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for p in crate::search::passages::split(&note.body) {
        ins.execute(params![
            entry.path,
            p.ordinal as i64,
            p.heading,
            p.line,
            p.text,
            p.hash()
        ])?;
    }
```

The `DELETE FROM notes` at the top of `write_note` cascades passages away, so
nothing else has to clear them.

- [ ] **Step 9: Run the tests**

Run: `cargo test -p engram-notes-core`
Expected: PASS, all existing tests plus the two new ones.

- [ ] **Step 10: Commit**

```bash
git add core/src/search core/src/index core/src/lib.rs
git commit -m "feat(index): schema 3 with passages, vectors and memory tables"
```

---

### Task 2: The Embedder trait and the fake

**Files:**
- Create: `core/src/embed/mod.rs`
- Modify: `core/src/error.rs`, `core/src/lib.rs`
- Test: inline in `core/src/embed/mod.rs`

**Interfaces:**
- Produces: `embed::Embedder` with `fn id(&self) -> String`,
  `fn dim(&self) -> usize`,
  `fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>`,
  `fn embed_query(&mut self, text: &str) -> Result<Vec<f32>>`;
  `embed::FakeEmbedder::new(dim: usize)`; `Error::Embed(String)`.

`embed` takes `&mut self` because fastembed's `TextEmbedding::embed` does.

- [ ] **Step 1: Write the failing test**

Create `core/src/embed/mod.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_is_deterministic_and_normalised() {
        let mut e = FakeEmbedder::new(64);
        assert_eq!(e.dim(), 64);
        assert_eq!(e.id(), "fake-64");
        let a = e.embed_documents(&["rust ownership".to_string()]).unwrap();
        let b = e.embed_documents(&["rust ownership".to_string()]).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0].len(), 64);
        let norm: f32 = a[0].iter().map(|x| x * x).sum();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}");
    }

    #[test]
    fn shared_words_score_higher_than_strangers() {
        let mut e = FakeEmbedder::new(64);
        let v = e
            .embed_documents(&[
                "rust ownership rules".to_string(),
                "ownership rules matter".to_string(),
                "coffee brewing kettle".to_string(),
            ])
            .unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&v[0], &v[1]) > dot(&v[0], &v[2]));
    }

    #[test]
    fn a_query_embeds_like_a_document() {
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("rust").unwrap();
        let d = e.embed_documents(&["rust".to_string()]).unwrap();
        assert_eq!(q, d[0]);
    }

    #[test]
    fn empty_text_is_a_zero_vector_rather_than_a_nan() {
        let mut e = FakeEmbedder::new(8);
        assert_eq!(e.embed_query("").unwrap(), vec![0.0; 8]);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Add `pub mod embed;` to `core/src/lib.rs` (before `error`).

Run: `cargo test -p engram-notes-core embed`
Expected: FAIL — `cannot find type FakeEmbedder`.

- [ ] **Step 3: Write the trait and the fake**

Above the tests in `core/src/embed/mod.rs`:

```rust
//! The embedding seam. One trait, a deterministic fake for tests, and
//! fastembed behind the `fastembed` feature so tests need no model.

use crate::Result;

#[cfg(feature = "fastembed")]
pub mod fastembed;

pub trait Embedder: Send {
    /// Identifies the vectors in the index; changing it clears them.
    fn id(&self) -> String;
    fn dim(&self) -> usize;
    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>>;
}

/// Hashes words into a vector. No model, no network, same answer every run.
pub struct FakeEmbedder {
    dim: usize,
}

impl FakeEmbedder {
    pub fn new(dim: usize) -> FakeEmbedder {
        FakeEmbedder { dim }
    }
}

fn fnv(word: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in word.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub(crate) fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}

impl Embedder for FakeEmbedder {
    fn id(&self) -> String {
        format!("fake-{}", self.dim)
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0.0f32; self.dim];
                for word in t.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
                    if word.is_empty() {
                        continue;
                    }
                    let h = fnv(word);
                    v[(h % self.dim as u64) as usize] += 1.0;
                    v[((h >> 32) % self.dim as u64) as usize] += 0.5;
                }
                normalise(&mut v);
                v
            })
            .collect())
    }

    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        Ok(self.embed_documents(&[text.to_owned()])?.pop().unwrap())
    }
}
```

- [ ] **Step 4: Add the error variant**

In `core/src/error.rs`, after the `Base` variant:

```rust
    #[error("embedding error: {0}")]
    Embed(String),
```

In `src-tauri/src/error.rs`, add `Error::Embed(_) => "embed",` to the `match`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p engram-notes-core embed && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, 4 tests; clippy clean.

- [ ] **Step 6: Commit**

```bash
git add core/src/embed core/src/error.rs core/src/lib.rs src-tauri/src/error.rs
git commit -m "feat(embed): the Embedder trait and a deterministic fake"
```

---

### Task 3: Vectors in the index and the cosine scan

**Files:**
- Create: `core/src/search/vector.rs`
- Modify: `core/src/search/mod.rs`
- Test: inline in `core/src/search/vector.rs`

**Interfaces:**
- Consumes: `search::passages`, `embed::Embedder`, `index::Index`.
- Produces, all on `Index`: `model_id()`, `set_model_id(&str)`,
  `pending_vectors(limit) -> Vec<Pending>`, `pending_count() -> usize`,
  `put_vectors(&[(String, Vec<f32>)])`, `search_vectors(&[f32], limit) ->
  Vec<VecHit>`, `similar_to(paths: &[String], limit) -> Vec<VecHit>`,
  `vector_of(hash) -> Option<Vec<f32>>`. `Pending { hash, text }`.
  `VecHit { path, heading, line, text, similarity }`.

- [ ] **Step 1: Write the failing test**

Create `core/src/search/vector.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn embedded() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Rust.md"), "# Rust\nownership rules keep memory safe").unwrap();
        fs::write(d.path().join("Coffee.md"), "# Coffee\nkettle water grind beans").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(100).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending
            .into_iter()
            .map(|p| p.hash)
            .zip(vecs)
            .collect();
        ix.put_vectors(&rows).unwrap();
        (d, ix)
    }

    #[test]
    fn pending_lists_each_passage_once_and_empties_as_vectors_arrive() {
        let (_d, ix) = embedded();
        assert_eq!(ix.pending_count().unwrap(), 0);
        assert!(ix.pending_vectors(10).unwrap().is_empty());
    }

    #[test]
    fn the_scan_ranks_by_cosine_and_carries_the_passage() {
        let (_d, ix) = embedded();
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("ownership rules").unwrap();
        let hits = ix.search_vectors(&q, 10).unwrap();
        assert_eq!(hits[0].path, "Rust.md");
        assert_eq!(hits[0].heading, "Rust");
        assert_eq!(hits[0].line, 2);
        assert!(hits[0].text.contains("ownership"));
        assert!(hits[0].similarity > hits[1].similarity);
    }

    #[test]
    fn a_new_model_id_clears_the_vectors() {
        let (_d, mut ix) = embedded();
        ix.set_model_id("other").unwrap();
        assert_eq!(ix.model_id().unwrap().as_deref(), Some("other"));
        assert!(ix.pending_count().unwrap() > 0);
    }

    #[test]
    fn similar_to_skips_the_note_itself() {
        let (_d, ix) = embedded();
        let hits = ix.similar_to(&["Rust.md".to_string()], 10).unwrap();
        assert!(hits.iter().all(|h| h.path != "Rust.md"));
        assert_eq!(hits[0].path, "Coffee.md");
    }

    #[test]
    fn a_deleted_note_leaves_no_passage_behind() {
        let (d, mut ix) = embedded();
        fs::remove_file(d.path().join("Coffee.md")).unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("kettle").unwrap();
        assert!(ix.search_vectors(&q, 10).unwrap().iter().all(|h| h.path != "Coffee.md"));
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Add `pub mod vector;` to `core/src/search/mod.rs`.

Run: `cargo test -p engram-notes-core vector`
Expected: FAIL — `no method named set_model_id`.

- [ ] **Step 3: Write the vector store**

Above the tests in `core/src/search/vector.rs`:

```rust
//! Vectors in SQLite blobs and a brute-force cosine scan over them.

use crate::Result;
use crate::index::Index;
use rusqlite::{OptionalExtension, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub hash: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct VecHit {
    pub path: String,
    pub heading: String,
    /// 1-based line in the file.
    pub line: u32,
    pub text: String,
    pub similarity: f32,
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// Vectors are stored normalised, so the cosine is a dot product.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// The line a passage starts on in the file, not in the body.
const HIT_COLUMNS: &str =
    "p.path, p.heading, p.line + n.body_line - 1, p.text, v.embedding";

impl Index {
    pub fn model_id(&self) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM meta WHERE key='model'", [], |r| r.get(0))
            .optional()?)
    }

    /// Records the model. A different one makes every vector meaningless.
    pub fn set_model_id(&self, id: &str) -> Result<()> {
        if self.model_id()?.as_deref() != Some(id) {
            self.conn().execute("DELETE FROM vectors", [])?;
        }
        self.conn().execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('model', ?1)",
            [id],
        )?;
        Ok(())
    }

    /// Distinct passage texts with no vector yet, longest note first is not
    /// worth it: oldest path order keeps the queue predictable.
    pub fn pending_vectors(&self, limit: usize) -> Result<Vec<Pending>> {
        Ok(self
            .conn()
            .prepare(
                "SELECT p.hash, CASE WHEN p.heading = '' THEN p.text
                                     ELSE p.heading || char(10) || char(10) || p.text END
                 FROM passages p LEFT JOIN vectors v ON v.hash = p.hash
                 WHERE v.hash IS NULL
                 GROUP BY p.hash ORDER BY min(p.path), min(p.ordinal) LIMIT ?1",
            )?
            .query_map([limit as i64], |r| {
                Ok(Pending {
                    hash: r.get(0)?,
                    text: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn pending_count(&self) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT count(DISTINCT p.hash) FROM passages p
             LEFT JOIN vectors v ON v.hash = p.hash WHERE v.hash IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn put_vectors(&mut self, rows: &[(String, Vec<f32>)]) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        {
            let mut ins = tx.prepare_cached(
                "INSERT OR REPLACE INTO vectors(hash, dim, embedding) VALUES (?1, ?2, ?3)",
            )?;
            for (hash, v) in rows {
                ins.execute(params![hash, v.len() as i64, to_blob(v)])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn vector_of(&self, hash: &str) -> Result<Option<Vec<f32>>> {
        Ok(self
            .conn()
            .query_row("SELECT embedding FROM vectors WHERE hash=?1", [hash], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()?
            .map(|b| from_blob(&b)))
    }

    /// Every passage scored against `query`, best `limit` first.
    pub fn search_vectors(&self, query: &[f32], limit: usize) -> Result<Vec<VecHit>> {
        let sql = format!(
            "SELECT {HIT_COLUMNS} FROM passages p
             JOIN vectors v ON v.hash = p.hash JOIN notes n ON n.path = p.path"
        );
        let mut hits: Vec<VecHit> = self
            .conn()
            .prepare(&sql)?
            .query_map([], |r| {
                let blob: Vec<u8> = r.get(4)?;
                Ok(VecHit {
                    path: r.get(0)?,
                    heading: r.get(1)?,
                    line: r.get(2)?,
                    text: r.get(3)?,
                    similarity: dot(query, &from_blob(&blob)),
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        hits.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
        hits.truncate(limit);
        Ok(hits)
    }

    /// Passages of other notes nearest to any passage of `paths`, one per note.
    pub fn similar_to(&self, paths: &[String], limit: usize) -> Result<Vec<VecHit>> {
        let mut mine: Vec<Vec<f32>> = Vec::new();
        let mut stmt = self.conn().prepare(
            "SELECT v.embedding FROM passages p JOIN vectors v ON v.hash = p.hash WHERE p.path = ?1",
        )?;
        for path in paths {
            for row in stmt.query_map([path], |r| r.get::<_, Vec<u8>>(0))? {
                mine.push(from_blob(&row?));
            }
        }
        if mine.is_empty() {
            return Ok(vec![]);
        }
        let sql = format!(
            "SELECT {HIT_COLUMNS} FROM passages p
             JOIN vectors v ON v.hash = p.hash JOIN notes n ON n.path = p.path"
        );
        let mut best: std::collections::HashMap<String, VecHit> = std::collections::HashMap::new();
        let mut stmt = self.conn().prepare(&sql)?;
        for row in stmt.query_map([], |r| {
            let blob: Vec<u8> = r.get(4)?;
            Ok(VecHit {
                path: r.get(0)?,
                heading: r.get(1)?,
                line: r.get(2)?,
                text: r.get(3)?,
                similarity: mine.iter().map(|m| dot(m, &from_blob(&blob))).fold(f32::MIN, f32::max),
            })
        })? {
            let hit = row?;
            if paths.contains(&hit.path) {
                continue;
            }
            match best.get(&hit.path) {
                Some(old) if old.similarity >= hit.similarity => {}
                _ => {
                    best.insert(hit.path.clone(), hit);
                }
            }
        }
        let mut out: Vec<VecHit> = best.into_values().collect();
        out.sort_by(|a, b| b.similarity.total_cmp(&a.similarity).then(a.path.cmp(&b.path)));
        out.truncate(limit);
        Ok(out)
    }
}
```

- [ ] **Step 4: Expose a mutable connection**

`put_vectors` needs a transaction. In `core/src/index/mod.rs`, beside
`pub fn conn`:

```rust
    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p engram-notes-core vector`
Expected: PASS, 5 tests.

- [ ] **Step 6: Commit**

```bash
git add core/src/search core/src/index/mod.rs
git commit -m "feat(search): vectors in the index and a cosine scan"
```

---

### Task 4: Fusion and engram's cliff divider

**Files:**
- Create: `core/src/search/fuse.rs`
- Modify: `core/src/search/mod.rs`
- Test: inline in `core/src/search/fuse.rs`

**Interfaces:**
- Produces: `search::fuse::{RRF_K, CLIFF_FACTOR, CLIFF_MIN_SHARE, rrf, cliff,
  mark_past_divider}`; `search::Hit` (defined here in `search/mod.rs`, used by
  every later task).
- `rrf(branches: &[Vec<String>], k: f64) -> Vec<(String, f64)>`, descending.
- `cliff(scores: &[f32], factor: f32, min_share: f32) -> Option<usize>`.
- `mark_past_divider(hits: &mut [Hit], factor: f32, min_share: f32)`.

- [ ] **Step 1: Define `Hit` and `SearchResults`**

Replace `core/src/search/mod.rs` with:

```rust
//! Full-text and semantic retrieval, fused, with a divider where relevance falls.

pub mod fuse;
pub mod passages;
pub mod vector;

/// One note in a result list, with the passage or snippet that found it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Hit {
    pub path: String,
    pub title: String,
    /// HTML-safe; the full-text branch marks its matches with `<mark>`.
    pub snippet: String,
    pub heading: Option<String>,
    /// 1-based line in the file, for jumping to the passage.
    pub line: u32,
    /// Cosine of the best passage; `None` when only full-text found it.
    pub similarity: Option<f32>,
    /// The fused rank score, for ordering only.
    pub score: f64,
    pub past_divider: bool,
    pub primed: bool,
}

/// A note that did not match but is associated with one that did.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Associated {
    pub path: String,
    pub title: String,
    /// The hit that recalled it.
    pub via: String,
    /// The query that bound them, when one did.
    pub cue: Option<String>,
    pub strength: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    pub associated: Vec<Associated>,
}
```

- [ ] **Step 2: Write the failing test**

Create `core/src/search/fuse.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Hit;

    fn hit(path: &str, similarity: Option<f32>) -> Hit {
        Hit {
            path: path.into(),
            title: path.into(),
            snippet: String::new(),
            heading: None,
            line: 1,
            similarity,
            score: 0.0,
            past_divider: false,
            primed: false,
        }
    }

    #[test]
    fn fusion_rewards_a_note_both_branches_found() {
        let dense = vec!["a.md".to_string(), "b.md".to_string()];
        let sparse = vec!["c.md".to_string(), "a.md".to_string()];
        let fused = rrf(&[dense, sparse], RRF_K);
        assert_eq!(fused[0].0, "a.md");
        assert_eq!(fused.len(), 3);
        assert!(fused[0].1 > fused[1].1);
        assert!(fused.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    #[test]
    fn the_cliff_needs_three_scores_and_a_gap_that_stands_out() {
        assert_eq!(cliff(&[0.9, 0.2], CLIFF_FACTOR, CLIFF_MIN_SHARE), None);
        assert_eq!(cliff(&[0.90, 0.88, 0.86], CLIFF_FACTOR, CLIFF_MIN_SHARE), None);
        assert_eq!(cliff(&[0.90, 0.88, 0.30, 0.29], CLIFF_FACTOR, CLIFF_MIN_SHARE), Some(2));
        // A tie is not a fall.
        assert_eq!(cliff(&[0.5, 0.5, 0.5], CLIFF_FACTOR, CLIFF_MIN_SHARE), None);
    }

    #[test]
    fn marking_leaves_a_tail_even_when_the_list_is_out_of_order() {
        // Fusion put 0.40 above 0.78 and 0.76. Read position by position the
        // largest gap is the first one, and the two good hits below it would be
        // thrown away; read over the sorted scores the fall is before 0.39.
        let mut hits = vec![
            hit("a.md", Some(0.80)),
            hit("b.md", Some(0.40)),
            hit("c.md", Some(0.78)),
            hit("d.md", Some(0.76)),
            hit("e.md", Some(0.39)),
        ];
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
        assert_eq!(
            hits.iter().map(|h| h.past_divider).collect::<Vec<_>>(),
            vec![false, false, false, false, true]
        );
    }

    #[test]
    fn a_hit_without_a_similarity_is_never_the_line_itself() {
        let mut hits = vec![
            hit("a.md", Some(0.80)),
            hit("b.md", None),
            hit("c.md", Some(0.78)),
            hit("d.md", Some(0.05)),
        ];
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
        assert!(!hits[1].past_divider);
        assert!(hits[3].past_divider);
    }

    #[test]
    fn nothing_is_marked_when_there_is_no_fall() {
        let mut hits = vec![hit("a.md", Some(0.9)), hit("b.md", Some(0.88)), hit("c.md", Some(0.86))];
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
        assert!(hits.iter().all(|h| !h.past_divider));
    }
}
```

- [ ] **Step 3: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core fuse`
Expected: FAIL — `cannot find function rrf`.

- [ ] **Step 4: Write fusion and the cliff**

Above the tests in `core/src/search/fuse.rs`:

```rust
//! Reciprocal rank fusion, and engram's cliff for the relevance divider.

use crate::search::Hit;
use std::collections::HashMap;

/// engram's `k`. Large enough that rank 1 and rank 2 are not far apart.
pub const RRF_K: f64 = 60.0;
/// The fall must be this many times the mean of the other gaps.
pub const CLIFF_FACTOR: f32 = 3.0;
/// …and at least this share of the top score, so a flat list has no fall.
pub const CLIFF_MIN_SHARE: f32 = 0.01;

/// One entry per key, scored `Σ 1/(k + rank)` over the branches, best first.
pub fn rrf(branches: &[Vec<String>], k: f64) -> Vec<(String, f64)> {
    let mut score: HashMap<&str, f64> = HashMap::new();
    let mut first: HashMap<&str, usize> = HashMap::new();
    for branch in branches {
        for (rank, key) in branch.iter().enumerate() {
            *score.entry(key).or_insert(0.0) += 1.0 / (k + rank as f64 + 1.0);
            first.entry(key).or_insert(rank);
        }
    }
    let mut out: Vec<(String, f64)> = score.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    // The rank a branch gave it breaks ties, then the path, so order is stable.
    out.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then(first[a.0.as_str()].cmp(&first[b.0.as_str()]))
            .then(a.0.cmp(&b.0))
    });
    out
}

/// Where a descending list of scores falls off, if it does.
///
/// engram's rule: at least three scores, the largest gap larger than
/// `factor ×` the mean of the others and larger than `min_share ×` the top.
pub fn cliff(scores: &[f32], factor: f32, min_share: f32) -> Option<usize> {
    if scores.len() < 3 {
        return None;
    }
    let gaps: Vec<f32> = scores.windows(2).map(|w| w[0] - w[1]).collect();
    let (at, largest) = gaps.iter().copied().enumerate().fold((0usize, 0.0f32), |best, (i, g)| {
        if g > best.1 { (i, g) } else { best }
    });
    if largest <= 0.0 || largest <= min_share * scores[0].abs() {
        return None;
    }
    let others = (gaps.iter().sum::<f32>() - largest) / (gaps.len() - 1) as f32;
    (largest > factor * others).then_some(at + 1)
}

/// Flag every hit from the fall on, leaving the list in its order.
///
/// The gaps are read over the similarities sorted, then carried back as "after
/// the last hit that still reaches the cut", so what is marked is always a
/// tail. A fused list is not in score order — a hit both branches found sits
/// above one only the dense branch ranked higher — and reading position by
/// position would cut the good hit below it away.
pub fn mark_past_divider(hits: &mut [Hit], factor: f32, min_share: f32) {
    let mut sorted: Vec<f32> = hits.iter().filter_map(|h| h.similarity).collect();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let Some(above) = cliff(&sorted, factor, min_share) else {
        return;
    };
    let cut = sorted[above - 1];
    let from = hits
        .iter()
        .rposition(|h| h.similarity.is_some_and(|s| s >= cut))
        .map_or(0, |i| i + 1);
    for h in hits.iter_mut().skip(from) {
        h.past_divider = true;
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p engram-notes-core fuse`
Expected: PASS, 5 tests.

- [ ] **Step 6: Commit**

```bash
git add core/src/search
git commit -m "feat(search): rank fusion and engram's cliff divider"
```

---

### Task 5: Search, embedding and memory settings

**Files:**
- Modify: `core/src/config.rs`
- Test: inline in `core/src/config.rs`

**Interfaces:**
- Produces: `config::{SearchConfig, MemoryConfig, EmbedConfig}` on
  `AppConfig` as fields `search`, `memory`, `embed`.

Values are engram's, or the spec's where it names one.

- [ ] **Step 1: Write the failing test**

Add to the test module in `core/src/config.rs`:

```rust
    #[test]
    fn search_and_memory_defaults_are_the_shipped_numbers() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.search.candidate_multiplier, 3);
        assert_eq!(cfg.search.rrf_k, 60.0);
        assert_eq!(cfg.search.cliff_factor, 3.0);
        assert!(cfg.memory.enabled);
        assert_eq!(cfg.memory.activation_half_life_days, 30.0);
        assert_eq!(cfg.memory.assoc_half_life_days, 90.0);
        assert_eq!(cfg.memory.sitting_gap_secs, 1800);
        assert_eq!(cfg.memory.prime_lift, 2);
        assert_eq!(cfg.memory.spread_max, 3);
        assert_eq!(cfg.embed.batch, 32);
        assert_eq!(cfg.embed.model_dir, None);
    }

    #[test]
    fn an_old_app_json_gains_the_new_sections() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"theme":"dark","memory":{"enabled":false}}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert!(!cfg.memory.enabled);
        assert_eq!(cfg.memory.spread_max, 3);
        assert_eq!(cfg.search.rrf_k, 60.0);
    }
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core config`
Expected: FAIL — `no field search on type AppConfig`.

- [ ] **Step 3: Add the three structs**

In `core/src/config.rs`, before `AppConfig`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    /// Each branch fetches `limit × multiplier` candidates before fusing.
    pub candidate_multiplier: usize,
    pub rrf_k: f64,
    pub cliff_factor: f32,
    pub cliff_min_share: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            candidate_multiplier: 3,
            rrf_k: crate::search::fuse::RRF_K,
            cliff_factor: crate::search::fuse::CLIFF_FACTOR,
            cliff_min_share: crate::search::fuse::CLIFF_MIN_SHARE,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub activation_half_life_days: f64,
    pub assoc_half_life_days: f64,
    /// Silence longer than this starts a new sitting.
    pub sitting_gap_secs: i64,
    /// Two notes opened this far apart in one sitting are associated.
    pub assoc_window_secs: i64,
    /// A link shows once its decayed strength reaches this.
    pub assoc_show: f64,
    pub prime_margin: f64,
    pub prime_lift: usize,
    pub spread_max: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            enabled: true,
            activation_half_life_days: 30.0,
            assoc_half_life_days: 90.0,
            sitting_gap_secs: 1800,
            assoc_window_secs: 600,
            assoc_show: 2.0,
            prime_margin: 0.5,
            prime_lift: 2,
            spread_max: 3,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EmbedConfig {
    /// A folder with the ONNX file and tokenizer, for machines with no network.
    pub model_dir: Option<String>,
    pub batch: usize,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        EmbedConfig {
            model_dir: None,
            batch: 32,
        }
    }
}
```

Add the fields to `AppConfig` and to its `Default`:

```rust
    pub search: SearchConfig,
    pub memory: MemoryConfig,
    pub embed: EmbedConfig,
```

```rust
            search: SearchConfig::default(),
            memory: MemoryConfig::default(),
            embed: EmbedConfig::default(),
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core config`
Expected: PASS.

- [ ] **Step 5: Mirror the types in the frontend**

In `ui/src/lib/api.ts`, extend `AppConfig`:

```ts
export interface AppConfig {
  editor: { default_mode: "live" | "source" | "reading" };
  daily_notes: { folder: string; template: string | null; format: string };
  hotkeys: Record<string, string>;
  theme: "system" | "light" | "dark";
  search: { candidate_multiplier: number; rrf_k: number; cliff_factor: number; cliff_min_share: number };
  memory: {
    enabled: boolean; activation_half_life_days: number; assoc_half_life_days: number;
    sitting_gap_secs: number; assoc_window_secs: number; assoc_show: number;
    prime_margin: number; prime_lift: number; spread_max: number;
  };
  embed: { model_dir: string | null; batch: number };
}
```

- [ ] **Step 6: Run the checks and commit**

Run: `cargo test -p engram-notes-core && cd ui && pnpm check`
Expected: PASS.

```bash
git add core/src/config.rs ui/src/lib/api.ts
git commit -m "feat(config): search, memory and embedding settings"
```

---

### Task 6: Events, sittings, activation and associations

**Files:**
- Create: `core/src/memory/mod.rs`, `core/src/memory/decay.rs`
- Modify: `core/src/lib.rs`, `core/src/rename.rs`
- Test: inline in `decay.rs` and `mod.rs`, plus one in `rename.rs`

**Interfaces:**
- Consumes: `config::MemoryConfig`, `index::Index`.
- Produces: `memory::decay::decayed(value, stamped_at, at, half_life_days) ->
  f64`; `memory::EventKind` (`Open`, `OpenFromSearch`, `FollowLink`, `Search`);
  `memory::AssocRow { via, other, value, cue }`; on `Index`:
  `record_event(kind, path: Option<&str>, query: Option<&str>, cfg, at)`,
  `activation_map(at, half_life) -> HashMap<String, f64>`,
  `assoc_from(paths: &[String], at, cfg) -> Vec<AssocRow>`,
  `move_memory(from, to)`, `forget_memory()`.

- [ ] **Step 1: Write the failing decay test**

Create `core/src/memory/decay.rs`:

```rust
//! One decay curve, read everywhere. Learning is a write; forgetting is free.

/// Strength now, from strength then. `half_life_days <= 0` turns decay off.
pub fn decayed(value: f64, stamped_at: i64, at: i64, half_life_days: f64) -> f64 {
    if half_life_days <= 0.0 {
        return value;
    }
    let elapsed = (at - stamped_at).max(0) as f64;
    value * 2f64.powf(-elapsed / (half_life_days * 86_400.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    #[test]
    fn a_half_life_halves_it() {
        assert!((decayed(4.0, 0, 30 * DAY, 30.0) - 2.0).abs() < 1e-9);
        assert!((decayed(4.0, 0, 60 * DAY, 30.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn time_never_runs_backwards_and_zero_turns_decay_off() {
        assert_eq!(decayed(4.0, 100, 0, 30.0), 4.0);
        assert_eq!(decayed(4.0, 0, 10_000 * DAY, 0.0), 4.0);
    }
}
```

- [ ] **Step 2: Run it**

Add `pub mod memory;` to `core/src/lib.rs` (after `index`) and create
`core/src/memory/mod.rs` with `pub mod decay;`.

Run: `cargo test -p engram-notes-core decay`
Expected: PASS, 2 tests. (The curve is small enough to write in one go.)

- [ ] **Step 3: Write the failing memory test**

Add to `core/src/memory/mod.rs` at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    const DAY: i64 = 86_400;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for name in ["A", "B", "C"] {
            fs::write(d.path().join(format!("{name}.md")), format!("# {name}\nbody")).unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    // A link shows from 0.5 here, so one co-appearance is enough to read back.
    fn cfg() -> MemoryConfig {
        MemoryConfig {
            assoc_show: 0.5,
            ..MemoryConfig::default()
        }
    }

    fn sittings(ix: &Index) -> Vec<i64> {
        ix.conn()
            .prepare("SELECT sitting FROM events ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn silence_longer_than_the_gap_starts_a_new_sitting() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0).unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 60).unwrap();
        ix.record_event(EventKind::Open, Some("C.md"), None, &cfg, 60 + 1801).unwrap();
        assert_eq!(sittings(&ix), vec![1, 1, 2]);
    }

    #[test]
    fn opening_bumps_activation_and_it_decays() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0).unwrap();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 10).unwrap();
        let now = ix.activation_map(10, cfg.activation_half_life_days).unwrap();
        assert!((now["A.md"] - 2.0).abs() < 1e-4, "{}", now["A.md"]);
        let later = ix.activation_map(30 * DAY, cfg.activation_half_life_days).unwrap();
        assert!((later["A.md"] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn two_notes_opened_in_one_sitting_are_associated_with_their_cue() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(EventKind::OpenFromSearch, Some("A.md"), Some("rust"), &cfg, 0).unwrap();
        ix.record_event(EventKind::OpenFromSearch, Some("B.md"), Some("rust"), &cfg, 30).unwrap();
        let rows = ix.assoc_from(&["A.md".into()], 30, &cfg).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].other, "B.md");
        assert_eq!(rows[0].cue.as_deref(), Some("rust"));
        assert_eq!(rows[0].value, 1.0);
    }

    #[test]
    fn a_gap_wider_than_the_window_associates_nothing() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0).unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 601).unwrap();
        assert!(ix.assoc_from(&["A.md".into()], 601, &cfg).unwrap().is_empty());
    }

    #[test]
    fn a_pair_is_stored_once_whichever_way_round_it_is_seen() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        for (at, path) in [(0, "B.md"), (10, "A.md"), (20, "B.md")] {
            ix.record_event(EventKind::Open, Some(path), None, &cfg, at).unwrap();
        }
        let n: i64 = ix.conn().query_row("SELECT count(*) FROM assoc", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        let a: String = ix.conn().query_row("SELECT a_path FROM assoc", [], |r| r.get(0)).unwrap();
        assert_eq!(a, "A.md");
        assert_eq!(ix.assoc_from(&["B.md".into()], 20, &cfg).unwrap()[0].other, "A.md");
    }

    #[test]
    fn only_three_cues_survive_the_busiest_first() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        for (i, q) in ["one", "two", "three", "four", "two"].iter().enumerate() {
            let at = i as i64 * 20;
            ix.record_event(EventKind::OpenFromSearch, Some("A.md"), Some(q), &cfg, at).unwrap();
            ix.record_event(EventKind::OpenFromSearch, Some("B.md"), Some(q), &cfg, at + 1).unwrap();
        }
        let raw: String = ix.conn().query_row("SELECT queries FROM assoc", [], |r| r.get(0)).unwrap();
        let cues: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0]["q"], "two");
    }

    #[test]
    fn forgetting_empties_memory_and_leaves_the_notes() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0).unwrap();
        ix.forget_memory().unwrap();
        assert!(ix.activation_map(0, 30.0).unwrap().is_empty());
        let n: i64 = ix.conn().query_row("SELECT count(*) FROM events", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        let notes: i64 = ix.conn().query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(notes, 3);
    }

    // Both reads join `notes`, so the file has to move with the memory.
    #[test]
    fn memory_follows_a_renamed_note() {
        let (d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0).unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 10).unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::rename(d.path().join("A.md"), d.path().join("sub/A.md")).unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.rebuild(&v).unwrap();
        ix.move_memory("A.md", "sub/A.md").unwrap();
        assert!(ix.activation_map(10, 30.0).unwrap().contains_key("sub/A.md"));
        assert_eq!(ix.assoc_from(&["B.md".into()], 10, &cfg).unwrap()[0].other, "sub/A.md");
    }
}
```

- [ ] **Step 4: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core memory`
Expected: FAIL — `cannot find type EventKind`.

- [ ] **Step 5: Write the memory store**

Put this above the tests in `core/src/memory/mod.rs`:

```rust
//! A small slice of engram: events in sittings, activation and associations,
//! every value stored as `(value, stamped_at)` and read through `decayed`.

pub mod decay;

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use decay::decayed;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

/// At most this many cues per link, the busiest kept.
const MAX_CUES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Open,
    OpenFromSearch,
    FollowLink,
    Search,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::Open => "open",
            EventKind::OpenFromSearch => "open_from_search",
            EventKind::FollowLink => "follow_link",
            EventKind::Search => "search",
        }
    }

    /// The kinds that say the user arrived at a note.
    fn is_arrival(self) -> bool {
        matches!(self, EventKind::Open | EventKind::OpenFromSearch | EventKind::FollowLink)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AssocRow {
    /// The note the link was read from.
    pub via: String,
    pub other: String,
    /// Already decayed to the caller's clock.
    pub value: f64,
    pub cue: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Cue {
    q: String,
    n: u32,
}

fn bump_cue(cues: &mut Vec<Cue>, q: &str) {
    match cues.iter_mut().find(|c| c.q == q) {
        Some(c) => c.n += 1,
        None => cues.push(Cue { q: q.to_owned(), n: 1 }),
    }
    cues.sort_by_key(|c| std::cmp::Reverse(c.n));
    cues.truncate(MAX_CUES);
}

/// Pairs are stored with `a < b`, so a link is one row whichever way it is seen.
fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

impl Index {
    /// Appends the event, then learns from it: activation for the note, an
    /// association with every note reached shortly before it in this sitting.
    pub fn record_event(
        &mut self,
        kind: EventKind,
        path: Option<&str>,
        query: Option<&str>,
        cfg: &MemoryConfig,
        at: i64,
    ) -> Result<()> {
        let last: Option<(i64, i64)> = self
            .conn()
            .query_row("SELECT at, sitting FROM events ORDER BY rowid DESC LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let sitting = match last {
            Some((last_at, s)) if at - last_at <= cfg.sitting_gap_secs => s,
            Some((_, s)) => s + 1,
            None => 1,
        };
        self.conn().execute(
            "INSERT INTO events(at, kind, path, query, sitting) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![at, kind.as_str(), path, query, sitting],
        )?;
        let (Some(path), true) = (path, kind.is_arrival()) else {
            return Ok(());
        };
        self.bump_activation(path, 1.0, cfg, at)?;
        // Both spec rules in one pass: opened from the same search, or opened
        // in the same sitting within the window. One row per note, however many
        // times it was reached; the second column is our query when that note
        // was reached under it too, which is exactly the cue.
        let recent: Vec<(String, Option<String>)> = self
            .conn()
            .prepare(
                "SELECT path, max(CASE WHEN query = ?5 THEN query END) FROM events
                 WHERE sitting = ?1 AND at >= ?2 AND at <= ?3 AND path IS NOT NULL AND path <> ?4
                   AND kind IN ('open', 'open_from_search', 'follow_link')
                 GROUP BY path",
            )?
            .query_map(
                params![sitting, at - cfg.assoc_window_secs, at, path, query],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<std::result::Result<_, _>>()?;
        for (other, cue) in recent {
            self.bump_assoc(path, &other, 1.0, cue.as_deref(), cfg, at)?;
        }
        Ok(())
    }

    /// Read the row, decay it to now, add the delta, write it back with a
    /// fresh stamp: one transaction, no stale number anywhere.
    pub fn bump_activation(&mut self, path: &str, delta: f64, cfg: &MemoryConfig, at: i64) -> Result<()> {
        let old: Option<(f64, i64)> = self
            .conn()
            .query_row("SELECT value, stamped_at FROM activation WHERE path=?1", [path], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let value = match old {
            Some((v, s)) => decayed(v, s, at, cfg.activation_half_life_days) + delta,
            None => delta,
        };
        self.conn().execute(
            "INSERT OR REPLACE INTO activation(path, value, stamped_at) VALUES (?1, ?2, ?3)",
            params![path, value, at],
        )?;
        Ok(())
    }

    pub fn bump_assoc(
        &mut self,
        a: &str,
        b: &str,
        delta: f64,
        cue: Option<&str>,
        cfg: &MemoryConfig,
        at: i64,
    ) -> Result<()> {
        let (a, b) = pair(a, b);
        let old: Option<(f64, i64, String)> = self
            .conn()
            .query_row(
                "SELECT value, stamped_at, queries FROM assoc WHERE a_path=?1 AND b_path=?2",
                [a, b],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (value, mut cues) = match old {
            Some((v, s, q)) => (
                decayed(v, s, at, cfg.assoc_half_life_days) + delta,
                serde_json::from_str::<Vec<Cue>>(&q).unwrap_or_default(),
            ),
            None => (delta, Vec::new()),
        };
        if let Some(q) = cue.map(str::trim).filter(|q| !q.is_empty()) {
            bump_cue(&mut cues, q);
        }
        let queries = serde_json::to_string(&cues).unwrap_or_else(|_| "[]".into());
        self.conn().execute(
            "INSERT OR REPLACE INTO assoc(a_path, b_path, value, stamped_at, queries) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![a, b, value, at, queries],
        )?;
        Ok(())
    }

    /// Every note's activation, decayed to `at`. Notes that are gone are left out.
    pub fn activation_map(&self, at: i64, half_life_days: f64) -> Result<HashMap<String, f64>> {
        Ok(self
            .conn()
            .prepare("SELECT a.path, a.value, a.stamped_at FROM activation a JOIN notes n ON n.path = a.path")?
            .query_map([], |r| {
                let path: String = r.get(0)?;
                let value: f64 = r.get(1)?;
                let stamped: i64 = r.get(2)?;
                Ok((path, decayed(value, stamped, at, half_life_days)))
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// Links from any of `paths`, strongest first, only those above the show
    /// threshold and only to notes that still exist.
    pub fn assoc_from(&self, paths: &[String], at: i64, cfg: &MemoryConfig) -> Result<Vec<AssocRow>> {
        let mut out = Vec::new();
        let mut stmt = self.conn().prepare(
            "SELECT CASE WHEN a_path = ?1 THEN b_path ELSE a_path END, value, stamped_at, queries
             FROM assoc JOIN notes n ON n.path = CASE WHEN a_path = ?1 THEN b_path ELSE a_path END
             WHERE a_path = ?1 OR b_path = ?1",
        )?;
        for via in paths {
            for row in stmt.query_map([via], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (other, value, stamped, queries) = row?;
                let value = decayed(value, stamped, at, cfg.assoc_half_life_days);
                if value < cfg.assoc_show {
                    continue;
                }
                let cue = serde_json::from_str::<Vec<Cue>>(&queries)
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|c| c.q);
                out.push(AssocRow { via: via.clone(), other, value, cue });
            }
        }
        out.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.other.cmp(&b.other)));
        Ok(out)
    }

    /// A renamed note keeps what it learned.
    pub fn move_memory(&mut self, from: &str, to: &str) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        tx.execute("UPDATE OR REPLACE activation SET path=?2 WHERE path=?1", [from, to])?;
        tx.execute("UPDATE OR REPLACE assoc SET a_path=?2 WHERE a_path=?1", [from, to])?;
        tx.execute("UPDATE OR REPLACE assoc SET b_path=?2 WHERE b_path=?1", [from, to])?;
        // The pair order is part of the key; a move can break it.
        tx.execute(
            "UPDATE assoc SET a_path = b_path, b_path = a_path WHERE a_path > b_path",
            [],
        )?;
        tx.execute("UPDATE events SET path=?2 WHERE path=?1", [from, to])?;
        tx.commit()?;
        Ok(())
    }

    pub fn forget_memory(&mut self) -> Result<()> {
        self.conn()
            .execute_batch("DELETE FROM activation; DELETE FROM assoc; DELETE FROM events;")?;
        Ok(())
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p engram-notes-core memory`
Expected: PASS, 8 tests.

- [ ] **Step 7: Write the failing rename test**

Add to the test module in `core/src/rename.rs`:

```rust
    #[test]
    fn a_rename_carries_memory_to_the_new_path() {
        let (_d, v, mut ix) = indexed(&[("Old.md", "I am old"), ("Ref.md", "[[Old]]")]);
        let cfg = crate::config::MemoryConfig::default();
        ix.record_event(crate::memory::EventKind::Open, Some("Old.md"), None, &cfg, 0)
            .unwrap();
        let plan = plan_rename(&v, &ix, "Old.md", "sub/New.md").unwrap();
        apply_rename(&v, &mut ix, &plan).unwrap();
        let act = ix.activation_map(0, 30.0).unwrap();
        assert!(act.contains_key("sub/New.md"), "{act:?}");
        assert!(!act.contains_key("Old.md"));
    }
```

- [ ] **Step 8: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core a_rename_carries_memory`
Expected: FAIL — the key is still `A.md`.

- [ ] **Step 9: Move memory in `apply_rename`**

In `core/src/rename.rs`, inside `apply_rename`, after `vault.rename(...)` and
before the rewrite loop:

```rust
    for m in &moves {
        if is_note(&m.to) {
            index.move_memory(&m.from, &m.to)?;
        }
    }
```

- [ ] **Step 10: Run everything**

Run: `cargo test -p engram-notes-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy clean.

- [ ] **Step 11: Commit**

```bash
git add core/src/memory core/src/rename.rs core/src/lib.rs
git commit -m "feat(memory): events, sittings, activation and associations"
```

---

### Task 7: Hybrid search with priming and spread

**Files:**
- Create: `core/src/memory/prime.rs`, `core/src/memory/spread.rs`
- Modify: `core/src/search/mod.rs`, `core/src/index/fts.rs`,
  `core/src/memory/mod.rs`
- Test: inline in `prime.rs`, `spread.rs` and `search/mod.rs`

**Interfaces:**
- Consumes: `Hit`, `Associated`, `SearchResults`, `Index::search_fts`,
  `Index::search_vectors`, `fuse::{rrf, mark_past_divider}`,
  `Index::{activation_map, assoc_from}`.
- Produces: `memory::prime::prime(hits: &mut Vec<Hit>, activation:
  &HashMap<String, f64>, margin: f64, lift: usize)`;
  `memory::spread::spread(index, hits, cfg, at) -> Result<Vec<Associated>>`;
  `search::hybrid(index, query, query_vec: Option<&[f32]>, search_cfg,
  memory_cfg, at, limit) -> Result<SearchResults>`.
- `FtsHit` gains `heading: Option<String>` and `line: u32`.

- [ ] **Step 1: Write the failing priming test**

Create `core/src/memory/prime.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Hit;
    use std::collections::HashMap;

    fn hits(paths: &[&str]) -> Vec<Hit> {
        paths
            .iter()
            .map(|p| Hit {
                path: (*p).into(),
                title: (*p).into(),
                snippet: String::new(),
                heading: None,
                line: 1,
                similarity: None,
                score: 0.0,
                past_divider: false,
                primed: false,
            })
            .collect()
    }

    fn paths(h: &[Hit]) -> Vec<&str> {
        h.iter().map(|x| x.path.as_str()).collect()
    }

    #[test]
    fn an_activated_hit_climbs_at_most_the_lift() {
        let mut h = hits(&["a", "b", "c", "d", "e"]);
        let act = HashMap::from([("e".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "e", "c", "d"]);
        assert!(h[2].primed);
        assert!(!h[0].primed);
    }

    #[test]
    fn the_first_two_places_never_move() {
        let mut h = hits(&["a", "b", "c"]);
        let act = HashMap::from([("c".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "c", "b"]);
    }

    #[test]
    fn only_hits_above_the_median_may_climb() {
        // Median of [0, 0, 0, 0, 1] is 0, so only "e" is above it.
        let mut h = hits(&["a", "b", "c", "d", "e"]);
        let act = HashMap::from([("d".to_string(), 0.0), ("e".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "e", "c", "d"]);
    }

    #[test]
    fn no_activation_leaves_the_list_alone() {
        let mut h = hits(&["a", "b", "c", "d"]);
        prime(&mut h, &HashMap::new(), 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "c", "d"]);
        assert!(h.iter().all(|x| !x.primed));
    }

    #[test]
    fn a_short_list_is_a_list_nothing_moves_in() {
        let mut h = hits(&["a", "b"]);
        let act = HashMap::from([("b".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b"]);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Add `pub mod prime;` and `pub mod spread;` to `core/src/memory/mod.rs` (after
`pub mod decay;`).

Run: `cargo test -p engram-notes-core prime`
Expected: FAIL — `cannot find function prime`.

- [ ] **Step 3: Write priming**

Above the tests in `core/src/memory/prime.rs`:

```rust
//! engram's bounded lift: a hit the user reaches for climbs, a little.

use crate::search::Hit;
use std::collections::HashMap;

/// Move hits up on activation, within hard bounds.
///
/// Rank-based, not score-based: a fused score means nothing across queries,
/// while "moved up two places" means the same thing every time. Activation is
/// normalised within this one list, so `margin` is a fraction of the most
/// accessible hit here. Index 0 is untouchable and index 1 cannot move, because
/// moving it would displace rank 1 — an exact match is never buried.
///
/// Every destination is decided against the original order in one pass, then
/// one stable sort reorders the list; deciding as the list moves would let a
/// row borrow the gap another row's move opened.
pub fn prime(hits: &mut Vec<Hit>, activation: &HashMap<String, f64>, margin: f64, lift: usize) {
    let n = hits.len();
    if lift == 0 || n < 3 {
        return;
    }
    let raw: Vec<f64> = hits
        .iter()
        .map(|h| activation.get(&h.path).copied().unwrap_or(0.0))
        .collect();
    let max = raw.iter().copied().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return;
    }
    // The spec's extra rule: only above the list's own median may climb.
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    let median = match n % 2 {
        0 => (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0,
        _ => sorted[n / 2],
    };
    let acts: Vec<f64> = raw.iter().map(|a| a / max).collect();
    let may_climb: Vec<bool> = raw.iter().map(|a| *a > median).collect();

    let mut climb = vec![0usize; n];
    for i in 2..n {
        if !may_climb[i] {
            continue;
        }
        let mut c = 0usize;
        while c < lift {
            let predecessor = i - c - 1;
            if predecessor < 1 || acts[i] - acts[predecessor] <= margin {
                break;
            }
            c += 1;
        }
        climb[i] = c;
    }

    // The secondary key is load-bearing: without it a climbing row sorts behind
    // the row it was meant to pass, its original index being larger.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (i - climb[i], u8::from(climb[i] == 0), i));
    let mut slots: Vec<Option<Hit>> = hits.drain(..).map(Some).collect();
    for i in order {
        let mut h = slots[i].take().expect("each hit is moved once");
        h.primed = climb[i] > 0;
        hits.push(h);
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core prime`
Expected: PASS, 5 tests.

- [ ] **Step 5: Write the failing spread test**

Create `core/src/memory/spread.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::index::Index;
    use crate::memory::EventKind;
    use crate::search::Hit;
    use crate::vault::Vault;
    use std::fs;

    fn hit(path: &str) -> Hit {
        Hit {
            path: path.into(),
            title: path.into(),
            snippet: String::new(),
            heading: None,
            line: 1,
            similarity: None,
            score: 0.0,
            past_divider: false,
            primed: false,
        }
    }

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for name in ["A", "B", "C", "D", "E"] {
            fs::write(d.path().join(format!("{name}.md")), "body").unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    // Two co-appearances put a link above the show threshold of 2.0.
    fn bind(ix: &mut Index, cfg: &MemoryConfig, a: &str, b: &str, cue: Option<&str>) {
        for _ in 0..2 {
            ix.bump_assoc(a, b, 1.0, cue, cfg, 0).unwrap();
        }
    }

    #[test]
    fn a_note_linked_to_a_top_hit_is_offered_with_its_cue() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "A.md", "D.md", Some("rust"));
        let out = spread(&ix, &[hit("A.md"), hit("B.md")], &cfg, 0).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "D.md");
        assert_eq!(out[0].via, "A.md");
        assert_eq!(out[0].cue.as_deref(), Some("rust"));
    }

    #[test]
    fn a_note_already_in_the_list_is_not_offered_again() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "A.md", "B.md", None);
        assert!(spread(&ix, &[hit("A.md"), hit("B.md")], &cfg, 0).unwrap().is_empty());
    }

    #[test]
    fn only_the_top_three_hits_are_anchors_and_the_spread_is_capped() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "E.md", "D.md", None); // E is the fourth hit
        let hits = [hit("A.md"), hit("B.md"), hit("C.md"), hit("E.md")];
        assert!(spread(&ix, &hits, &cfg, 0).unwrap().is_empty());
    }

    #[test]
    fn a_weak_link_stays_below_the_line_and_decay_takes_it_there() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("A.md", "D.md", 1.0, None, &cfg, 0).unwrap();
        assert!(spread(&ix, &[hit("A.md")], &cfg, 0).unwrap().is_empty());
        bind(&mut ix, &cfg, "A.md", "D.md", None);
        assert_eq!(spread(&ix, &[hit("A.md")], &cfg, 0).unwrap().len(), 1);
        // Three half-lives later the same link is 0.375 and shows no more.
        assert!(spread(&ix, &[hit("A.md")], &cfg, 270 * 86_400).unwrap().is_empty());
    }
}
```

- [ ] **Step 6: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core spread`
Expected: FAIL — `cannot find function spread`.

- [ ] **Step 7: Write spread**

Above the tests in `core/src/memory/spread.rs`:

```rust
//! Notes the embedding calls strangers and the person calls inseparable.

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use crate::search::{Associated, Hit};
use std::collections::HashSet;

/// The top three hits are anchors: their strongest links that are not already
/// in the list, strongest first, at most `spread_max`. One hop, no reordering.
pub fn spread(index: &Index, hits: &[Hit], cfg: &MemoryConfig, at: i64) -> Result<Vec<Associated>> {
    if cfg.spread_max == 0 || hits.is_empty() {
        return Ok(vec![]);
    }
    let anchors: Vec<String> = hits.iter().take(3).map(|h| h.path.clone()).collect();
    let listed: HashSet<&str> = hits.iter().map(|h| h.path.as_str()).collect();
    let titles: std::collections::HashMap<String, String> = index.titles()?.into_iter().collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in index.assoc_from(&anchors, at, cfg)? {
        if listed.contains(row.other.as_str()) || !seen.insert(row.other.clone()) {
            continue;
        }
        out.push(Associated {
            title: titles.get(&row.other).cloned().unwrap_or_else(|| row.other.clone()),
            path: row.other,
            via: row.via,
            cue: row.cue,
            strength: row.value,
        });
        if out.len() >= cfg.spread_max {
            break;
        }
    }
    Ok(out)
}
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p engram-notes-core spread`
Expected: PASS, 4 tests.

- [ ] **Step 9: Give `FtsHit` a heading and a line**

In `core/src/index/fts.rs`, add the fields to the struct:

```rust
pub struct FtsHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub heading: Option<String>,
    pub line: u32,
    pub score: f64,
}
```

Change the SQL and the row mapping to carry the note's first line and no
heading — the passage branch supplies headings, and full-text matches are not
tied to one:

```rust
        let sql = "SELECT n.path, n.title,
                          snippet(notes_fts, 1, char(1), char(2), '…', 24),
                          bm25(notes_fts, 10.0, 1.0), n.body_line
                   FROM notes_fts JOIN notes n ON n.rowid = notes_fts.rowid
                   WHERE notes_fts MATCH ?1
                   ORDER BY bm25(notes_fts, 10.0, 1.0)
                   LIMIT ?2";
```

```rust
                Ok(FtsHit {
                    path: r.get(0)?,
                    title: r.get(1)?,
                    snippet,
                    heading: None,
                    line: r.get::<_, i64>(4)? as u32,
                    score: -r.get::<_, f64>(3)?,
                })
```

- [ ] **Step 10: Write the failing hybrid test**

Add to `core/src/search/mod.rs` at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, SearchConfig};
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::memory::EventKind;
    use crate::vault::Vault;
    use std::fs;

    fn vault_with_vectors() -> (tempfile::TempDir, Index, FakeEmbedder) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Rust.md"), "# Rust\nownership rules keep memory safe").unwrap();
        fs::write(d.path().join("Borrow.md"), "# Borrow\nownership and borrowing explained").unwrap();
        fs::write(d.path().join("Coffee.md"), "# Coffee\nkettle water grind beans").unwrap();
        fs::write(d.path().join("Tea.md"), "# Tea\nleaves steep water").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(1000).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> =
            pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        (d, ix, e)
    }

    #[test]
    fn both_branches_fuse_into_one_entry_per_note() {
        let (_d, ix, mut e) = vault_with_vectors();
        let q = e.embed_query("ownership").unwrap();
        let out = hybrid(&ix, "ownership", Some(&q), &SearchConfig::default(), &MemoryConfig::default(), 0, 10).unwrap();
        let paths: Vec<&str> = out.hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths.iter().filter(|p| **p == "Rust.md").count(), 1);
        assert!(paths.contains(&"Rust.md") && paths.contains(&"Borrow.md"));
        assert!(out.hits[0].snippet.contains("<mark>"), "{}", out.hits[0].snippet);
        assert!(out.hits[0].similarity.is_some());
    }

    #[test]
    fn a_note_only_the_text_matched_has_no_similarity() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = hybrid(&ix, "kettle", None, &SearchConfig::default(), &MemoryConfig::default(), 0, 10).unwrap();
        assert_eq!(out.hits[0].path, "Coffee.md");
        assert!(out.hits[0].similarity.is_none());
        assert!(out.hits.iter().all(|h| !h.past_divider));
    }

    #[test]
    fn memory_off_skips_priming_and_spread() {
        let (_d, mut ix, mut e) = vault_with_vectors();
        let mut cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Tea.md", 5.0, Some("cue"), &cfg, 0).unwrap();
        ix.record_event(EventKind::Open, Some("Coffee.md"), None, &cfg, 0).unwrap();
        let q = e.embed_query("ownership").unwrap();
        cfg.enabled = false;
        let out = hybrid(&ix, "ownership", Some(&q), &SearchConfig::default(), &cfg, 0, 10).unwrap();
        assert!(out.associated.is_empty());
        assert!(out.hits.iter().all(|h| !h.primed));
    }

    #[test]
    fn memory_on_spreads_to_an_associated_note() {
        let (_d, mut ix, mut e) = vault_with_vectors();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Tea.md", 5.0, Some("cue"), &cfg, 0).unwrap();
        let q = e.embed_query("ownership").unwrap();
        let out = hybrid(&ix, "ownership rules", Some(&q), &SearchConfig::default(), &cfg, 0, 3).unwrap();
        assert!(out.hits.len() <= 3);
        assert_eq!(out.associated.iter().map(|a| a.path.as_str()).collect::<Vec<_>>(), vec!["Tea.md"]);
        assert_eq!(out.associated[0].cue.as_deref(), Some("cue"));
    }

    #[test]
    fn an_empty_query_returns_nothing() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = hybrid(&ix, "  ", None, &SearchConfig::default(), &MemoryConfig::default(), 0, 10).unwrap();
        assert_eq!(out, SearchResults::default());
    }
}
```

- [ ] **Step 11: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core search::tests`
Expected: FAIL — `cannot find function hybrid`.

- [ ] **Step 12: Write `hybrid`**

In `core/src/search/mod.rs`, after the type definitions:

```rust
use crate::Result;
use crate::config::{MemoryConfig, SearchConfig};
use crate::index::Index;
use std::collections::HashMap;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Full-text and semantic retrieval fused into one list, with the divider
/// drawn, priming applied and associated notes spread beneath it.
///
/// `query_vec` is `None` while no model is loaded; the full-text branch alone
/// then answers, which is why search works during the first download.
pub fn hybrid(
    index: &Index,
    query: &str,
    query_vec: Option<&[f32]>,
    cfg: &SearchConfig,
    mem: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<SearchResults> {
    if query.trim().is_empty() {
        return Ok(SearchResults::default());
    }
    let wide = limit * cfg.candidate_multiplier.max(1);
    let fts = index.search_fts(query, wide)?;
    // One entry per note: its best passage is the one the scan ranked first.
    let mut best: HashMap<String, vector::VecHit> = HashMap::new();
    let mut dense: Vec<String> = Vec::new();
    if let Some(q) = query_vec {
        for hit in index.search_vectors(q, wide)? {
            if best.contains_key(&hit.path) {
                continue;
            }
            dense.push(hit.path.clone());
            best.insert(hit.path.clone(), hit);
        }
        dense.truncate(wide);
    }
    let sparse: Vec<String> = fts.iter().map(|h| h.path.clone()).collect();
    let by_path: HashMap<&str, &crate::index::fts::FtsHit> =
        fts.iter().map(|h| (h.path.as_str(), h)).collect();
    let titles: HashMap<String, String> = index.titles()?.into_iter().collect();

    let mut hits: Vec<Hit> = fuse::rrf(&[dense, sparse], cfg.rrf_k)
        .into_iter()
        .take(limit)
        .map(|(path, score)| {
            let passage = best.get(&path);
            let text = by_path.get(path.as_str());
            Hit {
                title: titles.get(&path).cloned().unwrap_or_else(|| path.clone()),
                snippet: match text {
                    Some(h) => h.snippet.clone(),
                    None => escape_html(passage.map_or("", |p| p.text.as_str())),
                },
                heading: passage
                    .map(|p| p.heading.clone())
                    .filter(|h| !h.is_empty()),
                line: passage.map(|p| p.line).or(text.map(|h| h.line)).unwrap_or(1),
                similarity: passage.map(|p| p.similarity),
                score,
                past_divider: false,
                primed: false,
                path,
            }
        })
        .collect();

    if mem.enabled {
        let activation = index.activation_map(at, mem.activation_half_life_days)?;
        crate::memory::prime::prime(&mut hits, &activation, mem.prime_margin, mem.prime_lift);
    }
    fuse::mark_past_divider(&mut hits, cfg.cliff_factor, cfg.cliff_min_share);
    let associated = match mem.enabled {
        true => crate::memory::spread::spread(index, &hits, mem, at)?,
        false => vec![],
    };
    Ok(SearchResults { hits, associated })
}
```

- [ ] **Step 13: Run everything**

Run: `cargo test -p engram-notes-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. `src-tauri`'s serialisation smoke tests may reference `FtsHit`;
fix any field mismatch they report.

- [ ] **Step 14: Commit**

```bash
git add core/src
git commit -m "feat(search): hybrid retrieval with priming and spread"
```

---

### Task 8: The Related pane's three groups

**Files:**
- Create: `core/src/memory/related.rs`
- Modify: `core/src/memory/mod.rs`
- Test: inline in `core/src/memory/related.rs`

**Interfaces:**
- Produces: `memory::related::{Related, SimilarNote, related}`.
  `related(index, path, text, cfg, at, limit) -> Result<Related>` where
  `Related { associated: Vec<Associated>, similar: Vec<SimilarNote>, suggested:
  Vec<SimilarNote> }` and `SimilarNote { path, title, heading, text,
  similarity }`.

- [ ] **Step 1: Write the failing test**

Create `core/src/memory/related.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Rust.md"), "# Rust\nownership rules keep memory safe. see [[Borrow]]").unwrap();
        fs::write(d.path().join("Borrow.md"), "# Borrow\nownership rules and borrowing").unwrap();
        fs::write(d.path().join("Lifetimes.md"), "# Lifetimes\nownership rules over time").unwrap();
        fs::write(d.path().join("Coffee.md"), "# Coffee\nkettle grind beans").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(1000).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        (d, ix)
    }

    #[test]
    fn similar_lists_other_notes_nearest_first() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules keep memory safe. see [[Borrow]]";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.similar.iter().all(|s| s.path != "Rust.md"));
        assert_eq!(r.similar[0].path, "Borrow.md");
        assert_eq!(r.similar[0].heading, "Borrow");
    }

    #[test]
    fn a_note_already_linked_is_not_suggested() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules keep memory safe. see [[Borrow]]";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.suggested.iter().all(|s| s.path != "Borrow.md"));
        assert!(r.suggested.iter().any(|s| s.path == "Lifetimes.md"));
    }

    #[test]
    fn a_title_the_text_already_names_is_not_suggested() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules; lifetimes matter too";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.suggested.iter().all(|s| s.path != "Lifetimes.md"));
    }

    #[test]
    fn associated_shows_a_learned_link_and_memory_off_hides_it() {
        let (_d, mut ix) = ix();
        let mut cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Coffee.md", 5.0, Some("why"), &cfg, 0).unwrap();
        let r = related(&ix, "Rust.md", "x", &cfg, 0, 10).unwrap();
        assert_eq!(r.associated[0].path, "Coffee.md");
        assert_eq!(r.associated[0].cue.as_deref(), Some("why"));
        cfg.enabled = false;
        let off = related(&ix, "Rust.md", "x", &cfg, 0, 10).unwrap();
        assert!(off.associated.is_empty());
        assert!(!off.similar.is_empty(), "similar is shown with memory off");
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Add `pub mod related;` to `core/src/memory/mod.rs`.

Run: `cargo test -p engram-notes-core related`
Expected: FAIL — `cannot find function related`.

- [ ] **Step 3: Write it**

Above the tests in `core/src/memory/related.rs`:

```rust
//! What the open note is related to: learned links, near passages, and the
//! links it could have.

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use crate::search::Associated;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SimilarNote {
    pub path: String,
    pub title: String,
    pub heading: String,
    pub text: String,
    pub similarity: f32,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Related {
    pub associated: Vec<Associated>,
    pub similar: Vec<SimilarNote>,
    /// Obsidian's unlinked mentions, extended by meaning.
    pub suggested: Vec<SimilarNote>,
}

pub fn related(
    index: &Index,
    path: &str,
    text: &str,
    cfg: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<Related> {
    let titles: std::collections::HashMap<String, String> = index.titles()?.into_iter().collect();
    let associated = match cfg.enabled {
        true => index
            .assoc_from(&[path.to_owned()], at, cfg)?
            .into_iter()
            .map(|row| Associated {
                title: titles.get(&row.other).cloned().unwrap_or_else(|| row.other.clone()),
                path: row.other,
                via: row.via,
                cue: row.cue,
                strength: row.value,
            })
            .collect(),
        false => vec![],
    };
    let similar: Vec<SimilarNote> = index
        .similar_to(&[path.to_owned()], limit)?
        .into_iter()
        .map(|h| SimilarNote {
            title: titles.get(&h.path).cloned().unwrap_or_else(|| h.path.clone()),
            path: h.path,
            heading: h.heading,
            text: h.text,
            similarity: h.similarity,
        })
        .collect();
    let linked: HashSet<String> = index
        .outgoing(path)?
        .into_iter()
        .filter_map(|l| l.target_path)
        .collect();
    let lower = text.to_lowercase();
    let suggested = similar
        .iter()
        .filter(|s| !linked.contains(&s.path) && !lower.contains(&s.title.to_lowercase()))
        .cloned()
        .collect();
    Ok(Related { associated, similar, suggested })
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core related`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add core/src/memory
git commit -m "feat(memory): the Related pane's associated, similar and suggested"
```

---

### Task 9: Semantic edges for the graph

**Files:**
- Modify: `core/src/graph.rs`
- Test: inline in `core/src/graph.rs`

**Interfaces:**
- Produces: `graph::{SemanticKind, SemanticEdge, semantic_edges}`.
  `semantic_edges(index, paths: Option<&[String]>, top_k: usize, cfg:
  &MemoryConfig, at: i64) -> Result<Vec<SemanticEdge>>`;
  `SemanticEdge { source, target, weight: f32, kind }` where `kind` serialises
  as `"assoc"` or `"similar"`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `core/src/graph.rs`:

```rust
    #[test]
    fn semantic_edges_carry_associations_and_near_passages() {
        use crate::config::MemoryConfig;
        use crate::embed::{Embedder, FakeEmbedder};
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Rust.md"), "ownership rules keep memory safe").unwrap();
        std::fs::write(d.path().join("Borrow.md"), "ownership rules and borrowing").unwrap();
        std::fs::write(d.path().join("Coffee.md"), "kettle grind beans").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(1000).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Coffee.md", 5.0, None, &cfg, 0).unwrap();

        // Undirected: a pair is stored and drawn with the smaller path first.
        let all = semantic_edges(&ix, None, 1, &cfg, 0).unwrap();
        assert!(all.iter().any(|e| e.kind == SemanticKind::Assoc
            && e.source == "Coffee.md"
            && e.target == "Rust.md"));
        let similar: Vec<_> = all.iter().filter(|e| e.kind == SemanticKind::Similar).collect();
        assert!(similar.iter().any(|e| {
            (e.source.as_str(), e.target.as_str()) == ("Borrow.md", "Rust.md")
        }));
        // One row per pair, whichever way round it was found.
        let mut pairs: Vec<(String, String)> =
            similar.iter().map(|e| (e.source.clone(), e.target.clone())).collect();
        pairs.sort();
        pairs.dedup();
        assert_eq!(pairs.len(), similar.len());
        assert!(similar.iter().all(|e| e.weight > 0.0 && e.weight <= 1.0));
    }

    #[test]
    fn asking_for_one_note_gives_only_its_edges() {
        use crate::config::MemoryConfig;
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "a").unwrap();
        std::fs::write(d.path().join("B.md"), "b").unwrap();
        std::fs::write(d.path().join("C.md"), "c").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("A.md", "B.md", 5.0, None, &cfg, 0).unwrap();
        ix.bump_assoc("B.md", "C.md", 5.0, None, &cfg, 0).unwrap();
        let only_a = semantic_edges(&ix, Some(&["A.md".to_string()]), 3, &cfg, 0).unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].target, "B.md");
    }
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p engram-notes-core semantic_edges`
Expected: FAIL — `cannot find function semantic_edges`.

- [ ] **Step 3: Write it**

At the end of `core/src/graph.rs` (before the test module):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticKind {
    /// A link the memory learned from co-retrieval.
    Assoc,
    /// Near passages by embedding.
    Similar,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
    pub kind: SemanticKind,
}

/// The edges the graph draws dashed: what the vault is about, not what was
/// linked by hand. `paths` narrows it to one neighbourhood; `None` is the vault.
pub fn semantic_edges(
    index: &Index,
    paths: Option<&[String]>,
    top_k: usize,
    cfg: &crate::config::MemoryConfig,
    at: i64,
) -> Result<Vec<SemanticEdge>> {
    let all: Vec<String>;
    let subset: &[String] = match paths {
        Some(p) => p,
        None => {
            all = index.titles()?.into_iter().map(|(p, _)| p).collect();
            &all
        }
    };
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out = Vec::new();
    for row in index.assoc_from(subset, at, cfg)? {
        let (a, b) = if row.via < row.other {
            (row.via.clone(), row.other.clone())
        } else {
            (row.other.clone(), row.via.clone())
        };
        if seen.insert((a.clone(), b.clone())) {
            out.push(SemanticEdge {
                source: a,
                target: b,
                weight: row.value as f32,
                kind: SemanticKind::Assoc,
            });
        }
    }
    for path in subset {
        for hit in index.similar_to(std::slice::from_ref(path), top_k)? {
            let (a, b) = if *path < hit.path {
                (path.clone(), hit.path.clone())
            } else {
                (hit.path.clone(), path.clone())
            };
            if hit.similarity > 0.0 && seen.insert((a.clone(), b.clone())) {
                out.push(SemanticEdge {
                    source: a,
                    target: b,
                    weight: hit.similarity,
                    kind: SemanticKind::Similar,
                });
            }
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p engram-notes-core graph`
Expected: PASS, the existing graph tests plus the two new ones.

- [ ] **Step 5: Commit**

```bash
git add core/src/graph.rs
git commit -m "feat(graph): semantic edges from associations and near passages"
```

---

### Task 10: FastEmbedder behind a cargo feature

**Files:**
- Create: `core/src/embed/fastembed.rs`
- Modify: `core/Cargo.toml`, `src-tauri/Cargo.toml`, `Cargo.lock`
- Test: inline, `#[ignore]`d — it downloads a model.

**Interfaces:**
- Produces, only with `--features fastembed`:
  `embed::fastembed::FastEmbedder::download(cache_dir: &Path) -> Result<Self>`
  and `FastEmbedder::from_dir(dir: &Path) -> Result<Self>`, both `impl
  Embedder`. `id()` is `"multilingual-e5-small"` for the downloaded model and
  `"dir:<folder name>"` for a user-supplied one.

fastembed adds no e5 prefixes, so this file prepends `query: ` and `passage: `.

- [ ] **Step 1: Add the dependency**

In `core/Cargo.toml`:

```toml
[features]
default = []
fastembed = ["dep:fastembed"]

[dependencies]
fastembed = { version = "6.1", optional = true, default-features = false, features = ["ort-download-binaries-rustls-tls", "hf-hub-rustls-tls"] }
```

In `src-tauri/Cargo.toml`, enable it:

```toml
engram_core = { package = "engram-notes-core", path = "../core", features = ["fastembed"] }
```

Run: `cargo fetch` and check `cargo tree -p engram-notes-core --features fastembed | head`.

- [ ] **Step 2: Write the implementation**

Create `core/src/embed/fastembed.rs`:

```rust
//! fastembed with `multilingual-e5-small`. Downloaded on first use, or loaded
//! from a folder for machines with no network.

use super::{Embedder, normalise};
use crate::{Error, Result};
use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding,
    TextInitOptions, TokenizerFiles, UserDefinedEmbeddingModel,
};
use std::path::Path;

/// fastembed ships no quantized e5-small, so this is the full model.
pub const MODEL_ID: &str = "multilingual-e5-small";
pub const DIM: usize = 384;

pub struct FastEmbedder {
    model: TextEmbedding,
    id: String,
}

fn embed_error(e: impl std::fmt::Display) -> Error {
    Error::Embed(e.to_string())
}

fn read(dir: &Path, name: &str) -> Result<Vec<u8>> {
    std::fs::read(dir.join(name)).map_err(|e| Error::io(dir.join(name), e))
}

impl FastEmbedder {
    /// Fetches the model into `cache_dir` if it is not there already.
    pub fn download(cache_dir: &Path) -> Result<FastEmbedder> {
        let options = TextInitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(false);
        Ok(FastEmbedder {
            model: TextEmbedding::try_new(options).map_err(embed_error)?,
            id: MODEL_ID.to_owned(),
        })
    }

    /// A folder holding `model.onnx`, `tokenizer.json`, `config.json`,
    /// `special_tokens_map.json` and `tokenizer_config.json`.
    pub fn from_dir(dir: &Path) -> Result<FastEmbedder> {
        let model = UserDefinedEmbeddingModel {
            onnx_file: read(dir, "model.onnx")?,
            external_initializers: Default::default(),
            tokenizer_files: TokenizerFiles {
                tokenizer_file: read(dir, "tokenizer.json")?,
                config_file: read(dir, "config.json")?,
                special_tokens_map_file: read(dir, "special_tokens_map.json")?,
                tokenizer_config_file: read(dir, "tokenizer_config.json")?,
            },
            pooling: Some(Pooling::Mean),
            quantization: QuantizationMode::None,
            output_key: None,
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "model".into());
        Ok(FastEmbedder {
            model: TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
                .map_err(embed_error)?,
            id: format!("dir:{name}"),
        })
    }
}

impl Embedder for FastEmbedder {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn dim(&self) -> usize {
        DIM
    }

    // e5 distinguishes a question from a document; fastembed adds no prefix.
    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        let mut out = self.model.embed(prefixed, None).map_err(embed_error)?;
        for v in &mut out {
            normalise(v);
        }
        Ok(out)
    }

    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        let mut out = self
            .model
            .embed(vec![format!("query: {text}")], None)
            .map_err(embed_error)?;
        let mut v = out.pop().ok_or_else(|| Error::Embed("no vector".into()))?;
        normalise(&mut v);
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads the model"]
    fn the_real_model_embeds_and_ranks() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = FastEmbedder::download(dir.path()).unwrap();
        assert_eq!(e.dim(), DIM);
        let docs = e
            .embed_documents(&["rust ownership".into(), "kettle and beans".into()])
            .unwrap();
        assert_eq!(docs[0].len(), DIM);
        let q = e.embed_query("who owns the memory in rust").unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&q, &docs[0]) > dot(&q, &docs[1]));
    }
}
```

If a compile error says `UserDefinedEmbeddingModel` has no such literal form,
use its builders (`with_pooling`, `with_quantization`) as the handoff warns;
keep the same field values.

- [ ] **Step 3: Build both ways**

Run: `cargo test -p engram-notes-core` (no fastembed)
Expected: PASS, and nothing downloads.

Run: `cargo build -p engram-notes-core --features fastembed`
Expected: builds; ort fetches onnxruntime once.

- [ ] **Step 4: Confirm tests still need no model**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: PASS. The `#[ignore]`d test is listed as ignored, and `src-tauri`'s
tests (which do enable the feature) still pass without touching a model.

- [ ] **Step 5: Commit**

```bash
git add core/Cargo.toml core/src/embed src-tauri/Cargo.toml Cargo.lock
git commit -m "feat(embed): fastembed multilingual-e5-small behind a feature"
```

---

### Task 11: The embed thread and the new commands

**Files:**
- Create: `src-tauri/src/embed.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`,
  `src-tauri/src/commands.rs`
- Test: serialisation smoke tests in `src-tauri/src/commands.rs`

**Interfaces:**
- Produces the Tauri commands `search` (now returns `SearchResults`),
  `record_event`, `related`, `forget_memory`, `embed_status`, `typing`,
  `semantic_edges`, `set_model_dir`, and the event `embed-status` carrying
  `EmbedStatus { model: Option<String>, state: String, pending: usize, error:
  Option<String> }` with `state` one of `"off" | "loading" | "ready" | "error"`.

- [ ] **Step 1: Write the embed thread**

Create `src-tauri/src/embed.rs`:

```rust
//! One background thread that embeds passages, lowest priority, paused while
//! the user types.

use crate::state::AppState;
use engram_core::config::EmbedConfig;
use engram_core::embed::{Embedder, fastembed::FastEmbedder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct EmbedStatus {
    pub model: Option<String>,
    /// `off`, `loading`, `ready` or `error`.
    pub state: &'static str,
    pub pending: usize,
    pub error: Option<String>,
}

impl Default for EmbedStatus {
    fn default() -> Self {
        EmbedStatus {
            model: None,
            state: "off",
            pending: 0,
            error: None,
        }
    }
}

#[derive(Default)]
pub struct Embed {
    pub status: Mutex<EmbedStatus>,
    pub embedder: Mutex<Option<Box<dyn Embedder>>>,
    pub typing: Mutex<Option<Instant>>,
    stop: AtomicBool,
}

impl Embed {
    /// The queue waits two seconds after the last keystroke.
    fn typing_now(&self) -> bool {
        self.typing
            .lock()
            .unwrap()
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn publish(app: &AppHandle, embed: &Embed, f: impl FnOnce(&mut EmbedStatus)) {
    let status = {
        let mut s = embed.status.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("embed-status", status);
}

/// Loads the model, then drains the queue for as long as the vault is open.
pub fn spawn(app: AppHandle, embed: Arc<Embed>, cfg: EmbedConfig, models_dir: std::path::PathBuf) {
    std::thread::spawn(move || {
        publish(&app, &embed, |s| {
            s.state = "loading";
            s.error = None;
        });
        let loaded = match cfg.model_dir.as_deref() {
            Some(dir) => FastEmbedder::from_dir(std::path::Path::new(dir)),
            None => FastEmbedder::download(&models_dir),
        };
        let model: Box<dyn Embedder> = match loaded {
            Ok(m) => Box::new(m),
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(e.to_string());
                });
                return;
            }
        };
        let id = model.id();
        {
            let state = app.state::<AppState>();
            let mut guard = state.open.lock().unwrap();
            let Some(open) = guard.as_mut() else { return };
            if let Err(e) = open.index.set_model_id(&id) {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(e.to_string());
                });
                return;
            }
        }
        // One owner from here on: the queue and the query path both go through
        // this mutex.
        *embed.embedder.lock().unwrap() = Some(model);
        publish(&app, &embed, |s| {
            s.state = "ready";
            s.model = Some(id.clone());
        });

        let batch = cfg.batch.max(1);
        while !embed.stop.load(Ordering::Relaxed) {
            if embed.typing_now() {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            // The lock is held to take work and to write it back, never across
            // the embedding itself.
            let pending = {
                let state = app.state::<AppState>();
                let mut guard = state.open.lock().unwrap();
                let Some(open) = guard.as_mut() else { break };
                open.index.pending_vectors(batch).unwrap_or_default()
            };
            if pending.is_empty() {
                publish(&app, &embed, |s| s.pending = 0);
                std::thread::sleep(Duration::from_millis(750));
                continue;
            }
            let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
            let vectors = {
                let mut guard = embed.embedder.lock().unwrap();
                let Some(m) = guard.as_mut() else { break };
                m.embed_documents(&texts)
            };
            match vectors {
                Ok(vectors) => {
                    let rows: Vec<(String, Vec<f32>)> =
                        pending.into_iter().map(|p| p.hash).zip(vectors).collect();
                    let state = app.state::<AppState>();
                    let mut guard = state.open.lock().unwrap();
                    let Some(open) = guard.as_mut() else { break };
                    let _ = open.index.put_vectors(&rows);
                    let left = open.index.pending_count().unwrap_or(0);
                    drop(guard);
                    publish(&app, &embed, |s| s.pending = left);
                }
                Err(e) => {
                    // A failing embedding never blocks editing.
                    publish(&app, &embed, |s| {
                        s.state = "error";
                        s.error = Some(e.to_string());
                    });
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    });
}
```

A query embedding waits on the same mutex as a batch, so a search during a busy
queue can stall for as long as one batch of 32 takes. Accepted: one model, one
owner, and the batch is under a second.

- [ ] **Step 2: Hold the thread in the state**

In `src-tauri/src/state.rs`:

```rust
use crate::embed::Embed;
use std::sync::Arc;

pub struct Open {
    pub vault: Vault,
    pub index: Index,
    pub config: AppConfig,
    pub _watcher: Option<Watcher>,
}

#[derive(Default)]
pub struct AppState {
    pub open: Mutex<Option<Open>>,
    pub embed: Mutex<Arc<Embed>>,
}
```

In `src-tauri/src/lib.rs`, add `mod embed;` and register the new commands in
`generate_handler!`: `record_event`, `related`, `forget_memory`,
`embed_status`, `typing`, `semantic_edges`, `set_model_dir`.

- [ ] **Step 3: Start the thread when a vault opens**

At the end of `open_vault` in `src-tauri/src/commands.rs`, after the state is
set:

```rust
    // A previous vault's thread stops; a new one loads the model and drains.
    let embed = {
        let mut slot = state.embed.lock().unwrap();
        slot.stop();
        *slot = std::sync::Arc::new(crate::embed::Embed::default());
        slot.clone()
    };
    let models = config::data_dir()?.join("models");
    crate::embed::spawn(app.clone(), embed, cfg.embed.clone(), models);
```

- [ ] **Step 4: Replace the search command and add the rest**

In `src-tauri/src/commands.rs`, replace `search` and append:

```rust
fn now() -> i64 {
    chrono::Local::now().timestamp()
}

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> CmdResult<SearchResults> {
    // The query vector is taken before the index lock, and only if a model is up.
    let vector = {
        let embed = state.embed.lock().unwrap().clone();
        let mut guard = embed.embedder.lock().unwrap();
        guard.as_mut().and_then(|m| m.embed_query(&query).ok())
    };
    with_open(&state, |o| {
        Ok(engram_core::search::hybrid(
            &o.index,
            &query,
            vector.as_deref(),
            &o.config.search,
            &o.config.memory,
            now(),
            limit.unwrap_or(50),
        )?)
    })
}

#[tauri::command]
pub fn record_event(
    state: State<AppState>,
    kind: EventKind,
    path: Option<String>,
    query: Option<String>,
) -> CmdResult<()> {
    with_open(&state, |o| {
        if !o.config.memory.enabled {
            return Ok(());
        }
        o.index
            .record_event(kind, path.as_deref(), query.as_deref(), &o.config.memory, now())?;
        Ok(())
    })
}

#[tauri::command]
pub fn related(state: State<AppState>, path: String) -> CmdResult<Related> {
    with_open(&state, |o| {
        let text = o.vault.read(&path).unwrap_or_default();
        Ok(engram_core::memory::related::related(
            &o.index,
            &path,
            &text,
            &o.config.memory,
            now(),
            10,
        )?)
    })
}

#[tauri::command]
pub fn forget_memory(state: State<AppState>) -> CmdResult<()> {
    with_open(&state, |o| Ok(o.index.forget_memory()?))
}

#[tauri::command]
pub fn semantic_edges(
    state: State<AppState>,
    paths: Option<Vec<String>>,
    top_k: Option<usize>,
) -> CmdResult<Vec<SemanticEdge>> {
    with_open(&state, |o| {
        Ok(engram_core::graph::semantic_edges(
            &o.index,
            paths.as_deref(),
            top_k.unwrap_or(3),
            &o.config.memory,
            now(),
        )?)
    })
}

#[tauri::command]
pub fn embed_status(state: State<AppState>) -> EmbedStatus {
    let embed = state.embed.lock().unwrap().clone();
    let status = embed.status.lock().unwrap().clone();
    status
}

/// The editor pings while the user types; the queue waits.
#[tauri::command]
pub fn typing(state: State<AppState>) {
    let embed = state.embed.lock().unwrap().clone();
    *embed.typing.lock().unwrap() = Some(std::time::Instant::now());
}

/// A folder with the ONNX file and tokenizer, or `None` to download again.
#[tauri::command]
pub fn set_model_dir(app: AppHandle, state: State<AppState>, dir: Option<String>) -> CmdResult<()> {
    let cfg = with_open(&state, |o| {
        o.config.embed.model_dir = dir;
        config::save_config(&o.vault, &o.config)?;
        Ok(o.config.embed.clone())
    })?;
    let embed = {
        let mut slot = state.embed.lock().unwrap();
        slot.stop();
        *slot = std::sync::Arc::new(crate::embed::Embed::default());
        slot.clone()
    };
    crate::embed::spawn(app, embed, cfg, config::data_dir()?.join("models"));
    Ok(())
}
```

Add the imports at the top: `use crate::embed::EmbedStatus;`,
`use engram_core::graph::SemanticEdge;`,
`use engram_core::memory::{EventKind, related::Related};`,
`use engram_core::search::SearchResults;` and drop the now-unused `FtsHit`
import if nothing else needs it.

- [ ] **Step 5: Add serialisation smoke tests**

In the test module at the end of `src-tauri/src/commands.rs`, alongside the
existing ones:

```rust
    #[test]
    fn search_results_and_status_serialise_for_the_frontend() {
        let hit = engram_core::search::Hit {
            path: "A.md".into(),
            title: "A".into(),
            snippet: "<mark>a</mark>".into(),
            heading: Some("H".into()),
            line: 3,
            similarity: Some(0.8),
            score: 0.5,
            past_divider: true,
            primed: false,
        };
        let json = serde_json::to_value(engram_core::search::SearchResults {
            hits: vec![hit],
            associated: vec![],
        })
        .unwrap();
        assert_eq!(json["hits"][0]["past_divider"], true);
        assert_eq!(json["hits"][0]["similarity"], 0.8);
        let status = serde_json::to_value(crate::embed::EmbedStatus::default()).unwrap();
        assert_eq!(status["state"], "off");
        assert_eq!(status["pending"], 0);
    }

    #[test]
    fn an_event_kind_arrives_as_snake_case() {
        let kind: engram_core::memory::EventKind =
            serde_json::from_str("\"open_from_search\"").unwrap();
        assert_eq!(kind, engram_core::memory::EventKind::OpenFromSearch);
    }
```

- [ ] **Step 6: Run everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`
Expected: PASS, clippy and fmt clean.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src
git commit -m "feat(app): an embed thread, hybrid search and memory commands"
```

---

### Task 12: The search pane draws the divider

**Files:**
- Modify: `ui/src/lib/api.ts`, `ui/src/components/Search.svelte`,
  `ui/src/lib/state.svelte.ts`, `ui/src/app.css`
- Test: verified with `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: the `search`, `record_event` and `embed_status` commands.
- Produces: `api.SearchResults`, `api.Hit`, `api.Associated`,
  `api.EmbedStatus`, `api.recordEvent`, `api.related`, `api.forgetMemory`,
  `api.semanticEdges`, `api.embedStatus`, `api.typing`, `api.setModelDir`,
  `api.onEmbedStatus`; `app.openFromSearch(path, line, query)`.

- [ ] **Step 1: Extend the API module**

In `ui/src/lib/api.ts`, replace the `FtsHit` interface and the `search` binding
with:

```ts
export interface Hit {
  path: string; title: string; snippet: string; heading: string | null; line: number;
  similarity: number | null; score: number; past_divider: boolean; primed: boolean;
}
export interface Associated { path: string; title: string; via: string; cue: string | null; strength: number }
export interface SearchResults { hits: Hit[]; associated: Associated[] }
export interface SimilarNote { path: string; title: string; heading: string; text: string; similarity: number }
export interface Related { associated: Associated[]; similar: SimilarNote[]; suggested: SimilarNote[] }
export interface SemanticEdge { source: string; target: string; weight: number; kind: "assoc" | "similar" }
export interface EmbedStatus { model: string | null; state: "off" | "loading" | "ready" | "error"; pending: number; error: string | null }
export type EventKind = "open" | "open_from_search" | "follow_link" | "search";

export const search = (query: string, limit = 50) => invoke<SearchResults>("search", { query, limit });
export const recordEvent = (kind: EventKind, path?: string, query?: string) =>
  invoke<void>("record_event", { kind, path: path ?? null, query: query ?? null });
export const related = (path: string) => invoke<Related>("related", { path });
export const forgetMemory = () => invoke<void>("forget_memory");
export const semanticEdges = (paths: string[] | null, topK = 3) =>
  invoke<SemanticEdge[]>("semantic_edges", { paths, topK });
export const embedStatus = () => invoke<EmbedStatus>("embed_status");
export const typing = () => invoke<void>("typing");
export const setModelDir = (dir: string | null) => invoke<void>("set_model_dir", { dir });
export const onEmbedStatus = (f: (s: EmbedStatus) => void): Promise<UnlistenFn> =>
  listen<EmbedStatus>("embed-status", (e) => f(e.payload));
```

`GraphView.svelte` calls `search(words, 100000)` and reads `hits.map(h => h.path)`
— change it to `(await search(words, 100000)).hits`.

- [ ] **Step 2: Record events in the store**

In `ui/src/lib/state.svelte.ts`, add fields and methods:

```ts
  embed = $state<api.EmbedStatus>({ model: null, state: "off", pending: 0, error: null });
```

In `open()`, after the other listeners:

```ts
    this.embed = await api.embedStatus();
    await api.onEmbedStatus((s) => (this.embed = s));
```

`openNote` records the arrival itself, so every route through it counts once.
Give it the kind and the query:

```ts
  /** Opens `path` in the active pane, or activates its tab there; `line` scrolls to a 1-based line. */
  async openNote(path: string, line?: number, kind: api.EventKind = "open", query?: string) {
```

and at the end of it, after `this.jump` is set:

```ts
    // Memory is a nicety: recording never blocks the open and never toasts.
    void api.recordEvent(kind, path, query).catch(() => {});
```

```ts
  openFromSearch(path: string, line: number | undefined, query: string) {
    return this.openNote(path, line, "open_from_search", query);
  }
```

In `ui/src/components/NoteView.svelte`, `follow` becomes
`await app.openNote(found, line ?? undefined, "follow_link");`.

`record_event` is a no-op in the backend when `memory.enabled` is false, so the
frontend has no condition to carry.

- [ ] **Step 3: Redraw the search pane**

Replace `ui/src/components/Search.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { recordEvent, search, type SearchResults } from "../lib/api";
  let q = $state("");
  let out = $state<SearchResults>({ hits: [], associated: [] });
  let timer: ReturnType<typeof setTimeout> | undefined;
  // The first hit past the fall gets the divider above it.
  const divider = $derived(out.hits.findIndex((h) => h.past_divider));
  $effect(() => {
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      out = query.trim() ? await search(query) : { hits: [], associated: [] };
      if (query.trim()) void recordEvent("search", undefined, query).catch(() => {});
    }, 150);
  });
</script>

<div class="pane-title">Search</div>
<div style="padding:0 12px 8px"><input style="width:100%" placeholder="Search…" bind:value={q} /></div>
{#each out.hits as h, i (h.path)}
  {#if i === divider}<div class="divider-line">loose</div>{/if}
  <button class="linkrow" class:loose={h.past_divider} onclick={() => app.openFromSearch(h.path, h.line, q)}>
    <div class="src">
      {h.title}
      {#if h.heading}<span class="dim">— {h.heading}</span>{/if}
      {#if h.primed}<span class="badge" title="you reach for this one">primed</span>{/if}
    </div>
    <!-- escaped in core; only <mark> survives -->
    <div class="ctx">{@html h.snippet}</div>
  </button>
{/each}
{#if out.associated.length}
  <div class="pane-title sub">Associated</div>
  {#each out.associated as a (a.path)}
    <button class="linkrow" onclick={() => app.openFromSearch(a.path, undefined, q)}>
      <div class="src">{a.title}</div>
      <div class="ctx dim">with {a.via}{a.cue ? ` · “${a.cue}”` : ""}</div>
    </button>
  {/each}
{/if}
{#if app.embed.state === "loading"}<div class="pane-note">downloading model…</div>{/if}
```

- [ ] **Step 4: Style the divider and the loose hits**

Append to `ui/src/app.css`:

```css
/* The relevance divider: everything below it is a loose match. */
.divider-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px 12px 4px;
  font-size: 11px;
  color: var(--text-faint);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.divider-line::after {
  content: "";
  flex: 1;
  border-top: 1px solid var(--border);
}
.linkrow.loose { opacity: 0.62; font-size: 0.92em; }
.pane-title.sub { margin-top: 10px; font-size: 11px; }
.pane-note { padding: 6px 12px; font-size: 11px; color: var(--text-faint); }
.badge {
  margin-left: 6px;
  padding: 0 5px;
  border: 1px solid var(--border);
  border-radius: 8px;
  font-size: 10px;
  color: var(--text-faint);
}
.dim { color: var(--text-faint); }
```

Use the custom-property names `app.css` already defines; if `--text-faint` or
`--border` is spelled differently there, use the existing name.

- [ ] **Step 5: Teach the shot script the new commands**

`ui/scripts/shot.mjs` stubs every command by name and returns `null` for the
rest, so the new ones need entries. In its `switch`, replace the `search` case
and add the others:

```js
        case "search": return FX.search ?? { hits: [], associated: [] };
        case "related": return FX.related ?? { associated: [], similar: [], suggested: [] };
        case "semantic_edges": return FX.semanticEdges ?? [];
        case "embed_status": return FX.embed ?? { model: null, state: "off", pending: 0, error: null };
        case "record_event": case "typing": case "set_config": case "forget_memory": return null;
```

- [ ] **Step 6: Verify in a rendered window**

Write `/tmp/claude-1000/-home-user01-Projekte-engram-notes/d9594166-e9bc-4b44-b466-9517f67e4181/scratchpad/search.json`
from `ui/scripts/fixture.example.json`, with `workspace` opening the search pane
(`leftPane` is store state, so add `EVAL='document.querySelector(".panestrip
button:nth-child(2)").click()'`) and a `search` key holding a `SearchResults`
value: five hits where the last two have `past_divider: true`, one with
`primed: true`, one with a `heading`, and two `associated` entries with cues.

Run:
```bash
cd ui && node scripts/shot.mjs \
  /tmp/claude-1000/-home-user01-Projekte-engram-notes/d9594166-e9bc-4b44-b466-9517f67e4181/scratchpad/search.json \
  /tmp/claude-1000/-home-user01-Projekte-engram-notes/d9594166-e9bc-4b44-b466-9517f67e4181/scratchpad/search.png 1200 800
```
Expected: no console errors; the PNG shows the divider line labelled *loose*,
the hits below it smaller and faded, the primed badge, and the Associated band
with its cue. Read the PNG before calling the task done.

- [ ] **Step 6: Run the checks and commit**

Run: `cd ui && pnpm check && pnpm test`
Expected: PASS.

```bash
git add ui/src docs
git commit -m "feat(ui): the search pane draws the divider and associated notes"
```

---

### Task 13: The Related pane and the link action

**Files:**
- Create: `ui/src/components/Related.svelte`
- Modify: `ui/src/App.svelte`, `ui/src/lib/state.svelte.ts`,
  `ui/src/components/Editor.svelte`, `ui/src/components/NoteView.svelte`
- Test: verified with `ui/scripts/shot.mjs`

**Interfaces:**
- Consumes: `api.related`, `app.lastNote`.
- Produces: `app.insertAtCursor(text: string)` — sets
  `app.insertion = { path, text, n }`, which the mounted editor for that path
  dispatches at its cursor.

- [ ] **Step 1: Give the store an insert request**

In `ui/src/lib/state.svelte.ts`:

```ts
  // The Related pane asks the editor showing this note to insert at the cursor.
  insertion = $state<{ path: string; text: string; n: number } | null>(null);
```

```ts
  insertAtCursor(path: string, text: string) {
    this.insertion = { path, text, n: (this.insertion?.n ?? 0) + 1 };
  }
```

- [ ] **Step 2: Let the editor act on it**

In `ui/src/components/Editor.svelte`, add `insert` to `Props`:

```ts
    insert: { text: string; n: number } | null;
    onInserted: () => void;
```

and an effect after the existing `jump` effect:

```svelte
  $effect(() => {
    const req = insert;
    const v = view;
    if (!req || !v) return;
    const at = v.state.selection.main.head;
    v.dispatch({ changes: { from: at, insert: req.text }, selection: { anchor: at + req.text.length } });
    v.focus();
    onInserted();
  });
```

In `ui/src/components/NoteView.svelte`, pass it through to `<Editor>`:

```svelte
  const insert = $derived(app.insertion && app.insertion.path === path ? app.insertion : null);
```
```svelte
    {insert}
    onInserted={() => (app.insertion = null)}
```

- [ ] **Step 3: Write the pane**

Create `ui/src/components/Related.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, related, type Related } from "../lib/api";

  let data = $state<Related>({ associated: [], similar: [], suggested: [] });
  const path = $derived(app.lastNote);

  $effect(() => {
    const p = path;
    void app.files;
    if (!p) {
      data = { associated: [], similar: [], suggested: [] };
      return;
    }
    related(p).then((r) => (data = r)).catch((e) => app.say(errorMessage(e)));
  });

  function link(title: string) {
    if (!path) return;
    app.insertAtCursor(path, `[[${title}]]`);
  }
</script>

<div class="pane-title">Related</div>
{#if data.associated.length}
  <div class="pane-title sub">Associated</div>
  {#each data.associated as a (a.path)}
    <button class="linkrow" onclick={() => app.openNote(a.path)}>
      <div class="src">{a.title}</div>
      {#if a.cue}<div class="ctx dim">“{a.cue}”</div>{/if}
    </button>
  {/each}
{/if}
{#if data.similar.length}
  <div class="pane-title sub">Similar</div>
  {#each data.similar as s (s.path)}
    <button class="linkrow" onclick={() => app.openNote(s.path)}>
      <div class="src">{s.title}{#if s.heading}<span class="dim"> — {s.heading}</span>{/if}</div>
      <div class="ctx">{s.text}</div>
    </button>
  {/each}
{/if}
{#if data.suggested.length}
  <div class="pane-title sub">Suggested links</div>
  {#each data.suggested as s (s.path)}
    <div class="linkrow suggest">
      <button class="plain" onclick={() => app.openNote(s.path)}>{s.title}</button>
      <button class="chip" onclick={() => link(s.title)}>link</button>
    </div>
  {/each}
{/if}
{#if !data.associated.length && !data.similar.length && !data.suggested.length}
  <div class="pane-note">
    {app.embed.state === "ready" ? "Nothing related yet." : "The model is not ready."}
  </div>
{/if}
```

Append to `ui/src/app.css`:

```css
.linkrow.suggest { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.chip {
  padding: 1px 8px;
  border: 1px solid var(--border);
  border-radius: 10px;
  font-size: 11px;
  background: none;
  color: var(--text-muted);
}
.chip:hover { color: var(--text-normal); }
```

- [ ] **Step 4: Show it in the right sidebar**

In `ui/src/App.svelte`, import `Related` and add it after `Properties`:

```svelte
      {#if app.showRight}<Backlinks /><Outgoing /><Properties /><Related />{/if}
```

- [ ] **Step 5: Verify in a rendered window**

Extend the fixture with a `related` key holding two associated notes with cues,
three similar notes with headings and text, and two suggested notes; the shot
script returns it from the `related` command. Run `shot.mjs` at 1400×900 and
read the PNG: the three groups appear in order with their headings, the *link*
chip is on the suggested rows, and the pane is legible in the dark theme
(`config.theme = "dark"` in a second fixture).

Then check the insertion with `EVAL`: click a *link* chip and assert the editor
document gained `[[Title]]`:

```bash
cd ui && EVAL='document.querySelectorAll(".suggest .chip")[0].click()' \
  node scripts/shot.mjs <fixture> <out.png>
```
Expected: the DOM summary shows the editor text containing `[[`, and
`window.__calls` records no error.

- [ ] **Step 6: Run the checks and commit**

Run: `cd ui && pnpm check && pnpm test`
Expected: PASS.

```bash
git add ui/src
git commit -m "feat(ui): a Related pane that can insert the link it suggests"
```

---

### Task 14: Semantic edges in the graph

**Files:**
- Create: `ui/src/lib/semantic.ts`, `ui/src/lib/semantic.test.ts`
- Modify: `ui/src/lib/graph.ts`, `ui/src/lib/graph.test.ts`,
  `ui/src/components/GraphView.svelte`
- Test: vitest for the pure merge, `shot.mjs` for the canvas

**Interfaces:**
- Consumes: `api.semanticEdges`, `api.SemanticEdge`.
- Produces: `semantic.mergeEdges(links: GraphEdge[], semantic: SemanticEdge[],
  on: boolean) -> DrawEdge[]` where
  `DrawEdge = { source: string; target: string; kind: "link" | "assoc" |
  "similar"; weight: number }`; `GraphSettings` gains `showSemantic` (default
  `false`) and `showSemanticLocal` (default `true`).

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/semantic.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { mergeEdges } from "./semantic";

const links = [{ source: "a.md", target: "b.md" }];
const semantic = [
  { source: "a.md", target: "c.md", weight: 0.9, kind: "similar" as const },
  { source: "a.md", target: "b.md", weight: 4, kind: "assoc" as const },
];

describe("mergeEdges", () => {
  it("returns links alone when semantic edges are off", () => {
    expect(mergeEdges(links, semantic, false)).toEqual([
      { source: "a.md", target: "b.md", kind: "link", weight: 1 },
    ]);
  });

  it("adds semantic edges and never doubles a pair a link already draws", () => {
    const out = mergeEdges(links, semantic, true);
    expect(out).toHaveLength(2);
    expect(out[0].kind).toBe("link");
    expect(out[1]).toEqual({ source: "a.md", target: "c.md", kind: "similar", weight: 0.9 });
  });

  it("reads a pair the same way round as the link that drew it", () => {
    const out = mergeEdges([{ source: "b.md", target: "a.md" }], semantic, true);
    expect(out.filter((e) => e.kind === "assoc")).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cd ui && pnpm test semantic`
Expected: FAIL — cannot resolve `./semantic`.

- [ ] **Step 3: Write the merge**

Create `ui/src/lib/semantic.ts`:

```ts
import type { GraphEdge, SemanticEdge } from "./api";

export interface DrawEdge {
  source: string;
  target: string;
  kind: "link" | "assoc" | "similar";
  weight: number;
}

const key = (a: string, b: string) => (a < b ? `${a} ${b}` : `${b} ${a}`);

/** Explicit links, then the dashed ones, with no pair drawn twice. */
export function mergeEdges(links: GraphEdge[], semantic: SemanticEdge[], on: boolean): DrawEdge[] {
  const out: DrawEdge[] = links.map((e) => ({ source: e.source, target: e.target, kind: "link", weight: 1 }));
  if (!on) return out;
  const seen = new Set(out.map((e) => key(e.source, e.target)));
  for (const e of semantic) {
    const k = key(e.source, e.target);
    if (seen.has(k)) continue;
    seen.add(k);
    out.push({ source: e.source, target: e.target, kind: e.kind, weight: e.weight });
  }
  return out;
}
```

- [ ] **Step 4: Run the tests**

Run: `cd ui && pnpm test semantic`
Expected: PASS, 3 tests.

- [ ] **Step 5: Carry the edge kind through the filter**

In `ui/src/lib/graph.ts`:

- Add to `GraphSettings` and `DEFAULTS`: `showSemantic: false` and
  `showSemanticLocal: true`. Spec: off in the global graph, on in the local one.
- Change `ViewGraph` to `{ nodes: ViewNode[]; edges: DrawEdge[] }` and import
  `DrawEdge` from `./semantic`.
- `filterGraph(g, s, content, center, semantic: SemanticEdge[] = [])` builds
  its edge list with
  `mergeEdges(g.edges, semantic, center ? s.showSemanticLocal : s.showSemantic)`
  before filtering, and the tag edges it pushes get
  `kind: "link", weight: 1`.
- `neighbourhood` takes `DrawEdge[]`; its body is unchanged.

Update `ui/src/lib/graph.test.ts` where it asserts on `edges`: an edge is now
`{ source, target, kind: "link", weight: 1 }`. Add one case:

```ts
it("draws semantic edges only where the toggle for that graph says so", () => {
  const g = { nodes: [note("a.md"), note("b.md")], edges: [] };
  const sem = [{ source: "a.md", target: "b.md", weight: 0.8, kind: "similar" as const }];
  expect(filterGraph(g, DEFAULTS, null, null, sem).edges).toHaveLength(0);
  expect(filterGraph(g, DEFAULTS, null, "a.md", sem).edges).toHaveLength(1);
});
```

using whatever `note()` helper that file already has.

- [ ] **Step 6: Load and draw them**

In `ui/src/components/GraphView.svelte`:

```ts
  import { semanticEdges, type SemanticEdge } from "../lib/api";
  import { mergeEdges } from "../lib/semantic";
  let semantic = $state.raw<SemanticEdge[]>([]);
  const semanticOn = $derived(local ? settings.showSemanticLocal : settings.showSemantic);
```

```ts
  // The local graph asks for the centre's edges; the global graph for the vault's.
  $effect(() => {
    void app.files;
    if (!semanticOn) {
      semantic = [];
      return;
    }
    const paths = local ? (center ? [center] : []) : null;
    if (local && !center) return;
    semanticEdges(paths).then((e) => (semantic = e)).catch(say);
  });
```

Pass `semantic` as the fifth argument of `filterGraph` in the `shown` derived
value.

The simulation's own edge type carries the kind through to the renderer:

```ts
  interface Link { source: Node; target: Node; kind: DrawEdge["kind"]; weight: number }
```
```ts
    links = g.edges.map((e) => ({
      source: byId.get(e.source)!,
      target: byId.get(e.target)!,
      kind: e.kind,
      weight: e.weight,
    }));
```

In the renderer, draw a dashed line for a non-`link` edge, its width from the
weight, and reset the dash afterwards so the solid edges are unaffected:

```ts
      ctx.setLineDash(e.kind === "link" ? [] : [4 / view.k, 3 / view.k]);
      ctx.lineWidth = (e.kind === "link" ? 1 : Math.min(2, 0.5 + e.weight)) * settings.lineSizeMultiplier / view.k;
```
followed by `ctx.setLineDash([])` once the edge pass is done. Batch the dashed
edges after the solid ones so the dash state is set twice per frame, not per
edge — the renderer already batches by colour.

Add the toggle to the settings panel, beside *Show arrows*:

```svelte
      <label><input type="checkbox" checked={semanticOn}
        onchange={(e) => {
          const on = e.currentTarget.checked;
          if (local) settings.showSemanticLocal = on; else settings.showSemantic = on;
        }} /> Semantic edges</label>
```

- [ ] **Step 7: Verify in a rendered window**

Extend the fixture with a `semanticEdges` key returning three edges (two
`similar`, one `assoc`) between notes the `graph` fixture already has, and
`graphConfig` with `showSemantic: true`.

Run:
```bash
cd ui && SETTLE=5000 node scripts/shot.mjs <fixture> <out.png> 1200 900
```
Expected: the PNG shows dashed edges among the solid ones, thicker where the
weight is higher, and the *Semantic edges* checkbox in the panel. Take a second
shot with the checkbox off (`EVAL` clicking it) and confirm the dashes are gone
and the solid edges are still solid.

- [ ] **Step 8: Run the checks and commit**

Run: `cd ui && pnpm check && pnpm test`
Expected: PASS.

```bash
git add ui/src
git commit -m "feat(graph): dashed semantic edges with a toggle per graph"
```

---

### Task 15: The status bar, the palette commands and the docs

**Files:**
- Modify: `ui/src/components/StatusBar.svelte`, `ui/src/lib/commands.ts`,
  `ui/src/lib/commands.test.ts`, `ui/src/components/NoteView.svelte`,
  `docs/smoke.md`
- Create: `docs/memory.md`
- Test: vitest for the commands, `shot.mjs` for the bar

**Interfaces:**
- Consumes: `app.embed`, `api.typing`, `api.forgetMemory`, `api.setModelDir`,
  `api.setConfig`.
- Produces: palette commands `memory:toggle`, `memory:forget`,
  `embed:model-dir`.

- [ ] **Step 1: Show the queue and the memory toggle**

Replace `ui/src/components/StatusBar.svelte`:

```svelte
<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, setConfig } from "../lib/api";
  const words = $derived(app.activeDoc ? app.activeDoc.text.split(/\s+/).filter(Boolean).length : 0);
  const notes = $derived(app.files.filter((f) => f.is_markdown).length);
  const memory = $derived(app.config?.memory.enabled ?? false);
  const embed = $derived(app.embed);

  async function toggleMemory() {
    const cfg = app.config;
    if (!cfg) return;
    cfg.memory.enabled = !cfg.memory.enabled;
    try {
      await setConfig($state.snapshot(cfg));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
</script>

<div class="statusbar">
  <button onclick={() => (app.showLeft = !app.showLeft)} title="Toggle left sidebar">☰</button>
  <span>{notes} notes</span>
  {#if app.activeDoc}<span>{words} words</span>{/if}
  <span style="flex:1"></span>
  {#if embed.state === "loading"}<span>downloading model</span>
  {:else if embed.state === "error"}<span title={embed.error ?? ""}>embedding off</span>
  {:else if embed.pending > 0}<span>{embed.pending} passages pending</span>{/if}
  <button onclick={toggleMemory} title={memory ? "Memory is on" : "Memory is off"}>
    {memory ? "memory on" : "memory off"}
  </button>
  <button onclick={() => (app.showRight = !app.showRight)} title="Toggle right sidebar">☰</button>
</div>
```

- [ ] **Step 2: Write the failing command test**

Add to `ui/src/lib/commands.test.ts`:

```ts
it("offers the memory and model commands", () => {
  const ids = defaults.map((c) => c.id);
  expect(ids).toContain("memory-toggle");
  expect(ids).toContain("memory-forget");
  expect(ids).toContain("model-dir");
});
```

adding `defaults` to that file's import from `./commands`.

- [ ] **Step 3: Run it to make sure it fails**

Run: `cd ui && pnpm test commands`
Expected: FAIL — the ids are missing.

- [ ] **Step 4: Add the commands**

In `ui/src/lib/commands.ts`, append to the `defaults` array — the shape is
`{ id, name, hotkey, run }`, and these three have no default shortcut:

```ts
  {
    id: "memory-toggle",
    name: "Memory: turn on or off",
    hotkey: "",
    run: async () => {
      const cfg = app.config;
      if (!cfg) return;
      cfg.memory.enabled = !cfg.memory.enabled;
      await setConfig($state.snapshot(cfg));
      app.say(cfg.memory.enabled ? "Memory is on." : "Memory is off; what it learned is kept.");
    },
  },
  {
    id: "memory-forget",
    name: "Memory: forget everything learned",
    hotkey: "",
    run: async () => {
      await forgetMemory();
      app.say("Memory forgotten.");
    },
  },
  {
    id: "model-dir",
    name: "Embedding: choose the model folder",
    hotkey: "",
    run: async () => {
      const dir = await open({ directory: true });
      if (typeof dir !== "string") return;
      await setModelDir(dir);
      app.say("Loading the model from that folder.");
    },
  },
```

Extend the existing `import { dailyNote, createNote } from "./api";` with
`setConfig`, `forgetMemory` and `setModelDir`, and add
`import { open } from "@tauri-apps/plugin-dialog";` — `VaultPicker.svelte`
already uses that plugin, so it needs no new dependency.

- [ ] **Step 5: Pause the queue while typing**

In `ui/src/components/NoteView.svelte`, in `onChange`, beside the autosave
timer:

```ts
    void typing().catch(() => {});
```
with `typing` added to the imports from `../lib/api`. One ping per keystroke is
cheap; the backend only stamps an `Instant`.

- [ ] **Step 6: Run the tests**

Run: `cd ui && pnpm check && pnpm test`
Expected: PASS.

- [ ] **Step 7: Verify the bar in a rendered window**

Use a fixture where `embedStatus` returns `{ state: "ready", pending: 128 }` and
`config.memory.enabled` is true; run `shot.mjs` and read the PNG: the bar reads
*128 passages pending* and *memory on*. Take a second shot with
`{ state: "loading" }` and confirm it reads *downloading model*.

- [ ] **Step 8: Write `docs/memory.md`**

Create it with the numbers and the departures:

```markdown
# Search and memory

What engram-notes borrowed from engram, with the numbers it ships and the
places it does not follow the spec.

## Retrieval

Full-text (FTS5 BM25, title weighted 10) and semantic (cosine over every
passage vector) each fetch `limit × 3` candidates. Reciprocal rank fusion with
`k = 60` folds them into one list, one entry per note, its best passage as the
snippet.

Passages are the body split on headings, then on paragraphs, at most 1 200
characters, the heading path prepended before embedding. A vector is keyed by
the hash of that text, so an unchanged passage in a renamed or re-saved note
keeps it.

**The divider** is engram's cliff, not the spec's fraction of the top score.
Over the cosine similarities sorted descending, at least three of them, the
largest gap must exceed `3.0 ×` the mean of the other gaps and `0.01 ×` the top
score. It reads similarity, never the fused score, whose first gap is
structurally the largest. Everything from the fall on is marked loose, in rank
order, and shown smaller. AGENTS.md: engram is the reference for the concepts
it inspired.

**The model** is `multilingual-e5-small`, unquantized: fastembed ships no
quantized e5-small, though the spec asks for one. A quantized ONNX still loads
through *Embedding: choose the model folder*, which exists for machines with no
network. `query: ` and `passage: ` prefixes are added here, because fastembed
does not add them.

## Memory

Every value is `(value, stamped_at)` read through
`decayed(value, stamped_at, now, half_life) = value · 2^(−elapsed / half_life)`,
elapsed floored at zero. Learning is one write; forgetting costs nothing.

| | value |
| --- | --- |
| Activation half-life | 30 days |
| Association half-life | 90 days |
| Sitting gap | 30 minutes |
| Association window in a sitting | 10 minutes |
| An association shows above | 2.0 |
| Priming margin, lift | 0.5, 2 places |
| Spread | 3 notes, one hop |
| Cues kept per link | 3, the busiest |

A sitting is a column on `events`, not a table: it is a gap and nothing else.
One co-appearance adds 1.0, so a pair has to be met twice before it shows.
Priming is rank-based, decided against the original order in one pass; rows 0
and 1 never move and only hits above the list's median activation may climb.
engram subtracts a decayed baseline from activation; here a note starts with no
row at all, so there is none.

`activation` and `assoc` have no foreign key to `notes`: a note deleted and
restored keeps what it learned, and queries join `notes` so stale rows never
surface. A rename moves the rows. A **schema bump drops them** with the rest of
the index — memory is the one derived thing the files cannot rebuild, and the
spec's "a schema change rebuilds rather than migrates" wins anyway.

`memory.enabled` off records no events and bumps nothing; priming and spread
are skipped and the Related pane shows *similar* only. What was learned is
kept, and *Memory: forget everything learned* empties it.
```

- [ ] **Step 9: Add the smoke lines**

Append to `docs/smoke.md`, numbered on from 33 (renumber the final
`cargo test` line to last):

```markdown
34. On first open the status bar says *downloading model*, then counts passages
    pending down to nothing; full-text search answers throughout.
35. Search a phrase no note contains word for word: semantic hits appear, the
    divider is drawn above the loose ones and they are smaller.
36. Open two notes from the same search, search again a minute later: the pair
    shows under *Associated* with the query as its cue.
37. Open one note ten times, search for something it matches weakly: it carries
    the *primed* badge and has climbed at most two places.
38. The Related pane lists associated, similar and suggested links; *link*
    inserts `[[Title]]` at the cursor.
39. Turn memory off in the status bar: *Associated* and the badges go, *Similar*
    stays. *Memory: forget everything learned* empties it and the pane with it.
40. Turn on semantic edges in the local graph: dashed edges appear, thicker where
    the association is stronger, and the global graph still has none.
41. *Embedding: choose the model folder* on a folder with the ONNX file and
    tokenizer loads it with no network, and the vectors are rebuilt.
```

- [ ] **Step 10: Run the full check**

Run:
```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test --workspace && cd ui && pnpm check && pnpm test
```
Expected: all PASS.

- [ ] **Step 11: Commit**

```bash
git add ui/src docs
git commit -m "feat(ui): the embedding queue, the memory toggle and its commands"
```

---

## After the last task

1. Push the branch and wait for CI. The Rust job now builds fastembed for
   `src-tauri`, so the first run is slower; `ort` fetches onnxruntime at build
   time over rustls, which needs no OpenSSL package.
2. Build the real window: `cd ui && pnpm tauri build --debug --no-bundle`, run
   `target/debug/engram-notes <demo vault>` outside the sandbox. A demo vault
   with twenty or so notes on two or three subjects makes the divider and the
   Related pane show something.
3. Ask the user for screenshots of the search pane with a divider, the Related
   pane, the status bar while the model downloads, and the local graph with
   semantic edges on. Review them together.
4. Then ask whether to merge locally, open a pull request, or keep the branch.
