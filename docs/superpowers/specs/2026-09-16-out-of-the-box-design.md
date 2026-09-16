# Out of the box (roadmap 0.6) — design

Step 0.6 of `ROADMAP.md`. The direction spec
(`2026-09-14-writing-first-direction-design.md`, sections *Retrieval* and
*What changes*) says what and why; this document fixes the shape, records
the decisions it left open, and is what the implementation plan builds from.

## In one paragraph

The release artifact carries its own models. A fresh install opens a vault,
embeds it and answers a search without a download, a model folder or a
network connection, because the embedder and a cross-encoder reranker are
resources inside the bundle. The reranker becomes the stage between fusion
and the divider for deliberate search and the passage picker, inside a 500 ms
budget, and where a machine cannot keep that budget the search falls back to
fusion order and says so in the status bar, never to a wait.

## Decisions taken here

Each of these was open in the direction spec or is a departure from today's
code. They are settled; the plan does not re-decide them.

- **Tauri resources, not `include_bytes!`.** The model files travel in
  `bundle.resources` and are read from `resource_dir()`. One copy serves a
  universal macOS binary, rustc never holds 270 MB of static data, and
  `tauri-build` copies the same files into `target/` so `cargo tauri dev`
  loads them the same way. The slim Linux tarball and the Windows executable
  become a folder archive: the binary beside `models/`. That is the whole
  cost, and it is the ordinary Tauri shape.
- **int8 for both models.** `multilingual-e5-small` at int8 is 118 MB
  against 470 MB, the reranker 119 MB. The embedder's id changes, so every
  vault re-embeds once; the similarity floor was measured on fp32 and smoke
  rechecks it.
- **The reranker is `cross-encoder/mmarco-mMiniLMv2-L12-H384-v1`.** Apache
  2.0, multilingual, twelve layers of width 384: the only candidate that is
  both shippable and plausibly inside the budget on a modest CPU. fastembed's
  own multilingual rerankers are not: `jina-reranker-v2-base-multilingual` is
  CC-BY-NC 4.0, `bge-reranker-v2-m3` is 2.2 GB. The `quint8_avx2` export is
  used on every platform, so a universal macOS bundle needs one resource set;
  ONNX Runtime runs dynamically quantised models on any CPU, and measurement
  can revisit the choice on arm64.
- **`model_dir` stays as an override, the download goes.** A folder in
  `app.json` still replaces the bundled embedder, and a second one the
  reranker, for anyone who wants a different model. `FastEmbedder::download`
  and the `hf-hub` feature are removed: nothing in the binary can open a
  connection, and the status bar's `loading` state now means reading from
  disk.
- **Over budget means off, not slow.** A cross-encoder run cannot be
  interrupted, so the budget is enforced by measurement: once on load with a
  synthetic batch, then on every query. A run over budget switches
  reranking off for the session and the status bar names the time. No query
  ever waits on more than one over-budget run.
- **No network page.** Decided on the roadmap; this step does not write one.

## The Reranker seam

`core::embed` gains a trait beside `Embedder`:

```rust
pub trait Reranker: Send {
    fn id(&self) -> String;
    /// One score per document, higher is more relevant, in `documents` order.
    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>>;
}
```

- `FakeReranker`: deterministic, no model. Its score is the share of the
  query's words that occur in the document, so tests can arrange an order
  the fake will produce. Tests run without a model, as `AGENTS.md` requires.
- `FastReranker` behind the `fastembed` feature, built from a folder holding
  `model.onnx`, `tokenizer.json`, `config.json`, `special_tokens_map.json`
  and `tokenizer_config.json` through `TextRerank::try_new_from_user_defined`.
  Scores are the model's logits passed through a sigmoid, so they lie in
  `(0, 1)` and the divider's cliff and floor read them as they read a cosine.
- `TimedReranker<R>` wraps any reranker and records the wall time of its last
  `score`. The shell reads it after each query; core stays free of policy.

`FastEmbedder::from_dir` is kept and gains a caller-supplied id, so the
bundled folder loads as `multilingual-e5-small-int8` and an override folder
as `dir:<name>` as today. `FastEmbedder::download` is removed with the
`hf-hub-rustls-tls` feature; `ort-download-binaries-rustls-tls` stays, since
it fetches ONNX Runtime at build time, not at run time.

## Retrieval

`search::hybrid` and `search::hybrid_hits` take `Option<&mut dyn Reranker>`.
Deliberate search and the picker's first step pass the reranker; `[[`
completion passes `None` through `candidates::link_candidates`, since it
answers keystrokes.

Inside `hybrid_hits`, after fusion and before priming:

1. The top `rerank_n` fused entries (default 20) are gathered with their
   passage text: from the dense hit where the dense branch found the note,
   otherwise from a new `Index::passage_text(path)`. A note is one passage
   since 0.5, so this is the text the embedder saw.
2. The reranker scores the pairs. Those entries are reordered by score,
   descending, ties broken by fused order; everything past `rerank_n` keeps
   fusion order beneath them. Then `take(limit)` as today.
3. Each reranked `Hit` carries `rerank: Some(score)`; `similarity` stays the
   cosine, since the Related pane and the results show it, and `score` stays
   the fused score.

A reranker error is not a search error: the list stays in fusion order and
the error reaches the shell's status, which is how the embedder's failures
are treated today.

**The divider.** `mark_past_divider` reads `rerank` where any hit has one
and `similarity` otherwise. The cliff is the same rule over the same
descending sort. The floor is `search.rerank_floor` on the rerank path and
`similarity_floor` on the cosine path. Both floors are model-specific
numbers, and `rerank_floor`'s default is a starting point that measurement
sets, the way the direction spec says of `rerank_n`.

**Config.** `SearchConfig` gains `rerank_n: usize` (20), `rerank_floor:
f32` and `rerank_budget_ms: u64` (500). `EmbedConfig` gains `reranker_dir:
Option<String>` beside `model_dir`.

## The shell

The embed thread loads both models before it drains the queue. It resolves
the embedder folder as `embed.model_dir` if set, else
`<resource_dir>/models/embedder`, and the reranker folder as
`embed.reranker_dir` if set, else `<resource_dir>/models/reranker`. A
missing bundled folder is an error in the status, with the path it looked
for, so a broken package says what is wrong rather than silently searching
by full text alone.

After the reranker loads, one synthetic batch of `rerank_n` passages of
about `MAX_CHARS` characters is scored, which also warms ONNX Runtime. Over
budget: the reranker is dropped and the status reads `slow` with the
measured time. Under: it is installed behind a mutex like the embedder, and
the `search` command passes it. After each search the command reads the
timed wrapper; a run over budget drops the reranker and publishes `slow`.

`EmbedStatus` gains `rerank: "off" | "loading" | "ready" | "slow" |
"error"` and `rerank_ms: Option<u64>`. `set_model_dir` gains a sibling
`set_reranker_dir`; both restart the embed thread as today.

## The frontend

- Status bar: while the embedder is `ready` and the reranker is `slow`, the
  bar reads `reranking off: 1.2 s on this machine`, with the measured time.
  Every other reranker state is silent; the embedder's states read as today.
- Settings: the *Embedding model folder* row's default label becomes
  *Bundled*, and a *Reranker model folder* row appears under it with the
  same control.
- Nothing in the search results changes shape; a reranked list is a list.

## Packaging

- `scripts/fetch-models.sh` downloads the five files of each model from
  Hugging Face into `src-tauri/models/embedder/` and
  `src-tauri/models/reranker/`, renaming the chosen ONNX export to
  `model.onnx`, and verifies every file against a sha256 pinned in the
  script. The folder is gitignored. The script is the one source of which
  file is which; a developer runs it once, CI runs it before every build.
- `tauri.conf.json` lists `models/embedder/*` and `models/reranker/*` under
  `bundle.resources`.
- `build.yml` and `release.yml` run the script after checkout. The Linux
  tarball packs `engram-notes` and `models/` from `target/release`; the
  Windows job zips the executable beside `models/`.
- `docs/memory.md`'s retrieval section gains the rerank step and the
  budget. `ROADMAP.md` marks 0.5's reranker line and 0.6 as in review, and
  0.6's text loses the network page.

## What this step does not do

- No reranking for `[[` completion or the recall band; the direction spec
  keeps those on a typing rhythm.
- No setting to switch reranking off by hand. The budget is the switch.
- No per-architecture model files, and no macOS or arm64 measurement; the
  smoke run is Linux x86_64, and the release workflow's other platforms
  build the same resources.
- No network page.

## Smoke

Added to `docs/smoke.md`; every line is a yes or the tag waits.

- On a machine or a network namespace with no route out, install the
  AppImage and open a vault: the status bar goes `loading` to `ready` and
  passages embed; no `models` folder appears under the data directory.
- The same with the tarball unpacked to a folder: the binary finds `models/`
  beside it.
- Ctrl+K, a query with a paraphrased answer in the vault: the note comes
  first, and a query with nothing in the vault shows the divider with
  nothing above it.
- Set `search.rerank_budget_ms` to `1` in `app.json` and search: results
  still arrive, and the status bar reads `reranking off` with a time.
- Ctrl+Shift+K, the picker's first step lists the paraphrased note first.
- Point *Embedding model folder* at a folder holding the fp32 model: the
  status shows `dir:<name>`, every passage re-embeds, and search still
  answers meanwhile by full text. Point it at an empty folder: the status
  shows the error, and `[[` completion still lists notes by spelling.
