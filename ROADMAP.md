# Roadmap

What engram-notes is becoming, in the order it is being built. The two design
records this merges are the source of detail; this page is the order and the
state.

- [`docs/superpowers/specs/2026-09-12-engram-notes-design.md`](docs/superpowers/specs/2026-09-12-engram-notes-design.md)
  — the foundation: a folder of markdown with links, a graph, bases, hybrid
  search and a memory of use.
- [`docs/superpowers/specs/2026-09-14-writing-first-direction-design.md`](docs/superpowers/specs/2026-09-14-writing-first-direction-design.md)
  — the direction: writing is the centre, engram's retrieval lives inside the
  linking gesture, and the vault can say what it knows.

Legend: ✅ shipped on `master` · 🔶 merged or in review, smoke not yet run ·
⬜ not started.

## The position

> Logseq's ergonomics, Obsidian's files, engram's retrieval inside the gesture.

Three arguments, and only three, against the stack serious users build on
Obsidian today: it is **open**; it is **one piece** sharing one index instead
of five plugins each keeping their own; and it has **a memory of use** —
every search, open and link is an event, and no plugin has that because
Obsidian keeps no record.

Two things are not on this roadmap by decision: forgetting (nothing is ever
hidden, pruned or proposed for deletion — the vault is also an instrument of
investigation and documentation), and generative AI in any path the writer
cannot avoid.

## 0.1 — The foundation ✅

Everything the first design describes, less four items that design defers
itself; they are steps 0.3 and 0.10 below.

- ✅ Vault: files are the truth; `.engram-notes/` travels with the vault;
  watcher, external-edit conflict bar, atomic writes, rename with link rewrite.
- ✅ Index: SQLite with FTS5, incremental rebuild, schema bump rebuilds.
- ✅ Editor: live preview, source, reading; `[[` and `#` completion; Ctrl+Click
  follows and creates; checkboxes; autosave.
- ✅ Layout: explorer, tabs, splits, right sidebar (backlinks, outgoing,
  properties, all properties, related), status bar, ribbon, frameless window,
  command palette, Ctrl+K search. Daily notes, themes, settings.
- ✅ Graph: global and local, filters, semantic edges, settings in `graph.json`.
- ✅ Bases: `.base` YAML, table view, the documented filter and formula subset,
  sorting, cells that edit frontmatter.
- ✅ Search: FTS5 + dense (`multilingual-e5-small`), reciprocal rank fusion,
  the divider, a similarity floor (`docs/memory.md`).
- ✅ Memory: events, sittings, activation, association, bounded priming,
  spread, the Related pane, `memory.enabled`, one button to forget.

## 0.2 — The editor's floor 🔶

Parity with Obsidian and its outliner plugin, over ordinary markdown lists.
[PR #1](https://github.com/overcuriousity/engram-notes/pull/1) is merged; what
remains is smoke lines 49–54 in the real window and the fold gutter measured
beside Obsidian (`docs/memory.md`).

- 🔶 A bracket typed over a selection wraps it.
- 🔶 Enter continues a list; on an empty item it drops a level of markup.
- 🔶 Tab / Shift+Tab move an item with its subtree; Alt+↑/↓ move it among
  siblings; ordered lists renumber. Tab precedence: completion → outline →
  indentation.
- 🔶 List items and headings fold from a gutter chevron. Folds are remembered
  per note in the derived index, never in the file, and follow a rename.
- 🔶 `editor.indent`, default two spaces.

## 0.3 — Templates and stylesheets 🔶

The two small things an Obsidian user reaches for on the first day, in
Obsidian's shape. A templates folder in `app.json` and one command that
inserts a template at the cursor, with `{{date}}`, `{{time}}` and `{{title}}`;
the daily note keeps drawing on the same mechanism. CSS snippets: `.css` files
in `.engram-notes/snippets/`, each switched on or off in settings, over the
stable variable names the themes already expose. In review; what remains is
smoke lines 55–58 in the real window.

- 🔶 `templates.folder`, `date_format` and `time_format` in `app.json`, in
  Obsidian's tokens (`YYYY-MM-DD`); a strftime pattern still works.
- 🔶 *Templates: Insert template* picks from the folder and inserts at the
  cursor, over the selection; `{{date:FMT}}` and `{{time:FMT}}` pick their own
  format. The daily note's template is filled the same way.
- 🔶 `.engram-notes/snippets/*.css`, each on or off under Appearance, written to
  `css_snippets` in `app.json`; `body.theme-dark` / `.theme-light` say what is
  on screen.

## 0.4 — The Logseq importer 🔶

The door the named population walks through. One command, the original
untouched, a report of everything that could not be mapped. Journals to daily
notes, namespaces to folders, `key:: value` to frontmatter, `id::` to `^id`
anchors, `((uuid))` to `[[Page#^id]]`, `collapsed::` dropped, task markers to
checkboxes, `#[[multi word]]` to a link. In review; what remains is smoke
lines 63–67 in the real window.

- 🔶 *Import: Logseq graph* picks the graph and a folder in the vault; the
  graph is only read; `import-report.md` names every file and line that was
  kept as text or left out; nothing already in the vault is overwritten.
- 🔶 Journals to the daily folder, `a___b.md` to `a/b.md`, page properties
  to frontmatter, `id::` to `^anchor` and `((uuid))` to `[[Page#^anchor]]`,
  `{{embed}}` to `![[…]]`, `collapsed::` dropped, task markers to
  checkboxes, `#[[multi word]]` to a link, tabs to `editor.indent`,
  `assets/` copied beside the notes (`docs/import.md`).
- 🔶 Idempotent: the output imports to itself, and a second run over the
  vault writes nothing.

## 0.5 — Linking 🔶

The primary engram surface. Completion inside `[[…]]` becomes hybrid —
substring for what you know exactly, meaning for what you can only paraphrase.
A text-level link is picked from **passages**, hybrid and reranked, and written
in Obsidian's own form, `[[Note#^id|your words]]`; the identifier is never the
thing on screen. The trade is stated: a stable link to a line in a folder of
files requires an anchor in the target file, and this does what Obsidian does.
In review; what remains is smoke lines 69–75 in the real window
(`docs/superpowers/specs/2026-09-15-linking-design.md`).

- 🔶 One passage per note; `[[` completion lists spelling matches, then
  meaning matches marked as such, primed by memory.
- 🔶 *Link: to a passage* (Ctrl+Shift+K) and `[[^^`: notes, then blocks;
  `[[Note#^id|words]]`, the anchor written only then, a heading target
  written without one; an existing id reused.
- 🔶 Hovering a link shows the target passage; the id is never on screen.
- 🔶 The reranker: ships with 0.6 and the bundled models.

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
  rescored over their leads, the divider on that score.
- 🔶 Over budget on load or on a query halves the batch; under five pairs
  reranking is off for the session and the status bar names the time.

## 0.7 — Recall while writing, and the event log ⬜

The Related pane becomes a recall band: what the paragraph under the cursor
pulls toward, up to five passages, nothing below the divider, with a cadence
the writer sets (live · on paragraph end · on keypress · off). And memory's
truth moves to an append-only event log in the vault, one file per install,
which any sync tool merges; activation and association become derived from it
like every other index.

## 0.8 — Verdicts ⬜

engram's core idea. Opening a result is a verdict at its rank; a small
explicit yes/no adds what clicks alone cannot. recall@10 and MRR in settings.
An idle-time run replays real searches against a grid of settings and, when
one is better by a margin, **proposes** it with one button — never applies it
silently.

## 0.9 — Bases that filter by meaning ⬜

`similarity("text")` and `similar("text")` in the Bases expression language,
documented as ours. `similar("shell companies") and file.hasTag("case-42")`
is a living topic page as a file. Obsidian cannot do this; it has no
embeddings.

## 0.10 — Bases in notes and as cards ⬜

The base as a block: `![[Topic.base]]` renders the view inside a note, in
live preview and reading mode, with its sorting and its editing cells; with
0.9 that is a living topic page inside the note that discusses it. And the
card view, Obsidian's second view type: a cover image from a property, the
title, the chosen properties beneath, in a grid.

## 0.11 — Topics without a home ⬜

Entity-agnostic by construction. Passages cluster by meaning; a cluster
drawing on three or more notes with no note near its centre is offered one,
named from its distinctive terms, pre-filled with links to every passage —
and nothing is created without a click.

## Later ⬜

Provenance and capture into the vault; Ask that cites and abstains, behind a
user-supplied endpoint; the graph's remaining polish.

## Alongside, not as a phase ⬜

Reproducible builds, signed releases, Flatpak and AUR packaging instead of
curl-to-sh. Today: a CI build on every commit to `master`, an installer
script, an unsigned Windows executable.

## How this page is kept

A step moves to ✅ when its plan's smoke lines pass in the real window and it
is on `master`. The numbers are this page's: the direction spec's *Order of
work* predates steps 0.3 and 0.10 and counts from there. New work gets a
spec under `docs/superpowers/specs/` first and a line here second; this page
never carries detail the specs do not.
