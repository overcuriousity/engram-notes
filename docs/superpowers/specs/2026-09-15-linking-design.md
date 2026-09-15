# Linking (roadmap 0.5) — design

Step 0.5 of `ROADMAP.md`. The direction spec
(`2026-09-14-writing-first-direction-design.md`, sections *Linking*,
*Passages*, *Retrieval*) says what and why; this document fixes the shape,
records the decisions it left open, and is what the implementation plan
builds from.

## In one paragraph

What the writer reaches for is found by meaning as readily as by spelling.
Completion inside `[[…]]` becomes hybrid. A passage link is picked in two
steps, note then block, from the same retrieval, and written in Obsidian's
own form, `[[Note#^id|the words I selected]]`, with the anchor written into
the target only when the link is made and only where a heading cannot serve.
The identifier is never the thing on screen: the alias renders, and hovering
shows the target.

## Decisions taken here

Each of these was open in the direction spec or is a departure from today's
code. They are settled; the plan does not re-decide them.

- **One passage per note.** The passage is the note's body after the
  frontmatter, truncated at the end to the model window. A note's important
  part is at its beginning, and retrieval, the graph's semantic edges, the
  Related pane and the divider all stay note-level, which is the level the
  memory reasons at. The list-item rule of the direction spec moves from the
  splitter to *blocks* below, where the picker needs it.
- **Two steps in the picker.** Notes first, then the chosen note's blocks.
  The writer sees the line before the link is made.
- **Two ways in.** A command with a hotkey for the selection gesture, and
  Obsidian's `[[^^` inside completion for the vault-wide block search.
- **No rerank in this step.** The cross-encoder is a stage between fusion and
  the divider; it ships with step 0.6 together with the bundled models, at
  the latest with the step that ships the integrated embedding model, and
  the `Reranker` trait and its fake arrive with it. Nothing in 0.5 is shaped
  so that inserting that stage means more than one call at one point.
- **Reuse `search::hybrid`.** Both surfaces call it with a small limit. There
  is no second ranking path.

## Passages

`search::passages::split` returns at most one passage: ordinal 0, empty
heading path, line 1, the body trimmed and truncated at the end to
`MAX_CHARS`, preferring a whitespace boundary. The body is what `parse`
already yields with the frontmatter removed. An empty body yields nothing.

The `passages` and `vectors` tables keep their shape; `vectors` stays keyed
by the text hash, so an unchanged body keeps its embedding across a rename or
a re-save. `SCHEMA_VERSION` bumps, which rebuilds.

`search_vectors`, `hybrid`, `related`, `semantic_edges` and `pending_vectors`
all address rows by `(path, ordinal)` or hash and never assume more than one
row per note, so they keep working. What they return changes, and says so:
a passage carries no heading and starts at line 1, so `Hit` loses its
`heading`, a result opens at the note's first body line, and `SimilarNote`
shows a lead rather than the body.

## Blocks

`core::linking::blocks(body) -> Vec<Block>`, pure, on the body without
frontmatter. A block is one of:

| kind      | what it covers                                            |
|-----------|-----------------------------------------------------------|
| `Heading` | the heading line; its text is the heading text            |
| `List`    | a list item with its whole subtree, however deep          |
| `Paragraph` | consecutive non-blank lines that are neither of the above, including quotes and tables |

Each block carries `first` and `last` (1-based lines in the body), its `text`,
and for a `Heading` the heading text. Fenced code is skipped whole. A line
that is only a `^id` is part of the block above it. A list item's subtree is
every following line indented deeper than the item's marker, plus blank
lines inside that run; a sibling or shallower line ends it. This is a parser
rule and does not depend on the outliner.

`core::linking::best_block(blocks, query) -> Option<usize>` picks the block
to preselect: the one with the most distinct query terms (lowercased,
diacritics kept, two characters or longer) present in its text, ties to the
earlier block, `None` when nothing matches.

## Anchors

`core::linking::anchor(text, block, fresh) -> Anchored` on the whole file text,
given the block and a fresh id:

- A `Heading` block never gets an anchor; the link is `[[Note#Heading]]`.
  `[`, `]`, `|`, `#` and `^` cannot be written inside the fragment, so the
  link drops them and whitespace collapses: `## Pros | Cons` is linked as
  `[[Note#Pros Cons]]`. `linking::heading_key` drops the same characters on
  both sides, so `anchor_line` and `preview` still resolve that heading.
- If the block's anchor line already ends in `^existing`, that id is returned
  and the text is returned unchanged.
- Otherwise ` ^id` is appended to the anchor line and nothing else in the
  file changes: not line endings, not trailing whitespace elsewhere, not the
  frontmatter.

The anchor line is the block's **first** line for a `List` (Obsidian keys a
list item by its own line and the reference includes the children) and the
**last** line for a `Paragraph` (Obsidian's paragraph id sits at the end).

Ids are Obsidian's form: six characters from `[a-z0-9]`, random. `anchor`
takes the id as a `FnMut() -> String` and calls it again while the id
collides with any `^id` already in the file, so the shell supplies the
randomness and the uniqueness rule is tested in core with a fixed sequence.

The shell reads the target through the vault, applies `anchor`, writes it
back through the vault's atomic write only when the text changed, and
re-indexes the file. An external-edit conflict on the target surfaces like
any other write error; the link is not inserted then.

## Retrieval for both surfaces

One new core function, `search::link_candidates(index, query, query_vec,
cfg, mem, at, limit) -> Vec<LinkCandidate>`:

1. **Spelling:** notes whose title, stem or path contains the query,
   case-insensitively, in the index's existing title order. Kind `text`.
2. **Meaning:** `hybrid` with the same query, limited to `limit`, hits above
   the divider only, in its order, minus notes already listed by spelling.
   Kind `meaning`. Priming applies inside `hybrid` as it does everywhere.
3. Together, truncated to `limit`.

`LinkCandidate { path, title, kind, primed }`. An empty query yields the
spelling list of every note, so an empty `[[` still offers everything, as it
does today. With no model loaded, `hybrid` runs full-text only and the
meaning list is whatever the full-text branch finds.

## The picker

A palette mode, `link`. It is entered by:

- the command *Link: to a passage*, hotkey `Ctrl+Shift+K`. With a selection
  in an editing pane, the selection is the query and becomes the alias.
  Without one, the input is the query and the link carries no alias.
- `[[^^` typed in an editing pane: completion recognises it and opens the
  picker with whatever follows `^^` as the query and no alias, replacing the
  typed `[[^^…` when the link is inserted. This is Obsidian's gesture, so it
  behaves as Obsidian's does: no alias.

Step one lists `link_candidates`, meaning matches marked, primed marked as
search marks them. Enter or click goes to step two: the chosen note's
`blocks`, each shown as its first line with the rest dimmed, `best_block`
preselected. Escape returns to step one with the query intact; Escape again
closes. Enter on a block:

1. `Heading` → link `[[Stem#Heading]]`, nothing written.
2. otherwise → the shell's `anchor_block(path, first, last)` command writes
   the anchor if needed and returns the id; link `[[Stem#^id]]`.
3. the alias is appended as `|words` when there is one.
4. the link is inserted through the existing `insertAtCursor`, replacing the
   selection when the alias came from it, or the `[[^^…` text when
   completion opened the picker.

The link's target is the note's stem, as `[[` completion writes it today;
resolution is the index's existing rule.

## Completion

`wikiCompletion` becomes async. On each change inside `[[…` it calls
`link_candidates` with the typed text, debounced at 80 ms, and lists the
result with `detail` set to the path and meaning matches marked *meaning*.
Selecting inserts `[[Stem]]` as before. The substring pass runs on the
backend now, so the frontend no longer keeps a title list for this purpose;
the tag completion is untouched. Typing `[[^^` hands over to the picker and
returns no completions of its own. While the backend has not answered, the
popup keeps its last list rather than flickering empty.

## Hover

A shell command `link_preview(path, fragment) -> Option<Preview>` resolves a
link target: for `^id` the block that carries it, for `#Heading` the heading
and the lines under it up to the next heading of the same or higher level,
for a bare note the first block. `Preview { title, heading, text }`, text
capped at forty lines.

Live preview's `WikiWidget` and reading mode's rendered links show a small
popover after the pointer rests on a link for 300 ms, with the note title,
the heading path and the text, and dismiss it on leave. The alias renders as
it does today; the `^id` is never shown in the popover or the link.

## Errors

- The target file cannot be read or written: the error shows in the status
  bar, no link is inserted.
- The chosen block no longer exists because the file changed under the
  picker: `anchor_block` returns `Error::Conflict`, the picker says so and
  returns to step two with fresh blocks.
- No editing pane is active for the command: the same message the template
  picker gives.

## Testing

In `core`, without a model or a window:

- passages: one per note; truncation is at the end and on whitespace; an
  empty body yields none; the hash follows the text.
- blocks: a list item packs its subtree at any depth; a sibling ends it;
  fenced code is skipped; a trailing `^id` line belongs to its block;
  headings, paragraphs, quotes and tables have the expected lines.
- best_block: term overlap picks the expected block; ties go to the earlier;
  no overlap gives none.
- anchors: a paragraph's id goes on its last line, a list item's on its
  first; an existing id is reused and the text is unchanged; ids are unique
  in the file; every other byte of the file is untouched; a heading writes
  nothing.
- link_candidates: spelling before meaning; a note both branches find is
  listed once as spelling; nothing below the divider; an empty query lists
  every note.
- preview: `^id`, heading and bare targets resolve to the expected text.

In the frontend, with vitest: `[[^^` is recognised and the completion returns
nothing for it; the picker's two steps and Escape; the inserted text with and
without an alias, and over a selection.

Smoke lines, in `docs/smoke.md`: hybrid completion offering a note by
meaning; the command with a selection; `[[^^`; a heading target writing
nothing; hover on a passage link showing the passage.

## Out of scope

The reranker and `rerank_n` (0.6). The recall band (0.7). Block links inside
a note via `[[Note#^` completion, Obsidian's third block gesture: the picker
covers the need and completion stays small. Any rendering of `^id` beyond
the muted mark live preview already shows.
