# engram-notes: writing first — the editor, linking, import, and recall while you write

Written 2026-09-14, revised the same day after review. This amends the design
of 2026-09-12, which stands except where this says otherwise. It settles what
engram-notes is for, and therefore what the next eight releases build.

## Why

The first design described a folder of markdown with links, a graph, bases and
semantic recall. It did not say which of those is the spine, and the code grew
accordingly: the Obsidian half is largely present, the engram half sits in a
side panel. Strip the panel and what is left is a smaller Obsidian.

That is not a position anyone can hold. Obsidian has been free for commercial
use since February 2025, needs no account, and carries nine years of polish.
"The same, but open source" persuades only the already persuaded.

The honest competitor is not Obsidian alone but the stack serious users build
on it: Obsidian plus an outliner plugin, plus a local-embedding plugin for
related notes, plus a better search plugin. Against that stack we have three
real arguments, and only three:

1. **Open.** GPL, one binary, nothing phones home, and a page that says so.
2. **One piece instead of five.** The outliner, the search, the recall and the
   linking are one design that shares one index, not four plugins that each
   keep their own.
3. **A memory of use.** Every search, open and link is an event. What the
   writer reaches for ranks higher; what fires together sits closer. No plugin
   has this because Obsidian keeps no record of use.

The people this is for exist and are looking. Logseq's rewrite has been in
beta for years and moves graphs out of markdown files, which is why many chose
it; those users land in Obsidian and miss the outliner. So:

> **Logseq's ergonomics, Obsidian's files, engram's retrieval inside the
> gesture.**

Writing is the centre. Everything else earns its place by serving the act of
writing, or it is a view you can close.

## What this changes

- **The graph is demoted.** A view among views, switchable off entirely.
- **The editor gets its floor.** Bracket wrapping, list continuation, and
  outline gestures over ordinary markdown lists. This is parity with Obsidian
  and its outliner plugin, and it is missing today.
- **A Logseq importer**, early, because it is the door the named population
  walks through.
- **Linking becomes the primary engram surface.** Completion is hybrid; a
  text-level link targets a passage and is picked by meaning.
- **The models ship inside the artifact**, embedder and reranker both. No
  download, no configuration, no network.
- **The Related pane becomes a recall band**: cursor-aware, divider-governed,
  with a cadence the writer sets. One surface, not two.
- **Memory's truth is an event log in the vault.** Activation and association
  are derived from it, like every other index.
- **The vault can say what it knows.** Search verdicts tune the search; a
  base can filter by meaning; topics that recur without a note of their own
  are offered one. Three things that follow from having every passage as a
  vector and every search as an event, and that no plugin stack has.
- **Forgetting is not a feature.** Activation and association are ranking and
  layout signals only. Nothing is hidden, pruned or proposed for deletion; the
  vault is also an instrument of investigation and documentation.
- **No generative AI in the spine.** Ask stays on the roadmap, optional, behind
  a user-supplied endpoint.

## The editor's floor

### What is missing today

`ui/src/components/Editor.svelte` builds its keymap from `defaultKeymap`,
`historyKeymap`, `searchKeymap` and `indentWithTab`. There is no markdown
keymap, so `Enter` does not continue a list; there is no `closeBrackets`, so
typing a bracket over a selection replaces the selection instead of wrapping
it. Both are the first work in this document.

### Wrapping a selection

Select a word, type `[` `[`, and the selection is wrapped, not replaced:
`[[word]]`, selection preserved. The same for `(`, `` ` ``, `*`, `_` and `"`.

### Outlining over ordinary lists

Every outline operation is an ordinary markdown list operation. A file written
by this editor opens in Obsidian, in Vim and in `cat` and reads cleanly. The
outliner is input ergonomics; it does not touch the file format. This is what
Logseq gave up, and why its files travel badly.

- **No forced `- `.** Prose stays prose. Where the user wrote a list, the editor
  behaves like an outliner.
- **Indentation is two spaces**, configurable. CommonMark-correct under `- `.
- **Fold state is never written to the note.** It lives in the derived
  database, per machine, keyed by outline path. When a heavy edit invalidates a
  key the worst case is a wrongly folded item, not a dirty file.
- **No zoom.** Logseq zooms because its pages are huge. This design has files,
  and a subtree that deserves its own view deserves its own file.

Gestures: `Tab` / `Shift+Tab` indent and outdent the item **with its subtree**;
`Alt+Up` / `Alt+Down` move it among its siblings; `Enter` opens a sibling,
`Enter` on an empty item outdents it and at the outermost depth leaves the
list; `Backspace` at the start of an item outdents, then merges; a gutter
chevron folds an item with children. Ordered lists renumber on every operation.
Task items are list items and inherit all of it.

`Tab` is contested by completion, indentation and the outline. The order: an
open completion popup takes it; otherwise, inside a list item, the outline
takes it; otherwise it inserts indentation.

## The Logseq importer

One command: choose a Logseq graph folder, choose a destination folder inside
the vault. The original is never touched. Everything the importer cannot map is
written to `import-report.md` in the destination with file and line, so nothing
is lost silently.

The mapping, in the order the importer applies it:

- `journals/2026_09_14.md` becomes `<daily folder>/2026-09-14.md`.
- Namespaced page files (`a___b.md`) become folder paths (`a/b.md`).
- Page-level `key:: value` lines at the top of a file become YAML frontmatter.
  `title::` renames the file when it differs from the file name.
- `id:: <uuid>` on a block appends a short anchor `^<id>` to that block's first
  line, and a table of uuid to (file, anchor) drives the next two rules.
- `((uuid))` becomes `[[Page#^id]]`; `{{embed ((uuid))}}` becomes
  `![[Page#^id]]`; `{{embed [[page]]}}` becomes `![[page]]`.
- `collapsed::` is dropped. Other block-level properties are kept verbatim as
  text, since Obsidian has no block properties; the report lists them.
- `TODO` and `DONE` become `- [ ]` and `- [x]`. `DOING`, `LATER`, `NOW`, `WAITING`
  become `- [ ]` with the word kept. Priorities, `SCHEDULED:` and `DEADLINE:`
  lines are kept as text.
- `#[[multi word]]` becomes `[[multi word]]`, since a Logseq tag is a page
  reference. Single-word `#tag` stays.
- Tab indentation becomes the configured indent. Bullets are kept: a page that
  is one list stays one list, and the outliner handles it.
- `assets/` is copied beside the notes; `logseq/` is ignored.

The importer is a `core` module with no I/O in its mapping, tested on fixture
graphs, and it is idempotent on its own output.

## Linking

Three gestures, one idea: what you are reaching for is found by meaning as
readily as by spelling.

### Link to a note, disambiguated by meaning

Completion inside `[[…]]` today filters titles and paths with `includes()`
(`ui/src/editor/completions.ts`). It becomes hybrid: substring for what the
writer knows exactly, dense retrieval for what they can only paraphrase. Typing
"carousel" offers the note "VAT fraud chain", marked as a meaning match. Ranking
is fusion plus memory's bounded priming — no rerank, because the popup answers
keystrokes.

### Link to a passage

This is Logseq's `((…))` capability, and the trade behind it should be stated
rather than hidden: **in a folder of files, a stable, Obsidian-compatible link
to a line requires writing an anchor into the target file.** Obsidian appends
`^a1b2c3` to the line; Logseq writes `id:: <uuid>` under the block. This design
does what Obsidian does, and accepts the same cost: one short token at the end
of one line, only on lines that are actually linked to, and only when the link
is made. What this design improves is not the mechanism but the picker.

The writer selects their own words and presses the key. A picker searches
**passages**, not notes, hybrid and reranked. They choose one. On disk:

    [[Note#^a1b2c3|the words I selected]]

Live preview renders the alias; hovering shows the target passage; the
identifier is never the thing on screen. Where the passage begins with a
heading, the link is `[[Note#Heading]]` and nothing is written to the target.

## Passages

Simpler than engram's, deliberately. A vault holds many small documents, and a
note's important part is at its beginning.

- Split on list items, then headings, then paragraphs. A list item **with its
  subtree** is one passage where it fits the budget: an item with its children
  is one thought, and it is a target a writer recognises in the picker. This is
  a parser rule and does not depend on the outliner.
- Pack greedily up to the model's window. No overlap, no sliding windows.
- **Truncate at the end** when a unit does not fit.
- Prepend the heading and outline path so a passage carries its context.
- No PDF, image or HTML ingestion. engram captures; this does not.

## Retrieval

1. Dense retrieval over passage vectors, cosine, top `k × multiplier`.
2. Sparse retrieval, FTS5 BM25, top `k × multiplier`.
3. Reciprocal rank fusion.
4. **Cross-encoder rerank** of the top `rerank_n` (default 20).
5. The divider, on the rerank score where reranking ran, on the fused score
   otherwise.
6. Memory's bounded priming (at most `prime_lift` places).
7. Spread: at most `spread_max` associated notes that did not match.

Step 4 is new; the rest is the first design. Reranking runs for deliberate
search (`Ctrl+K`, the search pane) and the passage picker. It does not run for
the recall band or for `[[…]]` completion, which must keep a typing rhythm.

**Budget:** 500 ms end to end for a deliberate search. A cross-encoder over
twenty candidates on a modest CPU is the dominant cost and may exceed it;
`rerank_n` is a setting and measurement sets its default. If it cannot fit, it
degrades to fusion order with a note in the status bar, never to a wait.

**Backend:** `fastembed`'s reranking support behind a `Reranker` trait beside
`Embedder`, with a deterministic fake for tests. That fastembed-rs exposes a
multilingual reranker is an assumption to verify before this work starts.

## Recall while writing

The Related pane of the first design becomes the recall band. Same place, one
surface; the old pane's three groups fold into it.

The band shows what the paragraph under the cursor pulls toward: up to five
passages with their note names. Clicking opens; one key links at the cursor.
Below the passages, the first design's *associated* group: notes that fired
together with this one, with the query that bound them.

**The divider decides the quantity.** Where relevance falls away, nothing is
shown. The band is often empty, and that is the point: a band that always has
something to say is noise.

**The cadence belongs to the writer.** Four positions: *live* (300 ms after
typing stops), *on paragraph end*, *on keypress only*, *off*. The default is
*on paragraph end*.

## Memory

A change from the first design.

The first design stores `activation`, `assoc` and `events` in the derived
index, which lives in the OS data directory and must not be synced. But those
tables are not derived — they are the one thing the application makes that the
folder cannot rebuild — and under that rule a user who syncs a vault between
two machines loses them. Moving a database into the vault does not help: a
SQLite file written on every note open, synced by Syncthing or Dropbox, is a
conflict waiting to happen.

The resolution is to notice what the truth actually is. Activation is a sum of
decayed bumps over open events. Association is built from pairs of events in
one sitting or one search. Both are **derived from the event log**, so the
event log is the truth of memory and everything else is a cache.

- **`<vault>/.engram-notes/events/<install-id>.jsonl`** — append-only, one JSON
  object per line: `{at, kind, path, query?}`. One file per installation, so
  two machines never write the same file and any sync tool merges the folder
  without conflict. The install id is generated once per machine and kept in
  the OS data directory, not the vault.
- A rename appends `{at, kind: "rename", from, to}`; replay follows it. The
  log stays append-only and old events stay meaningful.
- Sittings are not stored. They are derived at replay from the thirty-minute
  gap.
- **`activation` and `assoc` are tables in the derived index**, rebuilt from the
  logs. The index remembers a byte offset per log file, so a normal open
  replays only the tail; a schema bump replays everything. Replaying a year of
  use is tens of thousands of lines and takes well under a second.
- *Forget* deletes the log files. Since they are the truth, that is the whole
  operation.

Markdown files remain the truth for what a note contains; the event logs are
the truth for how the vault was used; everything else is rebuildable. This is
the first design's rule, applied to memory too.

## What the vault can tell you

Three features that exist because every passage is a vector and every search
is an event. Each is a view or a setting the writer opens; none interrupts
writing.

### Search that tunes itself

engram's core idea, translated. A test query written while looking at the
answer passes on every system; the only honest measure is the searches made in
earnest.

- **A verdict is an event.** Opening a result from a search is a positive
  verdict at that rank, appended to the event log as
  `{at, kind: "verdict", query, path, rank}`. Not opening anything is no
  verdict, not a negative one. A small explicit *yes / no* under the search
  results adds the verdicts clicks alone are too thin to give.
- **Two numbers**, recall@10 and MRR over the last 200 verdicts or 90 days,
  whichever is smaller, shown in settings under *Search quality*.
- **Tuning is proposed, never applied silently.** An idle-time run replays the
  verdict queries against a small grid of settings — the RRF constant, the
  dense/sparse weight, the divider fraction, `rerank_n`, `prime_lift` — and
  when one improves MRR by a margin over enough verdicts, it appears in
  settings as a proposal with one button. A search that quietly changes its
  behaviour is unsettling; a search that says "this would have been better"
  is engram.
- Vectors drift as the model or the notes change, so old verdicts go stale.
  The window is the answer, and a model change clears them.

### Bases that filter by meaning

Two functions added to the Bases expression language, documented as ours in
`docs/bases.md`, not as Obsidian's:

- `similarity("text")` — a number: the note's similarity to the text, taken as
  the note's **best passage**, not its mean. A mean lets a long thin note beat
  a short exact one.
- `similar("text")` — a boolean: true above the divider for that text. An
  optional second argument sets a fixed threshold instead.

Everything else is Bases as it stands: `similar("shell companies") and
file.hasTag("case-42")`, sorted by `similarity(...)`, with a column showing
the score. Meaning and structure in one filter is the point; it takes two
tools anywhere else.

The query embedding is computed once per open and again when the `.base`
changes; note scores recompute when vectors change, through the watcher. Dense
only: no BM25 and no rerank, since a view may match hundreds of notes and
reranking them would take seconds. A `.base` using these functions is valid
YAML in Obsidian, which reports an unknown expression for that view.

### Topics without a home

Entity-agnostic by construction: it looks for recurrence in meaning, not for
names, so a person, a company, a procedure and a concept are found the same
way.

1. **Cluster passages** agglomeratively over cosine similarity with a fixed
   density threshold — no target count. Runs when the index is idle, never
   while typing.
2. **Check coverage.** For each cluster drawing on at least three notes: is
   there a note whose title-and-lead embedding lies near the centroid? If so
   the topic has a home. If not, it is a finding.
3. **Name it without patterns.** The terms over-represented in the cluster
   against the rest of the vault (TF-IDF, cluster versus vault) that occur in
   at least half its passages. A proposed title, edited by a person.
4. **One click** creates the note with that title, pre-filled with links to
   every participating passage — `[[Note#^id]]` or `[[Note#Heading]]`. A hub
   page made of meaning, not an empty placeholder.

Cluster quality depends on the model and term labels are sometimes nonsense.
The guards are the density threshold, the three-note minimum, and that
nothing is created without a click. It is an offer in a view, never a prompt
that appears.

## The graph

Kept, demoted. A view that opens beside a note or fills a tab, with the
semantic edges the first design describes. Switchable off in settings. No
global graph by default: at ten thousand notes a global force layout is a
hairball in every tool that has one.

## Non-goals

Everything the first design excludes, and additionally: no block-reference
syntax of our own; no forced outline file format; no zoom; no capture
pipeline; no generative AI in any path the writer cannot avoid; no feature
that removes, hides or proposes removing a note.

## Order of work

Each step ships on its own.

**0.2 — The editor's floor.** `closeBrackets` with selection wrapping; the
markdown keymap and list continuation; the outline gestures; folding; `Tab`
precedence.

**0.3 — The Logseq importer.** The mapping above, the report, fixture graphs.

**0.4 — Linking.** Hybrid `[[…]]` completion; the passage picker; anchors;
alias rendering in live preview and reading mode.

**0.5 — Out of the box.** Embedder and reranker in the release artifact; the
rerank stage; the `rerank_n` measurement; the network-behaviour page.

**0.6 — The recall band and the event log.** The band replaces the Related
pane; the cadence slider; the event log replaces the in-index events table,
with a one-time migration of existing rows into a log.

**0.7 — Verdicts.** The verdict event, the explicit control, *Search quality*
in settings, the idle-time grid and its proposal.

**0.8 — Bases that filter by meaning.** `similarity()` and `similar()`, the
score column, `docs/bases.md`.

**0.9 — Topics without a home.** Clustering, coverage, naming, the view and
its one click. Last because its thresholds need iteration on real vaults.

**Later.** Provenance and capture into the vault; Ask, citing and abstaining;
the graph's remaining polish; card views for bases.

Alongside, not as a phase: reproducible builds, signed releases, Flatpak and
AUR packaging.

## Testing

In `core`, without a model or a window:

- outline: indent and outdent carry the subtree; move preserves order; ordered
  lists renumber; the produced markdown parses to the same structure.
- import: each mapping rule on a fixture; the report names every unmapped
  construct; importing the importer's output changes nothing.
- linking: anchors are stable and unique per target line; the alias form
  parses; writing an anchor changes nothing else in the target file.
- passages: list items pack with their subtrees; truncation is at the end;
  heading and outline paths are prepended.
- retrieval: rerank changes order and the divider follows it; priming stays
  bounded after reranking; the fake reranker is deterministic.
- memory: replaying a log reproduces activation and association exactly; a
  rename event redirects earlier events; tail replay equals full replay; two
  logs from two installs merge to one memory.
- verdicts: recall@10 and MRR from a fixed verdict set; the grid proposes only
  above the margin; a model change clears the window.
- bases: `similarity()` takes the best passage; `similar()` follows the
  divider and honours a fixed threshold; both compose with `and` and `or`.
- topics: clustering is deterministic under the fake embedder; a covered
  cluster is not offered; a cluster under three notes is not offered; the
  label draws only from terms in at least half the passages.

In the frontend, with vitest: wrap-on-selection, `Tab` precedence, the cadence
slider's four states including *off*, and that the band renders nothing below
the divider.

## Risks, stated plainly

**Fold state drifts.** Keying folds by outline path will occasionally be wrong
after a large edit. Accepted: the failure is cosmetic and self-correcting.

**Reranking may not fit the budget.** The one cost in this document that is
not yet known. The design degrades to fusion order rather than to waiting.

**The band may be noise.** If the divider is tuned loosely, the band becomes
another suggestion box and gets switched off. Tight is the design.

**Import is lossy at the edges.** Block properties, priorities and scheduling
have no Obsidian equivalent. The report is the answer: nothing is dropped
without being named.

**`Tab` is overloaded.** The precedence rule is the whole answer, and it needs
to hold in every mode.

**Topic labels may be nonsense.** Term statistics name a cluster well when it
is tight and badly when it is not. The density threshold does most of the
work, and the person does the rest; a bad label costs one edit.

**Verdicts are sparse.** A single user produces few searches a day, and most
searches end without a click. The proposal margin and the minimum count exist
so that a handful of verdicts never moves a setting.
