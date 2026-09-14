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

Legend: ✅ shipped on `master` · 🔶 in review · ⬜ not started.

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

Everything the first design describes, less the two items marked below.

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
- ⬜ Card view for bases; bases embedded in notes.
- ⬜ Templates beyond the daily note; a user stylesheet picker.

## 0.2 — The editor's floor 🔶

Parity with Obsidian and its outliner plugin, over ordinary markdown lists.
[PR #1](https://github.com/overcuriousity/engram-notes/pull/1).

- 🔶 A bracket typed over a selection wraps it.
- 🔶 Enter continues a list; on an empty item it drops a level of markup.
- 🔶 Tab / Shift+Tab move an item with its subtree; Alt+↑/↓ move it among
  siblings; ordered lists renumber. Tab precedence: completion → outline →
  indentation.
- 🔶 List items and headings fold from a gutter chevron. Folds are remembered
  per note in the derived index, never in the file, and follow a rename.
- 🔶 `editor.indent`, default two spaces.

## 0.3 — The Logseq importer ⬜

The door the named population walks through. One command, the original
untouched, a report of everything that could not be mapped. Journals to daily
notes, namespaces to folders, `key:: value` to frontmatter, `id::` to `^id`
anchors, `((uuid))` to `[[Page#^id]]`, `collapsed::` dropped, task markers to
checkboxes, `#[[multi word]]` to a link.

## 0.4 — Linking ⬜

The primary engram surface. Completion inside `[[…]]` becomes hybrid —
substring for what you know exactly, meaning for what you can only paraphrase.
A text-level link is picked from **passages**, hybrid and reranked, and written
in Obsidian's own form, `[[Note#^id|your words]]`; the identifier is never the
thing on screen. The trade is stated: a stable link to a line in a folder of
files requires an anchor in the target file, and this does what Obsidian does.

## 0.5 — Out of the box ⬜

The embedder and a cross-encoder reranker ship **inside the release
artifact**: no download on first run, no model directory, no network. A page
in the repository states exactly what reaches the network, and it says
"nothing" for the default build. Reranking runs for deliberate search and the
passage picker, within a 500 ms budget, and degrades to fusion order rather
than to waiting.

## 0.6 — Recall while writing, and the event log ⬜

The Related pane becomes a recall band: what the paragraph under the cursor
pulls toward, up to five passages, nothing below the divider, with a cadence
the writer sets (live · on paragraph end · on keypress · off). And memory's
truth moves to an append-only event log in the vault, one file per install,
which any sync tool merges; activation and association become derived from it
like every other index.

## 0.7 — Verdicts ⬜

engram's core idea. Opening a result is a verdict at its rank; a small
explicit yes/no adds what clicks alone cannot. recall@10 and MRR in settings.
An idle-time run replays real searches against a grid of settings and, when
one is better by a margin, **proposes** it with one button — never applies it
silently.

## 0.8 — Bases that filter by meaning ⬜

`similarity("text")` and `similar("text")` in the Bases expression language,
documented as ours. `similar("shell companies") and file.hasTag("case-42")`
is a living topic page as a file. Obsidian cannot do this; it has no
embeddings.

## 0.9 — Topics without a home ⬜

Entity-agnostic by construction. Passages cluster by meaning; a cluster
drawing on three or more notes with no note near its centre is offered one,
named from its distinctive terms, pre-filled with links to every passage —
and nothing is created without a click.

## Later ⬜

Provenance and capture into the vault; Ask that cites and abstains, behind a
user-supplied endpoint; card views; the graph's remaining polish.

## Alongside, not as a phase ⬜

Reproducible builds, signed releases, Flatpak and AUR packaging instead of
curl-to-sh. Today: a CI build on every commit to `master`, an installer
script, an unsigned Windows executable.

## How this page is kept

A step moves to ✅ when its plan's smoke lines pass in the real window and it
is on `master`. New work gets a spec under `docs/superpowers/specs/` first and
a line here second; this page never carries detail the specs do not.
