# Out of the box (0.6) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The embedder and a cross-encoder reranker ship as Tauri resources
inside every release artifact, the reranker becomes the stage between fusion
and the divider for deliberate search and the passage picker inside a 500 ms
budget, and nothing in the binary can reach the network.

**Architecture:** `core::embed` gains a `Reranker` trait, a deterministic
fake, a timing wrapper, and a fastembed-backed `FastReranker` loaded from a
folder. `search::hybrid_hits` takes an optional reranker and reorders the top
`rerank_n` fused hits by its score; the divider reads that score where it
exists. The shell loads both models from `resource_dir()/models/…` (or the
override folders in `app.json`), warms the reranker, enforces the budget by
measurement, and reports both in the existing embed status. A pinned fetch
script fills `src-tauri/models/` for developers and CI; `bundle.resources`
carries the folder into every artifact.

**Tech Stack:** Rust 2024 (`fastembed` 6.1 with `ort`, `rusqlite`,
`tempfile` for tests), Tauri 2, Svelte 5, TypeScript, vitest, GitHub
Actions, POSIX sh.

**Spec:** `docs/superpowers/specs/2026-09-16-out-of-the-box-design.md`. Its
parent is `docs/superpowers/specs/2026-09-14-writing-first-direction-design.md`
(section *Retrieval*). `ROADMAP.md` step 0.6 is the summary.

**Branch:** `feat/out-of-the-box`, created from `master` at `74ce3a8`; the
spec commits `11b99b9` and its follow-up are on it. Stay on it. No worktree:
`target/` fills this disk (`cargo test --workspace` grows it to ~12 GB), so
one checkout shares one target directory.

---

## Global Constraints

- Rust 2024 edition, stable toolchain, `cargo fmt` and `cargo clippy
  --workspace -- -D warnings` clean before every commit.
- `core` has no Tauri dependency; the frontend never touches the filesystem.
- Tests run without a model, a window or the network. The embedder and the
  reranker have deterministic fakes.
- Errors are typed in `core::Error`; the UI shows them, never swallows them.
- Comments say why, not what. Doc comments on public items are one sentence
  unless the contract is worth stating.
- Commit messages: conventional prefix, imperative subject under 72
  characters, body only when the why is not in the subject, ending with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Build environment: `export PATH="$HOME/.cargo/bin:$PATH"` before any
  cargo command; prefer `CARGO_INCREMENTAL=0`; run `df -h /home/user01`
  before a `src-tauri` build and stop if under 4 GB free.
- Numbers from the spec: `rerank_n` 20, `rerank_floor` 0.1,
  `rerank_budget_ms` 500. Embedder id `multilingual-e5-small-int8`. Reranker
  id `mmarco-mMiniLMv2-L12-H384-v1-int8`.
- Bundled folders: `<resource_dir>/models/embedder` and
  `<resource_dir>/models/reranker`, each holding `model.onnx`,
  `tokenizer.json`, `config.json`, `special_tokens_map.json`,
  `tokenizer_config.json`.
- The frontend copy in the status bar for a reranker over budget is
  `reranking off: 1.2 s on this machine` (one decimal, the measured time).

---

## File Structure

| File | Responsibility |
|---|---|
| `core/src/embed/mod.rs` | `Embedder` (as is), new `Reranker` trait, `FakeReranker`, `TimedReranker`, `sigmoid` |
| `core/src/embed/fastembed.rs` | `FastEmbedder::from_dir(dir, id)` (download removed), new `FastReranker::from_dir(dir, id)` |
| `core/src/config.rs` | `SearchConfig` gains `rerank_n`, `rerank_floor`, `rerank_budget_ms`; `EmbedConfig` gains `reranker_dir` |
| `core/src/search/vector.rs` | `Index::passage_text(path)` |
| `core/src/search/mod.rs` | `Hit.rerank`; `hybrid`/`hybrid_hits` take `Option<&mut dyn Reranker>`; the rerank step |
| `core/src/search/fuse.rs` | `mark_past_divider` reads `rerank` where present |
| `core/src/search/candidates.rs`, `core/src/memory/prime.rs`, `core/src/memory/spread.rs` | pass `None`; `Hit` constructors gain `rerank: None` |
| `core/Cargo.toml` | drop `hf-hub-rustls-tls` |
| `src-tauri/src/embed.rs` | load both models from resources or overrides, warm-up, status fields, `publish` becomes `pub(crate)` |
| `src-tauri/src/commands.rs` | `search` passes the reranker and enforces the budget; `set_reranker_dir` |
| `src-tauri/src/lib.rs` | register `set_reranker_dir` |
| `src-tauri/tauri.conf.json` | `bundle.resources` |
| `scripts/fetch-models.sh` | pinned download into `src-tauri/models/` |
| `.gitignore` | `/src-tauri/models` |
| `ui/src/lib/api.ts` | types and `setRerankerDir` |
| `ui/src/components/StatusBar.svelte` | loading copy, reranker-off line |
| `ui/src/components/Settings.svelte` | *Bundled* label, reranker folder row |
| `ui/src/lib/commands.test.ts` | config fixture |
| `.github/workflows/build.yml`, `.github/workflows/release.yml` | fetch step, folder archives |
| `install.sh` | slim tarball installs `bin/` and `lib/engram-notes/` |
| `docs/memory.md`, `docs/smoke.md`, `ROADMAP.md` | the record |

---

### Task 1: The `Reranker` seam in core

**Files:**
- Modify: `core/src/embed/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  pub trait Reranker: Send {
      fn id(&self) -> String;
      fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>>;
  }
  pub struct FakeReranker;
  pub struct TimedReranker { /* private */ }
  impl TimedReranker {
      pub fn new(inner: Box<dyn Reranker>) -> TimedReranker;
      pub fn last(&self) -> Option<std::time::Duration>;
      pub fn last_error(&self) -> Option<String>;
  }
  impl Reranker for TimedReranker { … }
  pub fn sigmoid(x: f32) -> f32;
  ```

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `core/src/embed/mod.rs`:

```rust
    #[test]
    fn the_fake_reranker_scores_the_share_of_query_words_present() {
        let mut r = FakeReranker;
        assert_eq!(r.id(), "fake-reranker");
        let s = r
            .score(
                "rust ownership",
                &[
                    "rust ownership rules".to_string(),
                    "ownership alone".to_string(),
                    "coffee".to_string(),
                ],
            )
            .unwrap();
        assert_eq!(s, vec![1.0, 0.5, 0.0]);
        // Same answer every run, and an empty query says nothing about anything.
        assert_eq!(r.score("", &["a".to_string()]).unwrap(), vec![0.0]);
    }

    #[test]
    fn the_timed_wrapper_records_the_last_run() {
        let mut t = TimedReranker::new(Box::new(FakeReranker));
        assert!(t.last().is_none());
        assert_eq!(t.id(), "fake-reranker");
        let s = t.score("a", &["a b".to_string()]).unwrap();
        assert_eq!(s, vec![1.0]);
        assert!(t.last().is_some());
        assert!(t.last_error().is_none());
    }

    #[test]
    fn the_timed_wrapper_keeps_the_error_of_a_failing_reranker() {
        struct Broken;
        impl Reranker for Broken {
            fn id(&self) -> String {
                "broken".into()
            }
            fn score(&mut self, _: &str, _: &[String]) -> Result<Vec<f32>> {
                Err(crate::Error::Embed("no".into()))
            }
        }
        let mut t = TimedReranker::new(Box::new(Broken));
        assert!(t.score("a", &["b".to_string()]).is_err());
        assert_eq!(t.last_error().as_deref(), Some("embedding: no"));
        // A good run clears it.
    }

    #[test]
    fn sigmoid_maps_logits_into_the_unit_interval() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.99);
        assert!(sigmoid(-10.0) < 0.01);
    }
```

Check the `Error::Embed` display string first: run
`grep -n 'Embed' core/src/error.rs`. If the display is not
`embedding: {0}`, change the assertion to match what it prints.

- [ ] **Step 2: Run the tests to see them fail**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p engram-notes-core embed:: 2>&1 | tail -20
```

Expected: compile error, `FakeReranker`, `TimedReranker`, `sigmoid` not found.

- [ ] **Step 3: Implement**

In `core/src/embed/mod.rs`, after the `Embedder` trait:

```rust
/// The cross-encoder seam: a query against a few documents, one score each.
pub trait Reranker: Send {
    /// Identifies the model; the status bar shows it.
    fn id(&self) -> String;
    /// One score per document, higher is more relevant, in `documents` order.
    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>>;
}

/// Scores the share of the query's words the document contains. No model,
/// same answer every run, and a test can arrange the order it wants.
pub struct FakeReranker;

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter()
}

impl Reranker for FakeReranker {
    fn id(&self) -> String {
        "fake-reranker".into()
    }

    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let q: Vec<String> = words(query).collect();
        Ok(documents
            .iter()
            .map(|d| {
                if q.is_empty() {
                    return 0.0;
                }
                let have: std::collections::HashSet<String> = words(d).collect();
                q.iter().filter(|w| have.contains(*w)).count() as f32 / q.len() as f32
            })
            .collect())
    }
}

/// A reranker that remembers how long its last run took and whether it
/// failed, so the shell can hold it to a budget without core knowing one.
pub struct TimedReranker {
    inner: Box<dyn Reranker>,
    last: Option<std::time::Duration>,
    last_error: Option<String>,
}

impl TimedReranker {
    pub fn new(inner: Box<dyn Reranker>) -> TimedReranker {
        TimedReranker {
            inner,
            last: None,
            last_error: None,
        }
    }

    pub fn last(&self) -> Option<std::time::Duration> {
        self.last
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.clone()
    }
}

impl Reranker for TimedReranker {
    fn id(&self) -> String {
        self.inner.id()
    }

    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let start = std::time::Instant::now();
        let out = self.inner.score(query, documents);
        self.last = Some(start.elapsed());
        self.last_error = out.as_ref().err().map(|e| e.to_string());
        out
    }
}

/// A cross-encoder's logit as a probability, so the divider reads it like a cosine.
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p engram-notes-core embed:: 2>&1 | tail -20
```

Expected: all embed tests pass.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt --all && cargo clippy -p engram-notes-core -- -D warnings
git add core/src/embed/mod.rs
git commit -m "feat(embed): the Reranker seam, a fake and a timed wrapper

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Config for reranking and the reranker folder

**Files:**
- Modify: `core/src/config.rs` (`SearchConfig` ~line 64, `EmbedConfig` ~line 122, test ~line 355)
- Modify: `ui/src/lib/api.ts:13,19`
- Modify: `ui/src/lib/commands.test.ts:23,29`

**Interfaces:**
- Produces: `SearchConfig { …, rerank_n: usize, rerank_floor: f32, rerank_budget_ms: u64 }`, `EmbedConfig { model_dir, reranker_dir: Option<String>, batch }`.

- [ ] **Step 1: Extend the defaults test**

In `core/src/config.rs`, test `search_and_memory_defaults_are_the_shipped_numbers`, add after the `similarity_floor` assertion:

```rust
        assert_eq!(cfg.search.rerank_n, 20);
        assert_eq!(cfg.search.rerank_floor, 0.1);
        assert_eq!(cfg.search.rerank_budget_ms, 500);
```

and after `assert_eq!(cfg.embed.model_dir, None);`:

```rust
        assert_eq!(cfg.embed.reranker_dir, None);
```

- [ ] **Step 2: Run to see it fail**

```bash
cargo test -p engram-notes-core config:: 2>&1 | tail -5
```

Expected: compile error, no field `rerank_n`.

- [ ] **Step 3: Add the fields**

In `SearchConfig`, after `similarity_floor`:

```rust
    /// How many fused hits the cross-encoder rescores. Its cost is the
    /// dominant one in a search, so this is what the budget trades against.
    pub rerank_n: usize,
    /// Below this a rerank score is a stranger. A MiniLM cross-encoder's
    /// sigmoid sits near 0 or 1, so this only has to sort the two bands.
    pub rerank_floor: f32,
    /// A rerank run longer than this switches reranking off for the session.
    pub rerank_budget_ms: u64,
```

In `Default for SearchConfig`: `rerank_n: 20, rerank_floor: 0.1, rerank_budget_ms: 500,`.

In `EmbedConfig`, after `model_dir`:

```rust
    /// The same for the reranker.
    pub reranker_dir: Option<String>,
```

and `reranker_dir: None,` in its default. Change the doc comment on
`model_dir` to: `/// A folder with the ONNX file and tokenizer, replacing the bundled embedder.`

- [ ] **Step 4: Run**

```bash
cargo test -p engram-notes-core config:: 2>&1 | tail -5
```

Expected: pass.

- [ ] **Step 5: Frontend types and fixture**

`ui/src/lib/api.ts` line 13, the `search` type becomes:

```ts
  search: { candidate_multiplier: number; rrf_k: number; cliff_factor: number; cliff_min_share: number; similarity_floor: number; rerank_n: number; rerank_floor: number; rerank_budget_ms: number };
```

line 19: `embed: { model_dir: string | null; reranker_dir: string | null; batch: number };`

`ui/src/lib/commands.test.ts` line 23 and 29 likewise:

```ts
      search: { candidate_multiplier: 3, rrf_k: 60, cliff_factor: 3, cliff_min_share: 0.01, similarity_floor: 0.83, rerank_n: 20, rerank_floor: 0.1, rerank_budget_ms: 500 },
```
```ts
      embed: { model_dir: null, reranker_dir: null, batch: 32 },
```

Run: `cd ui && pnpm vitest run 2>&1 | tail -5`. Expected: pass.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add core/src/config.rs ui/src/lib/api.ts ui/src/lib/commands.test.ts
git commit -m "feat(config): rerank_n, rerank_floor, rerank_budget_ms and reranker_dir

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: `Index::passage_text`

**Files:**
- Modify: `core/src/search/vector.rs` (impl block near line 44)

**Interfaces:**
- Produces: `pub fn passage_text(&self, path: &str) -> Result<Option<String>>` on `Index`.

- [ ] **Step 1: Write the failing test**

In `core/src/search/vector.rs` `mod tests` (line ~203; it already has a
vault-building helper — read it and reuse whatever builds an `Index` from a
temp vault, else copy `vault_with_vectors` from `core/src/search/mod.rs`
tests):

```rust
    #[test]
    fn passage_text_is_the_note_body_the_embedder_saw() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "---\nk: v\n---\n# A\nbody words").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let text = ix.passage_text("A.md").unwrap().unwrap();
        assert!(text.contains("body words"), "{text}");
        assert!(!text.contains("k: v"));
        assert_eq!(ix.passage_text("missing.md").unwrap(), None);
    }
```

- [ ] **Step 2: Run to see it fail**

```bash
cargo test -p engram-notes-core passage_text 2>&1 | tail -5
```

Expected: no method `passage_text`.

- [ ] **Step 3: Implement**

In the `impl Index` block of `core/src/search/vector.rs`, after `vector_of`:

```rust
    /// The note's one passage, for a caller that has the path and not the hit.
    pub fn passage_text(&self, path: &str) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT text FROM passages WHERE path=?1 ORDER BY ordinal LIMIT 1",
                [path],
                |r| r.get(0),
            )
            .optional()?)
    }
```

`optional` is already imported in this file (`vector_of` uses it).

- [ ] **Step 4: Run, commit**

```bash
cargo test -p engram-notes-core passage_text 2>&1 | tail -5
cargo fmt --all
git add core/src/search/vector.rs
git commit -m "feat(index): passage_text for a note the text branch alone found

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: The rerank step in `hybrid_hits` and the divider on its score

**Files:**
- Modify: `core/src/search/mod.rs` (`Hit`, `hybrid`, `hybrid_hits`, tests)
- Modify: `core/src/search/fuse.rs` (`mark_past_divider`, `tail_from`, test helper)
- Modify: `core/src/search/candidates.rs:78`
- Modify: `core/src/memory/prime.rs:76`, `core/src/memory/spread.rs:51`
- Modify: `src-tauri/src/commands.rs:387,900` (compile only; the shell's real wiring is Task 6)

**Interfaces:**
- Consumes: `Reranker` (Task 1), `SearchConfig.rerank_n/rerank_floor` (Task 2), `Index::passage_text` (Task 3).
- Produces:
  ```rust
  pub struct Hit { …, pub rerank: Option<f32>, … }
  pub fn hybrid(index, query, query_vec, reranker: Option<&mut dyn Reranker>, cfg, mem, at, limit) -> Result<SearchResults>
  pub fn hybrid_hits(index, query, query_vec, reranker: Option<&mut dyn Reranker>, cfg, mem, at, limit) -> Result<Vec<Hit>>
  ```
  (`reranker` is the fourth parameter, right after `query_vec`.)

- [ ] **Step 1: Write the failing tests**

In `core/src/search/mod.rs` `mod tests`, add:

```rust
    /// Scores by a fixed table, so the test decides the order the reranker wants.
    struct Table(std::collections::HashMap<&'static str, f32>);
    impl crate::embed::Reranker for Table {
        fn id(&self) -> String {
            "table".into()
        }
        fn score(&mut self, _q: &str, docs: &[String]) -> crate::Result<Vec<f32>> {
            Ok(docs
                .iter()
                .map(|d| {
                    self.0
                        .iter()
                        .find(|(k, _)| d.contains(**k))
                        .map_or(0.0, |(_, v)| *v)
                })
                .collect())
        }
    }

    #[test]
    fn the_reranker_reorders_the_top_hits_and_marks_their_scores() {
        let (_d, ix, _e) = vault_with_vectors();
        // Full text alone finds Coffee (kettle water grind beans) and Tea
        // (leaves steep water) for "water"; the table prefers Tea.
        let mut table = Table([("steep", 0.9f32), ("kettle", 0.05f32)].into_iter().collect());
        let out = hybrid(
            &ix,
            "water",
            None,
            Some(&mut table),
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        let paths: Vec<&str> = out.hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths, vec!["Tea.md", "Coffee.md"]);
        assert_eq!(out.hits[0].rerank, Some(0.9));
        assert_eq!(out.hits[1].rerank, Some(0.05));
        // The divider reads the rerank score: 0.05 is under the floor.
        assert!(!out.hits[0].past_divider);
        assert!(out.hits[1].past_divider);
        // Cosine is untouched: nothing dense ran.
        assert!(out.hits.iter().all(|h| h.similarity.is_none()));
    }

    #[test]
    fn without_a_reranker_hits_carry_no_rerank_score() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = hybrid(
            &ix,
            "water",
            None,
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out.hits.len(), 2);
        assert!(out.hits.iter().all(|h| h.rerank.is_none()));
    }

    #[test]
    fn a_failing_reranker_leaves_fusion_order_and_no_scores() {
        struct Broken;
        impl crate::embed::Reranker for Broken {
            fn id(&self) -> String {
                "broken".into()
            }
            fn score(&mut self, _: &str, _: &[String]) -> crate::Result<Vec<f32>> {
                Err(crate::Error::Embed("down".into()))
            }
        }
        let (_d, ix, _e) = vault_with_vectors();
        let mut broken = Broken;
        let out = hybrid(
            &ix,
            "water",
            None,
            Some(&mut broken),
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out.hits.len(), 2);
        assert!(out.hits.iter().all(|h| h.rerank.is_none()));
    }

    #[test]
    fn rerank_n_bounds_what_is_rescored() {
        let (_d, ix, _e) = vault_with_vectors();
        let mut table = Table([("steep", 0.9f32)].into_iter().collect());
        let cfg = SearchConfig {
            rerank_n: 1,
            ..SearchConfig::default()
        };
        let out = hybrid_hits(&ix, "water", None, Some(&mut table), &cfg, &MemoryConfig::default(), 0, 10).unwrap();
        // Only the first fused hit was scored; the second keeps its place.
        assert_eq!(out.iter().filter(|h| h.rerank.is_some()).count(), 1);
    }
```

In `core/src/search/fuse.rs` `mod tests`, add:

```rust
    #[test]
    fn the_divider_reads_rerank_scores_when_any_hit_has_one() {
        let mut hits = vec![hit("a", Some(0.9)), hit("b", Some(0.9)), hit("c", Some(0.9))];
        hits[0].rerank = Some(0.95);
        hits[1].rerank = Some(0.90);
        hits[2].rerank = Some(0.02);
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(!hits[0].past_divider && !hits[1].past_divider);
        assert!(hits[2].past_divider, "0.02 is under rerank_floor 0.1");
    }
```

Every existing `hybrid(`/`hybrid_hits(` call in tests gets a `None,`
inserted after the `query_vec` argument.

- [ ] **Step 2: Run to see them fail**

```bash
cargo test -p engram-notes-core search:: 2>&1 | grep -E 'error|no field' | head
```

Expected: no field `rerank`, wrong argument count.

- [ ] **Step 3: `Hit.rerank` and every constructor**

In `core/src/search/mod.rs`, `struct Hit`, after `similarity`:

```rust
    /// The cross-encoder's score in `(0, 1)`, where reranking ran.
    pub rerank: Option<f32>,
```

Add `rerank: None,` to the `Hit { … }` literals at `core/src/search/mod.rs`
(inside `hybrid_hits`), `core/src/memory/prime.rs:76`,
`core/src/memory/spread.rs:51`, `core/src/search/fuse.rs:101`, and
`src-tauri/src/commands.rs:900`.

- [ ] **Step 4: The signatures and the step**

`hybrid` and `hybrid_hits` in `core/src/search/mod.rs` gain
`reranker: Option<&mut dyn crate::embed::Reranker>,` after `query_vec`;
`hybrid` passes it through. Update the doc comment on `hybrid`: replace the
last sentence "which is why search works during the first download" with
"which is why search works while the models load."

Replace the body of `hybrid_hits` from `let mut hits: Vec<Hit> = fuse::rrf(…)`
through the `.collect();` with:

```rust
    // With a reranker, more than `limit` fused entries are built so a note at
    // fused rank 15 can be lifted into a list of 10.
    let build = match reranker.is_some() {
        true => limit.max(cfg.rerank_n),
        false => limit,
    };
    let mut hits: Vec<Hit> = fuse::rrf(&[dense, sparse], cfg.rrf_k)
        .into_iter()
        .take(build)
        .map(|(path, score)| {
            let passage = best.get(&path);
            let text = by_path.get(path.as_str());
            Hit {
                title: titles.get(&path).cloned().unwrap_or_else(|| path.clone()),
                snippet: match text {
                    Some(h) => h.snippet.clone(),
                    None => escape_html(&lead(passage.map_or("", |p| p.text.as_str()))),
                },
                line: passage
                    .map(|p| p.line)
                    .or(text.map(|h| h.line))
                    .unwrap_or(1),
                similarity: passage.map(|p| p.similarity),
                rerank: None,
                score,
                past_divider: false,
                primed: false,
                path,
            }
        })
        .collect();

    if let Some(r) = reranker {
        rerank(index, query, r, &mut hits, &best, cfg.rerank_n)?;
    }
    hits.truncate(limit);
```

and add, below `hybrid_hits`:

```rust
/// Rescore the first `n` hits with the cross-encoder and put them in its
/// order; the rest keep fusion order beneath. A reranker that fails leaves the
/// list as it was: the failure is the shell's to report, not the search's.
fn rerank(
    index: &Index,
    query: &str,
    reranker: &mut dyn crate::embed::Reranker,
    hits: &mut [Hit],
    best: &HashMap<String, vector::VecHit>,
    n: usize,
) -> Result<()> {
    let n = n.min(hits.len());
    if n == 0 {
        return Ok(());
    }
    let mut texts = Vec::with_capacity(n);
    for h in &hits[..n] {
        texts.push(match best.get(&h.path) {
            Some(p) => p.text.clone(),
            None => index.passage_text(&h.path)?.unwrap_or_default(),
        });
    }
    let Ok(scores) = reranker.score(query, &texts) else {
        return Ok(());
    };
    if scores.len() != n {
        return Ok(());
    }
    for (h, s) in hits[..n].iter_mut().zip(scores) {
        h.rerank = Some(s);
    }
    // Stable, so equal scores keep fusion order.
    hits[..n].sort_by(|a, b| b.rerank.unwrap().total_cmp(&a.rerank.unwrap()));
    Ok(())
}
```

- [ ] **Step 5: The divider**

In `core/src/search/fuse.rs`, replace `mark_past_divider` and `tail_from`:

```rust
/// Flag every hit from the fall on, leaving the list in its order.
///
/// Reads the rerank score where reranking ran and the cosine otherwise, each
/// against its own floor. The gaps are read over the scores sorted, then
/// carried back as "after the last hit that still reaches the cut", so what
/// is marked is always a tail. A fused list is not in score order -- a hit
/// both branches found sits above one only the dense branch ranked higher --
/// and reading it position by position would cut the good hits below that
/// one away.
pub fn mark_past_divider(hits: &mut [Hit], cfg: &SearchConfig) {
    let reranked = hits.iter().any(|h| h.rerank.is_some());
    let (read, floor): (fn(&Hit) -> Option<f32>, f32) = match reranked {
        true => (|h| h.rerank, cfg.rerank_floor),
        false => (|h| h.similarity, cfg.similarity_floor),
    };
    let mut sorted: Vec<f32> = hits.iter().filter_map(read).collect();
    if sorted.is_empty() {
        return;
    }
    sorted.sort_by(|a, b| b.total_cmp(a));
    // The later of two readings, so whichever leaves more hits standing wins:
    // the cliff finds a fall within the list, the floor knows that a score can
    // be the best one here and still mean nothing.
    let by_cliff = cliff(&sorted, cfg.cliff_factor, cfg.cliff_min_share)
        .map(|above| tail_from(hits, read, sorted[above - 1]));
    let by_floor = tail_from(hits, read, floor);
    let from = by_cliff.unwrap_or(0).max(by_floor);
    for h in hits.iter_mut().skip(from) {
        h.past_divider = true;
    }
}

// The place after the last hit that still reaches `cut`, so what is marked is
// always a tail.
fn tail_from(hits: &[Hit], read: fn(&Hit) -> Option<f32>, cut: f32) -> usize {
    hits.iter()
        .rposition(|h| read(h).is_some_and(|s| s >= cut))
        .map_or(0, |i| i + 1)
}
```

- [ ] **Step 6: Callers**

`core/src/search/candidates.rs:78`:
`let found = super::hybrid_hits(index, query, query_vec, None, cfg, mem, at, limit)?;`

`src-tauri/src/commands.rs:387` (`search`): insert `None,` after
`vector.as_deref(),`. Task 6 replaces it.

- [ ] **Step 7: Run everything in core and the shell's compile**

```bash
cargo test -p engram-notes-core 2>&1 | tail -5
cargo fmt --all && cargo clippy --workspace -- -D warnings 2>&1 | tail -5
```

Expected: all pass, clippy clean. If the `Table` test's fused order puts
Tea first already, the test still proves reordering only through the scores;
change the table to `("kettle", 0.9), ("steep", 0.05)` and the expected
order to `["Coffee.md", "Tea.md"]` so that the rerank order is the reverse of
whatever fusion produced. Check fusion's order first with
`without_a_reranker_hits_carry_no_rerank_score` by printing the paths.

- [ ] **Step 8: Commit**

```bash
git add core/src src-tauri/src/commands.rs
git commit -m "feat(search): rerank the top fused hits, divider on the rerank score

Between fusion and priming, the top rerank_n hits are rescored by a
cross-encoder and reordered; the divider reads that score with its own
floor where it exists. Completion passes no reranker: it answers keystrokes.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: `FastReranker`, `from_dir` with an id, no download

**Files:**
- Modify: `core/src/embed/fastembed.rs`
- Modify: `core/Cargo.toml:32-35`
- Modify: `src-tauri/src/embed.rs:70-73` (compile only; Task 6 rewrites it)

**Interfaces:**
- Produces:
  ```rust
  pub const EMBEDDER_ID: &str = "multilingual-e5-small-int8";
  pub const RERANKER_ID: &str = "mmarco-mMiniLMv2-L12-H384-v1-int8";
  impl FastEmbedder { pub fn from_dir(dir: &Path, id: &str) -> Result<FastEmbedder> }
  pub struct FastReranker;
  impl FastReranker { pub fn from_dir(dir: &Path, id: &str) -> Result<FastReranker> }
  impl Reranker for FastReranker
  pub fn dir_id(dir: &Path) -> String   // "dir:<folder name>", for an override
  ```

- [ ] **Step 1: Drop the network feature**

`core/Cargo.toml`, the `fastembed` dependency: remove the line
`"hf-hub-rustls-tls",` so only `"ort-download-binaries-rustls-tls"` remains
(that one fetches ONNX Runtime at build time, not at run time).

- [ ] **Step 2: Rewrite `core/src/embed/fastembed.rs`**

```rust
//! fastembed over folders of ONNX files: the bundled models, or a folder the
//! writer points at. Nothing here opens a connection.

use super::{Embedder, Reranker, normalise, sigmoid};
use crate::{Error, Result};
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, RerankInitOptionsUserDefined, TextEmbedding,
    TextRerank, TokenizerFiles, UserDefinedEmbeddingModel, UserDefinedRerankingModel,
};
use std::path::Path;

/// `multilingual-e5-small`, dynamically quantised to int8.
pub const EMBEDDER_ID: &str = "multilingual-e5-small-int8";
/// `cross-encoder/mmarco-mMiniLMv2-L12-H384-v1`, dynamically quantised to int8.
pub const RERANKER_ID: &str = "mmarco-mMiniLMv2-L12-H384-v1-int8";
pub const DIM: usize = 384;
/// Both models take 512 tokens.
const MAX_LENGTH: usize = 512;

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

/// The five files every model folder holds.
fn tokenizer_files(dir: &Path) -> Result<TokenizerFiles> {
    Ok(TokenizerFiles {
        tokenizer_file: read(dir, "tokenizer.json")?,
        config_file: read(dir, "config.json")?,
        special_tokens_map_file: read(dir, "special_tokens_map.json")?,
        tokenizer_config_file: read(dir, "tokenizer_config.json")?,
    })
}

/// The id an override folder gets, so the index knows its vectors are not the
/// bundled model's.
pub fn dir_id(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".into());
    format!("dir:{name}")
}

impl FastEmbedder {
    /// A folder holding `model.onnx`, `tokenizer.json`, `config.json`,
    /// `special_tokens_map.json` and `tokenizer_config.json`.
    pub fn from_dir(dir: &Path, id: &str) -> Result<FastEmbedder> {
        let model = UserDefinedEmbeddingModel {
            onnx_file: read(dir, "model.onnx")?,
            external_initializers: Default::default(),
            tokenizer_files: tokenizer_files(dir)?,
            pooling: Some(Pooling::Mean),
            quantization: QuantizationMode::None,
            output_key: None,
        };
        Ok(FastEmbedder {
            model: TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
                .map_err(embed_error)?,
            id: id.to_owned(),
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

pub struct FastReranker {
    model: TextRerank,
    id: String,
}

impl FastReranker {
    /// The same five files as the embedder's folder.
    pub fn from_dir(dir: &Path, id: &str) -> Result<FastReranker> {
        let model = UserDefinedRerankingModel::new(read(dir, "model.onnx")?, tokenizer_files(dir)?);
        let options = RerankInitOptionsUserDefined {
            max_length: MAX_LENGTH,
            ..Default::default()
        };
        Ok(FastReranker {
            model: TextRerank::try_new_from_user_defined(model, options).map_err(embed_error)?,
            id: id.to_owned(),
        })
    }
}

impl Reranker for FastReranker {
    fn id(&self) -> String {
        self.id.clone()
    }

    // fastembed returns the pairs sorted by score; the trait promises document order.
    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let docs: Vec<&str> = documents.iter().map(String::as_str).collect();
        let ranked = self
            .model
            .rerank(query, &docs, false, None)
            .map_err(embed_error)?;
        let mut out = vec![0.0f32; documents.len()];
        for r in ranked {
            let slot = out
                .get_mut(r.index)
                .ok_or_else(|| Error::Embed(format!("rerank index {} out of range", r.index)))?;
            *slot = sigmoid(r.score);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both need the folders `scripts/fetch-models.sh` fills.
    fn models() -> Option<std::path::PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/models");
        dir.join("embedder/model.onnx").exists().then_some(dir)
    }

    #[test]
    #[ignore = "needs src-tauri/models from scripts/fetch-models.sh"]
    fn the_bundled_embedder_embeds_and_ranks() {
        let Some(dir) = models() else { panic!("run scripts/fetch-models.sh") };
        let mut e = FastEmbedder::from_dir(&dir.join("embedder"), EMBEDDER_ID).unwrap();
        assert_eq!(e.dim(), DIM);
        let docs = e
            .embed_documents(&["rust ownership".into(), "kettle and beans".into()])
            .unwrap();
        assert_eq!(docs[0].len(), DIM);
        let q = e.embed_query("who owns the memory in rust").unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&q, &docs[0]) > dot(&q, &docs[1]));
    }

    #[test]
    #[ignore = "needs src-tauri/models from scripts/fetch-models.sh"]
    fn the_bundled_reranker_scores_in_document_order() {
        let Some(dir) = models() else { panic!("run scripts/fetch-models.sh") };
        let mut r = FastReranker::from_dir(&dir.join("reranker"), RERANKER_ID).unwrap();
        let s = r
            .score(
                "who owns the memory in rust",
                &["kettle and beans".into(), "rust ownership rules".into()],
            )
            .unwrap();
        assert_eq!(s.len(), 2);
        assert!(s[1] > s[0], "{s:?}");
        assert!(s.iter().all(|x| (0.0..=1.0).contains(x)));
    }

    #[test]
    fn an_override_folder_is_named_after_itself() {
        assert_eq!(dir_id(Path::new("/x/my-model")), "dir:my-model");
    }
}
```

`rerank` takes `impl AsRef<[S]>` with `S: AsRef<str>`; `&Vec<&str>` satisfies
it. If the compiler objects, pass `docs.as_slice()`.

- [ ] **Step 3: Keep the shell compiling**

`src-tauri/src/embed.rs` lines 70–73 for now:

```rust
        let loaded = match cfg.model_dir.as_deref() {
            Some(dir) => {
                let dir = std::path::Path::new(dir);
                FastEmbedder::from_dir(dir, &engram_core::embed::fastembed::dir_id(dir))
            }
            None => FastEmbedder::from_dir(
                &models_dir.join("embedder"),
                engram_core::embed::fastembed::EMBEDDER_ID,
            ),
        };
```

- [ ] **Step 4: Build and test**

```bash
df -h /home/user01 | tail -1
CARGO_INCREMENTAL=0 cargo test -p engram-notes-core --features fastembed 2>&1 | tail -5
cargo clippy --workspace --all-features -- -D warnings 2>&1 | tail -5
```

Expected: pass (the two real-model tests are ignored), clippy clean.
`cargo tree -p engram-notes-core --features fastembed -i hf-hub` must print
`error: package ID specification `hf-hub` did not match any packages`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add core/Cargo.toml Cargo.lock core/src/embed/fastembed.rs src-tauri/src/embed.rs
git commit -m "feat(embed): FastReranker from a folder, and no download path

The hf-hub feature leaves the tree: both models load from a folder, so
nothing in the binary can open a connection.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: The shell loads both models from resources, warms and budgets the reranker

**Files:**
- Modify: `src-tauri/src/embed.rs`
- Modify: `src-tauri/src/commands.rs` (`search` ~375, `set_model_dir` ~582, the open-vault spawn ~119, test ~899)
- Modify: `src-tauri/src/lib.rs:64`

**Interfaces:**
- Consumes: `TimedReranker`, `FastReranker::from_dir`, `FastEmbedder::from_dir`, `dir_id`, `EMBEDDER_ID`, `RERANKER_ID`, `hybrid(.., Some(&mut dyn Reranker), ..)`.
- Produces:
  ```rust
  pub struct EmbedStatus { model, state, pending, error, pub rerank: &'static str, pub rerank_ms: Option<u64> }
  pub struct Embed { …, pub reranker: Mutex<Option<TimedReranker>> }
  pub fn spawn(app: AppHandle, embed: Arc<Embed>, cfg: EmbedConfig, search: SearchConfig)
  pub(crate) fn publish(app: &AppHandle, embed: &Embed, f: impl FnOnce(&mut EmbedStatus))
  #[tauri::command] pub fn set_reranker_dir(app, state, dir: Option<String>) -> CmdResult<()>
  ```

- [ ] **Step 1: Extend the serialisation test**

In `src-tauri/src/commands.rs`, test `search_results_and_status_serialise_for_the_frontend` (~line 899): the `Hit` literal gains `rerank: Some(0.7),`; after the `pending` assertion add:

```rust
        assert_eq!(status["rerank"], "off");
        assert!(status["rerank_ms"].is_null());
```

and where the hit is serialised, `assert_eq!(hit_json["rerank"], 0.7);` (read the test to find the variable name for the serialised hit).

- [ ] **Step 2: Run to see it fail**

```bash
CARGO_INCREMENTAL=0 cargo test -p engram-notes --lib serialise 2>&1 | tail -5
```

Expected: no field `rerank` on `EmbedStatus`.

- [ ] **Step 3: `embed.rs`**

Replace the status struct, `Embed`, `publish` and `spawn`:

```rust
use engram_core::config::{EmbedConfig, SearchConfig};
use engram_core::embed::fastembed::{EMBEDDER_ID, FastEmbedder, FastReranker, RERANKER_ID, dir_id};
use engram_core::embed::{Embedder, Reranker, TimedReranker};
use std::path::{Path, PathBuf};
```

```rust
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct EmbedStatus {
    pub model: Option<String>,
    /// `off`, `loading`, `ready` or `error`.
    pub state: &'static str,
    pub pending: usize,
    pub error: Option<String>,
    /// `off`, `loading`, `ready`, `slow` or `error`.
    pub rerank: &'static str,
    /// The last measured run, and the one that switched it off when `slow`.
    pub rerank_ms: Option<u64>,
}

impl Default for EmbedStatus {
    fn default() -> Self {
        EmbedStatus {
            model: None,
            state: "off",
            pending: 0,
            error: None,
            rerank: "off",
            rerank_ms: None,
        }
    }
}

#[derive(Default)]
pub struct Embed {
    pub status: Mutex<EmbedStatus>,
    pub embedder: Mutex<Option<Box<dyn Embedder>>>,
    pub reranker: Mutex<Option<TimedReranker>>,
    pub typing: Mutex<Option<Instant>>,
    stop: AtomicBool,
}
```

`publish` becomes `pub(crate) fn publish(...)`.

Model folders: an override wins, else the bundle's resource folder.

```rust
/// The folder a model loads from and the id it gets: an override folder is
/// named after itself, the bundled one after the model.
fn model_folder(override_dir: Option<&str>, resources: &Path, bundled: &str, id: &str) -> (PathBuf, String) {
    match override_dir {
        Some(d) => {
            let d = PathBuf::from(d);
            let id = dir_id(&d);
            (d, id)
        }
        None => (resources.join("models").join(bundled), id.to_owned()),
    }
}

/// About one passage's worth of text, `n` times: what one search costs.
fn warm_up_batch(n: usize) -> Vec<String> {
    let text = "the vault keeps a folder of markdown notes and an index that can be rebuilt "
        .repeat(16);
    vec![text; n.max(1)]
}
```

`spawn`:

```rust
/// Loads both models, then drains the queue for as long as the vault is open.
pub fn spawn(app: AppHandle, embed: Arc<Embed>, cfg: EmbedConfig, search: SearchConfig) {
    std::thread::spawn(move || {
        publish(&app, &embed, |s| {
            s.state = "loading";
            s.error = None;
            s.rerank = "loading";
            s.rerank_ms = None;
        });
        let resources = match app.path().resource_dir() {
            Ok(r) => r,
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(format!("no resource folder: {e}"));
                    s.rerank = "error";
                });
                return;
            }
        };
        let (dir, id) = model_folder(cfg.model_dir.as_deref(), &resources, "embedder", EMBEDDER_ID);
        let model: Box<dyn Embedder> = match FastEmbedder::from_dir(&dir, &id) {
            Ok(m) => Box::new(m),
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(format!("{e} (looked in {})", dir.display()));
                    s.rerank = "off";
                });
                return;
            }
        };
        // ... the existing set_model_id block, unchanged ...
        *embed.embedder.lock().unwrap() = Some(model);
        publish(&app, &embed, |s| {
            s.state = "ready";
            s.model = Some(id);
        });

        load_reranker(&app, &embed, &cfg, &search, &resources);

        // ... the existing drain loop, unchanged ...
    });
}

/// Loads the reranker and scores one search's worth of pairs. Over budget it
/// is not installed: a run cannot be interrupted, so the budget is enforced
/// by measurement and a slow machine searches by fusion order.
fn load_reranker(app: &AppHandle, embed: &Embed, cfg: &EmbedConfig, search: &SearchConfig, resources: &Path) {
    let (dir, id) = model_folder(cfg.reranker_dir.as_deref(), resources, "reranker", RERANKER_ID);
    let loaded = match FastReranker::from_dir(&dir, &id) {
        Ok(r) => r,
        Err(e) => {
            publish(app, embed, |s| {
                s.rerank = "error";
                s.error = Some(format!("{e} (looked in {})", dir.display()));
            });
            return;
        }
    };
    let mut timed = TimedReranker::new(Box::new(loaded));
    let n = search.rerank_n.max(1);
    if let Err(e) = timed.score("a query about the notes", &warm_up_batch(n)) {
        publish(app, embed, |s| {
            s.rerank = "error";
            s.error = Some(e.to_string());
        });
        return;
    }
    let took = timed.last().unwrap_or_default();
    let ms = took.as_millis() as u64;
    if ms > search.rerank_budget_ms {
        publish(app, embed, |s| {
            s.rerank = "slow";
            s.rerank_ms = Some(ms);
        });
        return;
    }
    *embed.reranker.lock().unwrap() = Some(timed);
    publish(app, embed, |s| {
        s.rerank = "ready";
        s.rerank_ms = Some(ms);
    });
}
```

`app.path()` needs `use tauri::Manager;` (already imported).

- [ ] **Step 4: `commands.rs`**

`search` takes the reranker and holds it to the budget. Lock order is
reranker, then `open`; the embed thread never takes the reranker lock while
holding `open`, so there is no cycle.

```rust
#[tauri::command]
pub fn search(
    app: AppHandle,
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> CmdResult<SearchResults> {
    let embed = state.embed.lock().unwrap().clone();
    // The query vector is taken before the index lock, and only if a model is up.
    let vector = {
        let mut guard = embed.embedder.lock().unwrap();
        guard.as_mut().and_then(|m| m.embed_query(&query).ok())
    };
    let mut reranker = embed.reranker.lock().unwrap();
    let (out, budget_ms) = with_open(&state, |o| {
        let hits = engram_core::search::hybrid(
            &o.index,
            &query,
            vector.as_deref(),
            reranker.as_mut().map(|r| r as &mut dyn engram_core::embed::Reranker),
            &o.config.search,
            &o.config.memory,
            now(),
            limit.unwrap_or(50),
        )?;
        Ok((hits, o.config.search.rerank_budget_ms))
    })?;
    // A run over budget switches reranking off for the session; a failure is
    // reported and the list stays in fusion order either way.
    if let Some(r) = reranker.as_ref() {
        let ms = r.last().map(|d| d.as_millis() as u64);
        if let Some(e) = r.last_error() {
            *reranker = None;
            crate::embed::publish(&app, &embed, |s| {
                s.rerank = "error";
                s.error = Some(e);
            });
        } else if ms.is_some_and(|ms| ms > budget_ms) {
            *reranker = None;
            crate::embed::publish(&app, &embed, |s| {
                s.rerank = "slow";
                s.rerank_ms = ms;
            });
        } else if let Some(ms) = ms {
            crate::embed::publish(&app, &embed, |s| s.rerank_ms = Some(ms));
        }
    }
    Ok(out)
}
```

Both `spawn` call sites (~line 119 in the open-vault command and in
`set_model_dir`) become:

```rust
    crate::embed::spawn(app.clone(), embed, cfg.embed.clone(), cfg.search.clone());
```

For `set_model_dir` the closure has to return both: change it to
`Ok(o.config.clone())` and pass `cfg.embed.clone(), cfg.search.clone()`. Its
doc comment becomes `/// A folder with the ONNX file and tokenizer, or `None` for the bundled model.`

Add beneath it:

```rust
/// The same for the reranker.
#[tauri::command]
pub fn set_reranker_dir(app: AppHandle, state: State<AppState>, dir: Option<String>) -> CmdResult<()> {
    let cfg = with_open(&state, |o| {
        o.config.embed.reranker_dir = dir;
        config::save_config(&o.vault, &o.config)?;
        Ok(o.config.clone())
    })?;
    let embed = {
        let mut slot = state.embed.lock().unwrap();
        slot.stop();
        *slot = std::sync::Arc::new(crate::embed::Embed::default());
        slot.clone()
    };
    crate::embed::spawn(app, embed, cfg.embed.clone(), cfg.search.clone());
    Ok(())
}
```

If `set_model_dir` and `set_reranker_dir` are now identical but for one
field, fold the shared tail into `fn restart_embed(app, state, cfg: &AppConfig)`
and call it from both. Register `commands::set_reranker_dir` in
`src-tauri/src/lib.rs` after `set_model_dir`. Remove the now-unused
`config::data_dir()?.join("models")` arguments; if `data_dir` has no other
caller, leave it (the index lives there).

- [ ] **Step 5: Build, test, lint**

```bash
df -h /home/user01 | tail -1
CARGO_INCREMENTAL=0 cargo test -p engram-notes --lib 2>&1 | tail -5
cargo fmt --all && cargo clippy --workspace --all-features -- -D warnings 2>&1 | tail -5
```

Expected: pass, clean.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "feat(shell): load both models from resources, budget the reranker

The embed thread reads the embedder and the reranker from the bundle's
resource folder, or the override folders in app.json. The reranker is
warmed with one search's worth of pairs; over budget, on load or on any
query, it is dropped and the status says so.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Frontend: status copy, reranker folder row

**Files:**
- Modify: `ui/src/lib/api.ts:31,40,83`
- Modify: `ui/src/components/StatusBar.svelte:48-54`
- Modify: `ui/src/components/Settings.svelte:3,36-41,119-120`

**Interfaces:**
- Consumes: `EmbedStatus.rerank`, `EmbedStatus.rerank_ms`, `Hit.rerank`, command `set_reranker_dir`.

- [ ] **Step 1: Types and the command**

`ui/src/lib/api.ts`:

```ts
export interface Hit {
  path: string; title: string; snippet: string; line: number;
  similarity: number | null; rerank: number | null; score: number; past_divider: boolean; primed: boolean;
}
```
```ts
export interface EmbedStatus {
  model: string | null; state: "off" | "loading" | "ready" | "error"; pending: number; error: string | null;
  rerank: "off" | "loading" | "ready" | "slow" | "error"; rerank_ms: number | null;
}
```
```ts
export const setRerankerDir = (dir: string | null) => invoke<void>("set_reranker_dir", { dir });
```

- [ ] **Step 2: Status bar**

`ui/src/components/StatusBar.svelte`, the embed block becomes:

```svelte
  {#if embed.state === "loading"}
    <span>loading model</span>
  {:else if embed.state === "error"}
    <span title={embed.error ?? ""}>embedding off</span>
  {:else if embed.pending > 0}
    <span>{embed.pending} passages pending</span>
  {/if}
  {#if embed.state === "ready" && embed.rerank === "slow"}
    <span title="The cross-encoder took longer than the budget, so search uses fusion order.">
      reranking off: {((embed.rerank_ms ?? 0) / 1000).toFixed(1)} s on this machine
    </span>
  {:else if embed.state === "ready" && embed.rerank === "error"}
    <span title={embed.error ?? ""}>reranking off</span>
  {/if}
```

- [ ] **Step 3: Settings**

Import `setRerankerDir` beside `setModelDir`. Replace `pickModel` with:

```ts
  async function pickModel() {
    const dir = await open({ directory: true });
    if (typeof dir !== "string" || !cfg) return;
    cfg.embed.model_dir = dir;
    await setModelDir(dir);
  }

  async function pickReranker() {
    const dir = await open({ directory: true });
    if (typeof dir !== "string" || !cfg) return;
    cfg.embed.reranker_dir = dir;
    await setRerankerDir(dir);
  }
```

Rows (replacing the existing model row):

```svelte
        {#snippet model()}<button class="pick" onclick={pickModel}>{cfg.embed.model_dir ?? "Bundled"}</button>{/snippet}
        {@render row("Embedding model folder", "A folder with model.onnx and its tokenizer; the default ships with the app.", model)}
        {#snippet reranker()}<button class="pick" onclick={pickReranker}>{cfg.embed.reranker_dir ?? "Bundled"}</button>{/snippet}
        {@render row("Reranker model folder", "A cross-encoder in the same shape; the default ships with the app.", reranker)}
```

- [ ] **Step 4: Check and commit**

```bash
cd ui && pnpm check 2>&1 | tail -5 && pnpm vitest run 2>&1 | tail -5 && cd ..
git add ui/src
git commit -m "feat(ui): reranker status and folder row, models are bundled

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

If `pnpm check` is not a script in `ui/package.json`, run the script that
exists for svelte-check (`grep check ui/package.json`).

---

### Task 8: The models: fetch script and bundle resources

**Files:**
- Create: `scripts/fetch-models.sh`
- Modify: `.gitignore`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: The script**

```sh
#!/bin/sh
# Fills src-tauri/models/ with the two models the app ships. Every file is
# pinned by sha256; a mismatch is a failure, not a warning. Idempotent: a file
# already present and matching is kept.
#   scripts/fetch-models.sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=$root/src-tauri/models
hf=https://huggingface.co

if command -v sha256sum >/dev/null 2>&1; then
  sum() { sha256sum "$1" | cut -c1-64; }
else
  sum() { shasum -a 256 "$1" | cut -c1-64; }
fi

# fetch <dest dir> <repo> <path in repo> <name on disk> <sha256>
fetch() {
  dest=$1/$4
  if [ -f "$dest" ] && [ "$(sum "$dest")" = "$5" ]; then
    return 0
  fi
  mkdir -p "$1"
  printf 'fetching %s/%s\n' "$2" "$3"
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 2 \
    -o "$dest.part" "$hf/$2/resolve/main/$3"
  got=$(sum "$dest.part")
  if [ "$got" != "$5" ]; then
    rm -f "$dest.part"
    printf 'checksum mismatch for %s: %s\n' "$3" "$got" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
}

# multilingual-e5-small, int8 (MIT; the int8 export is Xenova's).
e=Xenova/multilingual-e5-small
d=$out/embedder
fetch "$d" $e onnx/model_int8.onnx      model.onnx              4d24e2bc01a447951524466ef533e52944bf48509e6552810bcee1a2711cb02c
fetch "$d" $e tokenizer.json            tokenizer.json          0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39
fetch "$d" $e config.json               config.json             cb99455288675345e1a4f411438d5d0adbba5fbd3a67ea4fb03c015433b996c1
fetch "$d" $e special_tokens_map.json   special_tokens_map.json d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7
fetch "$d" $e tokenizer_config.json     tokenizer_config.json   a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b

# mmarco-mMiniLMv2-L12-H384-v1, int8 (Apache 2.0). The quint8_avx2 export is
# used on every platform so a universal macOS bundle needs one resource set.
r=cross-encoder/mmarco-mMiniLMv2-L12-H384-v1
d=$out/reranker
fetch "$d" $r onnx/model_quint8_avx2.onnx model.onnx              6c2513767fb63d008a4377bef7a7a3555433d9436342bb53e35a3a72ffc52d4b
fetch "$d" $r tokenizer.json              tokenizer.json          62c24cdc13d4c9952d63718d6c9fa4c287974249e16b7ade6d5a85e7bbb75626
fetch "$d" $r config.json                 config.json             cc2cfe51aa3fd759d21d21acf5dfd6994aa67a3c9210636d22e143699d336c77
fetch "$d" $r special_tokens_map.json     special_tokens_map.json 378eb3bf733eb16e65792d7e3fda5b8a4631387ca04d2015199c4d4f22ae554d
fetch "$d" $r tokenizer_config.json       tokenizer_config.json   e7fbfbfa6347b4e414c1cee50d142e2c2f9a895dad68b068ae83a8b564c3837e

printf 'models are in %s\n' "$out"
```

`chmod +x scripts/fetch-models.sh`. Add `/src-tauri/models` to `.gitignore`
under the `/src-tauri/gen` line.

- [ ] **Step 2: Run it, then run it again**

```bash
scripts/fetch-models.sh && scripts/fetch-models.sh && du -sh src-tauri/models && ls src-tauri/models/*
```

Expected: the first run fetches ten files, the second prints only the last
line; about 272 MB; five files in each folder.

- [ ] **Step 3: Resources**

`src-tauri/tauri.conf.json`, in `bundle`, after `"icon": [...]`:

```json
    "resources": ["models/embedder/*", "models/reranker/*"]
```

- [ ] **Step 4: Real-model tests and the dev copy**

```bash
df -h /home/user01 | tail -1
CARGO_INCREMENTAL=0 cargo test -p engram-notes-core --features fastembed -- --ignored bundled 2>&1 | tail -8
CARGO_INCREMENTAL=0 cargo build -p engram-notes 2>&1 | tail -2
ls target/debug/models/embedder target/debug/models/reranker
```

Expected: both ignored tests pass (the reranker puts the rust document
first), and `tauri-build` has copied both folders beside the debug binary.
If `ls` fails, the glob did not preserve the folder: switch `resources` to
the map form `{"models/embedder/": "models/embedder/", "models/reranker/": "models/reranker/"}`
and rebuild.

- [ ] **Step 5: Commit**

```bash
git add scripts/fetch-models.sh .gitignore src-tauri/tauri.conf.json
git commit -m "feat(build): fetch the two int8 models and ship them as resources

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: CI and the installer carry the models

**Files:**
- Modify: `.github/workflows/build.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `install.sh:62-68`

- [ ] **Step 1: `build.yml`**

In both jobs, after `pnpm install --frozen-lockfile`:

```yaml
      - name: Fetch the models
        shell: bash
        run: scripts/fetch-models.sh
```

Linux *Package* step: the tarball becomes a folder archive in the layout
Tauri resolves for a bare binary (`../lib/engram-notes` beside `bin/`).
Replace the last line of that step (`tar -czf … engram-notes`) with:

```bash
          stage=$RUNNER_TEMP/slim
          mkdir -p "$stage/bin" "$stage/lib/engram-notes"
          cp target/release/engram-notes "$stage/bin/"
          cp -r target/release/models "$stage/lib/engram-notes/"
          tar -czf dist/engram-notes-x86_64-unknown-linux-gnu.tar.gz -C "$stage" bin lib
```

Windows *Package* step:

```pwsh
          Compress-Archive -Path target/release/engram-notes.exe, target/release/models `
            -DestinationPath dist/engram-notes-x86_64-pc-windows-msvc.zip -Force
```

In the *Publish* notes, change the tarball line to:
`The .tar.gz is the slim build for machines that already have webkit2gtk 4.1 and GTK 3: unpack it and keep bin/ and lib/ together.` and the Windows line to
`**Windows**: download the .zip below, unpack it whole and run \`engram-notes.exe\` beside its \`models\` folder. Unsigned, so SmartScreen will warn.`

- [ ] **Step 2: `release.yml`**

After `pnpm install --frozen-lockfile`:

```yaml
      - name: Fetch the models
        shell: bash
        run: scripts/fetch-models.sh
```

- [ ] **Step 3: `install.sh`**

Replace lines 62–68 (the `case "$asset"` extract and the `install`):

```sh
mkdir -p "$bin_dir"
case "$asset" in
  *.tar.gz)
    tar -xzf "$tmp/$asset" -C "$tmp"
    lib_dir=$(dirname "$bin_dir")/lib/engram-notes
    rm -rf "$lib_dir"
    mkdir -p "$lib_dir"
    cp -r "$tmp/lib/engram-notes/." "$lib_dir/"
    install -m 755 "$tmp/bin/engram-notes" "$bin_dir/engram-notes"
    printf 'engram-notes: models installed to %s\n' "$lib_dir" ;;
  *) install -m 755 "$tmp/$asset" "$bin_dir/engram-notes" ;;
esac
printf 'engram-notes: installed to %s/engram-notes\n' "$bin_dir"
```

- [ ] **Step 4: Check the shell scripts and commit**

```bash
sh -n install.sh && sh -n scripts/fetch-models.sh
python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in ['.github/workflows/build.yml','.github/workflows/release.yml']]" 2>/dev/null || echo "no pyyaml; eyeball the indentation"
git add .github/workflows/build.yml .github/workflows/release.yml install.sh
git commit -m "ci: fetch the models before every build, ship them beside the binary

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: The record: memory.md, smoke.md, ROADMAP.md

**Files:**
- Modify: `docs/memory.md:6-12`
- Modify: `docs/smoke.md:129-130` and append
- Modify: `ROADMAP.md:125-134`

- [ ] **Step 1: `docs/memory.md`**

Replace the first paragraph of *Retrieval* with:

```markdown
Full-text (FTS5 BM25, title weighted 10) and semantic (cosine over every
passage vector) each fetch `limit × 3` candidates. Reciprocal rank fusion with
`k = 60` folds them into one list, one entry per note, its best passage as the
snippet. For a deliberate search (Ctrl+K, the search pane) and the passage
picker, a cross-encoder then rescores the top `rerank_n` (20) fused entries
against the query and puts them in its order; the rest keep fusion order
beneath. The divider reads the rerank score, with `rerank_floor` (0.1), where
reranking ran, and the cosine with `similarity_floor` otherwise. `[[`
completion never reranks: it answers keystrokes. With no model loaded the
full-text branch answers alone, which is why search works while the models
load.

Both models ship inside the app as resources: `multilingual-e5-small` and
`cross-encoder/mmarco-mMiniLMv2-L12-H384-v1`, each dynamically quantised to
int8, about 270 MB together. Nothing is downloaded and nothing in the binary
can open a connection. A run of the reranker cannot be interrupted, so the
500 ms budget (`rerank_budget_ms`) is enforced by measurement: one warm-up
batch of `rerank_n` pairs on load, then every query. A run over budget
switches reranking off for the session and the status bar names the time.
```

- [ ] **Step 2: `docs/smoke.md`**

Replace line 75 with:

```markdown
75. Point *Embedding model folder* at a folder holding the fp32 model: the
    status shows `dir:<name>`, every passage re-embeds, and search still
    answers meanwhile by full text. Point it at an empty folder: the status
    shows the error, and `[[` completion still lists notes by spelling.
```

Append:

```markdown
76. In a network namespace with no route out (`unshare -rn`), run the
    AppImage and open a vault: the status bar goes `loading model` to
    `ready` and passages embed; no `models` folder appears under the data
    directory.
77. Unpack the slim tarball and run `bin/engram-notes`: it finds
    `lib/engram-notes/models` beside it and reaches `ready`.
78. Ctrl+K, a query that paraphrases a note: the note comes first. A query
    with nothing in the vault shows the divider with nothing above it.
79. Set `search.rerank_budget_ms` to `1` in `app.json` and search: results
    arrive, and the status bar reads `reranking off: … s on this machine`.
80. Ctrl+Shift+K with a paraphrasing selection: the picker's first step
    lists the paraphrased note first.
81. Two notes with nothing in common still sit under the divider: the 0.83
    similarity floor holds for the int8 embedder, or the number that does is
    recorded in `docs/memory.md`.
```

- [ ] **Step 3: `ROADMAP.md`**

Line 125: `- 🔶 The reranker: ships with 0.6 and the bundled models.`

Section 0.6 heading and text:

```markdown
## 0.6 — Out of the box 🔶

The embedder and a cross-encoder reranker ship **inside the release
artifact**: no download on first run, no network, and a model folder only
for whoever wants a different model. Reranking runs for deliberate search
and the passage picker, within a 500 ms budget, and degrades to fusion order
rather than to waiting. In review; what remains is smoke lines 76–81 in the
real window (`docs/superpowers/specs/2026-09-16-out-of-the-box-design.md`).

- 🔶 `multilingual-e5-small` and `mmarco-mMiniLMv2-L12-H384-v1`, int8, as
  Tauri resources; `scripts/fetch-models.sh` pins them by sha256.
- 🔶 The `Reranker` seam with its fake; the top `rerank_n` fused hits
  rescored, the divider on that score.
- 🔶 Over budget on load or on a query switches reranking off for the
  session; the status bar names the time.
```

- [ ] **Step 4: Commit**

```bash
git add docs/memory.md docs/smoke.md ROADMAP.md
git commit -m "docs: retrieval with the reranker, smoke lines 75-81, roadmap 0.6 in review

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Final verification

- [ ] **Step 1: The whole workspace**

```bash
df -h /home/user01 | tail -1
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings 2>&1 | tail -3
CARGO_INCREMENTAL=0 cargo test --workspace 2>&1 | grep -E '^test result|FAILED|panicked' 
cd ui && pnpm vitest run 2>&1 | tail -3 && cd ..
```

Expected: fmt clean, clippy clean, every `test result` line `ok`, vitest
green.

- [ ] **Step 2: Nothing reaches the network**

```bash
cargo tree -p engram-notes -i hf-hub 2>&1 | head -2
cargo tree -p engram-notes -i ureq 2>&1 | head -2
```

Expected: both report the package is not in the graph. If `ureq` or another
HTTP client remains through a path other than fastembed, name it in the PR
description; only fastembed's download path was in scope.

- [ ] **Step 3: Push and open the PR**

```bash
git push -u origin feat/out-of-the-box
gh pr create --title "feat: out of the box (roadmap 0.6)" --body "$(cat <<'EOF'
Roadmap step 0.6: the embedder and a cross-encoder reranker ship as Tauri
resources inside every artifact; the reranker is the stage between fusion
and the divider for deliberate search and the passage picker, inside a
500 ms budget enforced by measurement. The hf-hub feature is gone, so
nothing in the binary opens a connection.

Spec: docs/superpowers/specs/2026-09-16-out-of-the-box-design.md
Plan: docs/superpowers/plans/2026-09-16-out-of-the-box.md

Not done here: smoke lines 76–81 in the real window.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

The build workflow runs on merge to master, not on the PR; watch the first
master build after merging for the tarball layout and the Windows zip.
