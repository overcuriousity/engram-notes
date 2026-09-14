# engram-notes: writing first — the outliner, linking, and recall while you write

Written 2026-09-14. This amends the design of 2026-09-12, which stands except
where this says otherwise. It settles what engram-notes is for, and therefore
what the next four releases build.

## Why

The first design described a folder of markdown with links, a graph, bases and
semantic recall. It did not say which of those is the spine, and the code grew
accordingly: the Obsidian half is largely present, the engram half sits in a
side panel. Strip the panel and what is left is a smaller Obsidian.

That is not a position anyone can hold. Obsidian has been free for commercial
use since February 2025, needs no account, and carries nine years of polish and
a plugin ecosystem. Competing on its own terms — the faithful, fast filing
cabinet — is a losing game, and "the same, but open source" only persuades
people who were already persuaded.

The open field is elsewhere, and it is real. Logseq's rewrite has been in beta
for years and is moving graphs from markdown files into a database, which is
the reason many of its users chose it; a population is moving. Most of them land
in Obsidian and miss the outliner. Meanwhile the open-source alternatives each
give up something essential — SiYuan and Trilium leave plain files behind,
Anytype and AppFlowy are object stores, Joplin links poorly — and the local-AI
note apps (Reor, Khoj) prove the demand for semantic recall without being
daily-driver editors.

So:

> **Logseq's ergonomics, Obsidian's files, engram's retrieval inside the
> gesture.**

Writing is the centre. Everything else earns its place by serving the act of
writing, or it is a view you can close.

## What this changes

- **The graph is demoted.** It is a view among views and can be turned off
  entirely. It is not the second half of the editor.
- **The outliner is added**, as editor behaviour over ordinary markdown lists,
  never as a file format.
- **Linking becomes the primary engram surface**, not a sidebar. Completion is
  hybrid; a text-level link targets a passage and is picked by meaning.
- **The models ship inside the artifact.** No download on first run, no model
  directory to configure, no network. A reranker ships with them.
- **Recall while writing** becomes a quiet band beside the text, governed by
  engram's divider and by a cadence the user sets.
- **Forgetting is not a feature.** Activation and association remain, purely as
  ranking and layout signals. Nothing is ever hidden, pruned or proposed for
  deletion. The vault is also an instrument of investigation and documentation;
  a tool that tidies things away is disqualified for that use.
- **No generative AI in the spine.** Ask stays on the roadmap, optional, behind
  a user-supplied endpoint, and nothing depends on it.

## The writing surface

### Outlining on flat markdown

Every outline operation is an ordinary markdown list operation. A file written
by this editor opens in Obsidian, in Vim and in `cat` and reads cleanly. The
outliner is input ergonomics; it does not touch the file format. This is
precisely what Logseq gave up, and why its files travel badly.

- **No forced `- `.** Prose stays prose. Where the user wrote a list, the editor
  behaves like an outliner.
- **Indentation is two spaces**, configurable. CommonMark-correct under `- `,
  and it keeps files narrow.
- **Fold state is never written to the note.** Logseq's `collapsed:: true` in
  the text is the second thing to avoid. Fold state lives in the database, keyed
  by outline path. When a heavy edit invalidates a key the worst case is a
  wrongly folded item, not a dirty file.
- **Zoom is view state.** Zooming into an item shows it and its subtree with a
  breadcrumb; Escape returns. The file does not change.

### Gestures

- `Tab` / `Shift+Tab` indent and outdent the item **together with its subtree**.
- `Alt+Up` / `Alt+Down` move the item and its subtree among its siblings.
- `Enter` opens a sibling at the same depth; `Enter` on an empty item outdents
  it; at the outermost depth it leaves the list.
- `Backspace` at the start of an item outdents, then merges.
- A chevron in the gutter folds an item that has children.
- Ordered lists renumber on indent, outdent and move. Task items (`- [ ]`) are
  list items and inherit all of the above.

`Tab` is contested by autocompletion, by indentation and by the outline. The
order is: an open completion popup takes it; otherwise, inside a list item, the
outline takes it; otherwise it inserts indentation.

### What is missing today, beyond the outliner

`ui/src/components/Editor.svelte` builds its keymap from `defaultKeymap`,
`historyKeymap`, `searchKeymap` and `indentWithTab`. There is no markdown
keymap, so `Enter` does not continue a list at all, and there is no
`closeBrackets`, so typing a bracket over a selection replaces the selection
instead of wrapping it. The outliner is not a layer on top of a working base; it
is the base that is not there yet.

## Linking

Three gestures, one idea: the thing you are trying to reach is found by meaning
as readily as by spelling.

### Wrap a selection

Select a word, type `[` `[`, and the selection is wrapped, not replaced:
`[[word]]`, selection preserved. The same holds for `(`, `` ` ``, `*`, `_` and
`"`. This is `closeBrackets()` with bracket-wrapping enabled, and it is the
smallest piece of work in this document.

### Link to a note, disambiguated by meaning

Completion inside `[[…]]` today filters titles and paths with `includes()`
(`ui/src/editor/completions.ts`). It becomes hybrid: substring for what the
writer knows exactly, dense retrieval for what they can only paraphrase. Typing
"carousel" offers the note "VAT fraud chain", flagged as a meaning match rather
than a spelling match. A suggestion that is not letter-identical is the thing no
other editor can offer.

Ranking inside the popup is fusion plus memory's bounded priming — no rerank,
because the popup answers keystrokes — so notes the writer reaches for surface
first.

### Link to a passage

This is Logseq's `((…))` capability without Logseq's opacity.

The writer selects their own words and presses the key. A picker searches
**passages**, not notes, hybrid again. They choose the passage. On disk the
result is Obsidian's own form:

    [[Note#^a1b2c3|the words I selected]]

and the anchor `^a1b2c3` is appended to the target line in the target file,
exactly as Obsidian does it. The editor never shows an identifier: live preview
renders the alias, and hovering shows the target passage.

The writer gets Logseq's power, Obsidian's file format in both directions, and
a picker better than either — because the index already holds every passage as
an embedded unit. The unit of retrieval and the unit of reference become the
same thing without any of it being visible.

## Recall while writing

A narrow band beside the text shows what the paragraph under the cursor pulls
toward: three to five passages, each with its note name. Clicking opens it; one
key turns it into a link at the cursor.

Two rules decide whether this is kept or switched off:

**The divider decides the quantity.** engram's cutoff applies: where relevance
falls away, nothing is shown. The band is often empty, and that is the point. A
band that always has something to say is noise; a band that usually says nothing
is a signal.

**The cadence belongs to the writer.** A slider with four positions: *live*
(300 ms after typing stops), *on paragraph end*, *on keypress only*, and *off*.
The default is *on paragraph end* — quiet is worth more than reaction speed
while writing.

The band uses fusion only; it does not rerank. Reranking is for deliberate
search, where a second of latency is acceptable and a paragraph of typing is
not.

## Retrieval

The pipeline, in order:

1. Dense retrieval over passage vectors, cosine, top `k × multiplier`.
2. Sparse retrieval, FTS5 BM25, top `k × multiplier`.
3. Reciprocal rank fusion into one list.
4. **Cross-encoder rerank** of the top `rerank_n` (default 20).
5. The divider, drawn on the rerank score where reranking ran, on the fused
   score otherwise.
6. Memory's bounded priming (at most `prime_lift` places).
7. Spread: at most `spread_max` associated notes that did not match.

Steps 1–3 and 5–7 are as the first design describes them. Step 4 is new.

**Where reranking runs:** deliberate search (the search pane, `Ctrl+K`) and the
passage picker. **Where it does not:** the recall band, and completion inside
`[[…]]`, both of which must stay under a typing rhythm.

**Budget:** 500 ms end to end for a deliberate search. A cross-encoder over 20
candidates on a modest CPU is the dominant cost and may exceed that; `rerank_n`
is therefore a setting, and the measurement decides its default. If reranking
cannot be made to fit, it degrades to fusion order with a note in the status
bar — never to a hang.

**Backend:** `fastembed`'s reranking support, behind a `Reranker` trait
alongside `Embedder`, with a deterministic fake for tests. That fastembed-rs
exposes the reranker models we want is an assumption to verify before this work
starts.

## Passages

Simpler than engram's, deliberately. A PKM vault holds many small documents, not
few large ones, and a note's important part is at its beginning.

- Split on outline items, then headings, then paragraphs. An outline item
  **together with its subtree** is one passage where it fits the budget, because
  an item with its children is one thought. This also gives the passage picker
  targets that a writer recognises.
- Pack greedily up to the model's window. No overlap, no sliding windows, no
  second pass.
- **Truncate at the end** when a unit does not fit. Losing the tail of a long
  passage is acceptable; the cost of being cleverer is not.
- Prepend the heading and outline path so a passage carries its context.
- No PDF, image or HTML ingestion. Attachments are listed and previewed, never
  indexed. engram does capture; this does not.

## What ships in the binary

The application must work on first launch with no network and no configuration.
This is a release-pipeline requirement, not a packaging preference.

- The embedding model (`multilingual-e5-small` or better) and the reranker ship
  **inside the release artifact**. No download, no model directory, no setup.
- The AppImage grows accordingly. That is the accepted trade: size is cheap,
  a first launch that cannot search is not.
- `ENGRAM_NOTES_SLIM=1` remains the path for someone bringing their own model.
- Nothing reaches the network in normal operation. A page in the repository
  states exactly what does and when, and it says "nothing" for the default
  build.

## State, files and sync

A change from the first design, and the one point worth arguing about.

The first design puts everything derived in
`data_dir/engram-notes/vaults/<hash>/index.db` and says it must not be synced.
But `activation`, `assoc` and `events` are not derived: they cannot be rebuilt
from the folder, they are the only irreplaceable thing the application makes,
and under that rule a user who syncs a vault between two machines loses them.

So the state splits by replaceability, not by size:

- `data_dir/engram-notes/vaults/<hash>/index.db` — **derived and rebuildable**:
  notes, links, tags, properties, FTS, passages, vectors. Large. Never synced. A
  schema bump deletes and rebuilds it.
- `<vault>/.engram-notes/memory.db` — **learned and irreplaceable**: activation,
  association, events, and outline view state (folds, zoom). Small. Travels with
  the vault exactly as `.obsidian/` does. Migrated, never rebuilt.

Markdown files remain the truth for everything a note contains. The databases
hold what a file cannot: what was used, what fired together, and what is folded.
No note's content, structure or links live only in a database.

## The graph

Kept, demoted. A view that opens beside a note or fills a tab, with the semantic
edges the first design describes. It is switchable off in settings, and nothing
else depends on it. There is no global graph by default: at ten thousand notes a
global force layout is a hairball in every tool that has one, and it opens
deliberately, with filters.

## Non-goals

Everything the first design excludes, and additionally: no block-reference
syntax of our own; no forced outline file format; no capture pipeline; no
generative AI in any path the writer cannot avoid; no feature that removes,
hides or proposes removing a note.

## Order of work

Each step is useful on its own and ships on its own.

**0.2 — The writing surface.** `closeBrackets` with selection wrapping; the
markdown keymap and list continuation; the outline gestures; folding; zoom; the
`Tab` precedence rule.

**0.3 — Linking.** Hybrid completion inside `[[…]]`; the passage picker; anchor
writing into target files; alias rendering in live preview and reading mode.

**0.4 — Out of the box.** Embedder and reranker in the release artifact; the
rerank stage and its trait; measurement of `rerank_n` against the 500 ms budget;
the network-behaviour page.

**0.5 — The recall band.** The band, the cadence slider, divider-governed
quantity, link-from-band.

**0.6 and later.** Provenance and capture into the vault; Ask, citing and
abstaining; the graph's remaining polish; card views for bases.

Running alongside, not as a phase: reproducible builds, signed releases,
Flatpak and AUR packaging instead of curl-to-sh.

## Testing

In `core`, without a model or a window:

- outline: indent and outdent carry the subtree; move preserves order; ordered
  lists renumber; the produced markdown round-trips through the parser
  unchanged in meaning.
- linking: anchor generation is stable and unique per target line; alias form
  parses; an anchor written into a target file changes nothing else about it.
- passages: outline items pack with their subtrees; truncation happens at the
  end; heading and outline paths are prepended.
- retrieval: rerank changes order and the divider follows the rerank score;
  priming stays bounded after reranking; the fake reranker is deterministic.
- state: `memory.db` survives a schema bump of `index.db`; fold keys degrade
  without corrupting anything.

In the frontend, with vitest: the wrap-selection behaviour, `Tab` precedence,
the cadence slider's four states including *off*, and that the band renders
nothing below the divider.

## Risks, stated plainly

**Fold state drifts.** Keying folds by outline path is fiddly and will
occasionally be wrong after a large edit. Accepted: the failure mode is
cosmetic and self-correcting.

**Reranking may not fit the budget.** A cross-encoder on CPU is the one part of
this document whose cost is not yet known. The design degrades to fusion order
rather than to waiting.

**The band may be noise.** If the divider is tuned loosely, the band becomes
another suggestion box and gets switched off. Tuning it tightly — three
passages, often zero — is the design, not a fallback.

**`Tab` is overloaded.** Three consumers, one key. The precedence rule above is
the whole answer, and it needs to hold in every mode.
