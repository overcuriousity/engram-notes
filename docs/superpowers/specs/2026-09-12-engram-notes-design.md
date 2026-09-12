# engram-notes: a markdown note taker with a graph, bases and semantic recall

Written 2026-09-12. The first design record for this repository; nothing
precedes it.

## Why

Obsidian organises notes as a folder of markdown files, links them, and draws
the graph. It is closed source. Logseq is open source but stores blocks rather
than files and ships whiteboards and flashcards that a note taker does not ask
for. engram (`overcuriousity/engram`) searches by meaning and keeps a memory
that rearranges itself on use, but it is a server with a capture pipeline, not
an editor.

engram-notes takes the folder-of-markdown, links, graph and bases from
Obsidian; the open licence from Logseq; and the semantic search and memory
concepts from engram, reimplemented locally so the application is one
executable with no server behind it. It is a note taking application first.
Search and memory serve the notes.

## What it is not

No whiteboard, no flashcards or cloze, no plugin system, no sync, no mobile,
no collaboration. Not a client of engram: it runs on its own. No LLM
question-answering in the first version; see Roadmap.

## Decisions this rests on

1. **Files are the truth.** A vault is a folder of `.md` files. Every index,
   vector and memory value is derived and can be rebuilt from the folder. A
   schema change rebuilds the index rather than migrating it.
2. **Obsidian is the reference where Obsidian is the inspiration.** Link
   syntax, frontmatter, the `.base` format, the vault-level `.obsidian`-style
   config folder, daily notes, conflict handling and default shortcuts follow
   Obsidian's behaviour. Nothing Obsidian-shaped is reinvented.
3. **engram is the reference for search and memory.** Dense plus sparse
   retrieval fused; a divider where relevance falls off; activation that
   decays; links learned from co-retrieval; a bounded priming lift; a spread
   of associated notes. Reimplemented in a small slice, not linked.
4. **One executable per platform.** Tauri 2 produces a native window with a
   webview; Rust holds all state and logic; the frontend renders and edits.
5. **KISS.** Brute-force vector scan over SQLite blobs until measurement says
   otherwise. One trait per seam, one implementation per trait until a second
   is needed. Comments say why, briefly.

## The vault

A vault is a directory chosen at startup or from the recent list. One window
shows one vault.

**Notes** are `.md` files anywhere below the vault root, except inside hidden
directories. A note's identity is its path relative to the root. Its title is
the filename without extension unless frontmatter sets `title`.

**Syntax recognised** (Obsidian's):

- `[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`, `[[Note#^block]]`,
  `![[Note]]` and `![[image.png]]` embeds.
- Standard markdown links `[text](path.md)` and images.
- YAML frontmatter between leading `---` fences. Properties are typed as
  Obsidian types them: text, list, number, checkbox, date, datetime.
- `#tag` and `#nested/tag` in the body, and the `tags` property.
- Headings, task checkboxes, fenced code, tables, callouts (`> [!note]`).

**Link resolution**, in order: exact relative path; basename anywhere in the
vault; with the `.md` extension implied. Case-insensitive. Ambiguous basenames
resolve to the shortest path and the index records the ambiguity. Unresolved
links are kept in the index so the graph can draw them hollow and the editor
can create the note on click.

**Rename and move** rewrite every wikilink and markdown link that pointed at
the old path, after a dialog lists the files that will change. Memory values
(activation, learned links) follow the note.

**Vault-level config** lives in `<vault>/.engram-notes/`: `app.json`
(settings), `workspace.json` (open tabs, sidebar state), `graph.json` (graph
filters). It travels with a synced vault exactly as `.obsidian/` does. The
folder is created on first open and is never indexed.

**Per-vault derived state** lives in the OS data directory
(`dirs::data_dir()/engram-notes/vaults/<hash of vault path>/index.db`). It is
large, rebuildable and must not be synced.

**Attachments**: non-markdown files are listed in the explorer, previewed
where the webview can (images, PDF), never indexed or embedded.

## The index

SQLite through `rusqlite` with the bundled engine. One file per vault.

Tables (columns abbreviated):

- `notes(path PRIMARY KEY, title, mtime, size, hash, frontmatter JSON, body)`
- `links(src_path, target_raw, target_path NULL, kind, heading NULL, line)` —
  `kind` is wikilink, markdown or embed; `target_path` is `NULL` when
  unresolved.
- `tags(path, tag)`
- `properties(path, key, value_json, value_type)`
- `notes_fts` — FTS5 over title and body, external content on `notes`.
- `passages(id, path, heading, ordinal, text, hash)`
- `vectors(passage_id PRIMARY KEY, model, dim, embedding BLOB)`
- `activation(path PRIMARY KEY, value REAL, stamped_at)`
- `assoc(a_path, b_path, value REAL, stamped_at, queries JSON)` — learned
  links from co-retrieval, undirected, `a < b`.
- `sittings` and `events` — see Memory.
- `meta(key, value)` — schema version, embedding model id.

**Rebuild on open**: walk the vault, compare `(mtime, size)` with the index,
re-hash and re-parse only what changed, delete rows for files that are gone.
A vault of ten thousand notes must open in under two seconds on the second
run.

**Watcher**: `notify` with a 300 ms debounce. A change event re-parses that
file and updates links, tags, properties, FTS and passages; embedding is
queued.

**Parser**: `pulldown-cmark` for the markdown structure and a small scanner
for wikilinks and tags, since neither is CommonMark. Frontmatter through
`serde_yaml`. The parser produces one `ParsedNote` value that the index writes;
it does no I/O.

## The editor and UI

**Stack**: Tauri 2, Svelte 5 with TypeScript, Vite, CodeMirror 6.

**Editor modes**: live preview (default), source, reading. Live preview is
CodeMirror decorations: on lines the cursor is not on, markup is hidden and
headings, emphasis, inline code, links, checkboxes and callouts are rendered
in place; the active line shows its source. Reading mode renders the whole
note through `markdown-it` with plugins for wikilinks, tags, callouts and
embeds. Source mode is CodeMirror with markdown highlighting only.

**Editor features**: `[[` autocompletes note names, `#` autocompletes tags,
`Ctrl+Click` follows a link and creates the note when it does not exist,
checkbox toggling in preview, code block highlighting, find and replace,
undo history per tab, autosave on a 500 ms debounce and on blur.

**Layout**: left sidebar with file explorer (folders, drag to move, context
menu for rename, delete, new note, new folder) and search; centre with tabs
and horizontal and vertical splits; right sidebar with backlinks, outgoing
links, properties (editable, writes frontmatter), and *Related* (see Memory).
Status bar shows word count, embedding queue state and the memory toggle.

**Global actions**: command palette `Ctrl+P`, quick switcher `Ctrl+O`, search
`Ctrl+Shift+F`, new note `Ctrl+N`, today's daily note, toggle graph, toggle
sidebars. Shortcuts are Obsidian's defaults and are rebindable in `app.json`.

**Daily notes**: one command opens or creates `<folder>/<YYYY-MM-DD>.md`
from an optional template file, both set in `app.json`. Nothing else.

**Themes**: light and dark on CSS custom properties, following the OS by
default. The variable names are stable so a user stylesheet can override
them; that is the whole theming story.

**External edits**: the watcher sees a change to a note that is open. If the
editor buffer is unmodified, the tab reloads silently. If it is modified, the
buffer stays, a bar appears with *reload from disk* and *keep mine*, and the
file is not written until the user chooses. The last write wins otherwise, as
in Obsidian.

## The graph

A canvas-drawn force layout, `d3-force` for the simulation and a hand-written
canvas renderer for speed at ten thousand nodes. Nodes are notes; explicit
links are solid edges; unresolved targets are hollow nodes. Global graph and a
local graph (depth 1 to 3 around the current note) share the renderer.

**Semantic edges**: with a toggle in the graph controls, learned associations
from `assoc` and the top-k nearest passages by embedding are drawn as dashed
edges, weighted by strength. The graph then shows what the vault is about,
not only what was linked by hand. Off by default in the global graph, on by
default in the local graph.

Filters: search text, tags, folders, show orphans, show attachments. Node size
by inbound link count. Click opens; hover highlights neighbours. Layout
parameters and filters persist in `graph.json`.

## Bases

A `.base` file is Obsidian's YAML: `filters`, `formulas`, `properties`,
`views`. Opening one renders its first view. The first version implements the
table view: columns are properties or formulas, rows are notes matching the
filter, sorting per view, and cells edit the note's frontmatter in place.

The filter and formula language is Obsidian's expression syntax. The first
version implements the subset needed for day-to-day bases and documents it in
`docs/bases.md`: comparison and boolean operators, `file.name`,
`file.folder`, `file.tags`, `file.hasTag()`, `file.inFolder()`,
`file.hasLink()`, `note.<property>`, `date()` and `now()`, string and
number literals, `.contains()` and `.isEmpty()`. Anything outside the subset
is reported in the view as an unsupported expression, never silently true or
false.

Bases are evaluated in Rust against the index, so a view over ten thousand
notes is one query. Card view and embedding a base in a note come later.

## Search

**Full-text**: FTS5 BM25 over title and body, with prefix matching. Always
on, no model needed. Results show the note, a highlighted snippet and the
heading it sits under.

**Semantic**: passages are the units. A note is split on headings, then on
paragraphs, into pieces of at most the model's window; a heading path is
prepended so a passage carries its context. Each passage is embedded once and
re-embedded when its hash changes. Query embedding uses the model's query
prompt where the model distinguishes queries from documents (e5 does).

**Retrieval**: cosine over all vectors in one scan, top `k × multiplier`;
BM25 top `k × multiplier`; reciprocal rank fusion into one list, one entry per
note with its best passage as the snippet. A divider is drawn where the fused
score falls below a fraction of the top score, and hits below it are shown
smaller, in rank order, labelled loose. The numbers are settings with the
values engram ships.

**Backend trait**: `Embedder { fn embed_documents(&[String]) -> Vec<Vec<f32>>;
fn embed_query(&str) -> Vec<f32>; fn id() -> &str; fn dim() -> usize }`.
Implemented by `FastEmbedder` and by a deterministic `FakeEmbedder` for tests
that hashes words into a vector.

**Model**: fastembed with `multilingual-e5-small` quantized as the default.
On first run the model is fetched to
`data_dir/engram-notes/models/<id>/` with a progress indicator in the status
bar; full-text search, the graph and bases work while it downloads, and
semantic results appear once the queue drains. Settings offer *model
directory*: a path to a directory holding the ONNX file and tokenizer, for
machines without network. Changing the model clears `vectors` and re-embeds.

**Embedding queue**: one background thread, batches of 32 passages, lowest
priority, paused while the user types. The status bar shows *n passages
pending*.

## Memory

A small slice of engram, chosen for a single-user editor with no verdict UI.
Every value is stored as `(value, stamped_at)` and read through
`decayed(value, stamped_at, now, half_life)`, so learning is one write and
forgetting costs nothing.

**Events**: opening a note, opening a note from a search result, following a
link, and running a search are appended to `events(at, kind, path, query,
sitting)`. A *sitting* is a session of activity separated by thirty minutes
of silence.

**Activation** per note: bumped on open, half-life 30 days by default. It is
the memory's picture of what the user reaches for.

**Association** between two notes: bumped when both are opened from the same
search, or opened in the same sitting within ten minutes. Stores the queries
that bound them, three at most, as a cue the Related pane can show. Half-life
90 days.

**Priming**: a search hit whose activation is above the median may climb at
most `prime_lift` places (default 2). Bounded, so a favourite never buries a
better match.

**Spread**: under the fused list, at most `spread_max` (default 3) notes that
are associated with the top hits but did not themselves match, labelled as
associated. This is the engram idea that two texts can be strangers to the
embedding and inseparable to the person.

**Related pane** for the open note has three groups: *associated* (from `assoc`, strongest first, with the cue query), *similar*
(nearest passages by embedding, one entry per note), and *suggested links*.

**Suggested links**: a similar note whose title or alias does not appear in
the current note's text, and which the current note does not already link,
with a one-click *link* action that inserts `[[Title]]` at the cursor. This
is Obsidian's unlinked mentions extended by meaning.

**One setting**: `memory.enabled`. Off records no events and bumps nothing;
priming and spread are skipped; the Related pane shows *similar* only. The
data already learned is kept and one button forgets it.

## Code structure

A Cargo workspace:

```
core/                 library, no Tauri, all logic and all tests
  src/vault.rs        walking, reading, writing, rename with link rewrite
  src/parse.rs        markdown, frontmatter, wikilinks, tags -> ParsedNote
  src/index/          schema, rebuild, watcher-driven update, queries
  src/search/         fts, vector scan, fusion, divider
  src/embed/          Embedder trait, fastembed impl, fake, model fetch
  src/memory/         events, decay, activation, association, priming, spread
  src/bases/          .base parsing, expression parser and evaluator, views
  src/config.rs       app.json, defaults
src-tauri/            thin shell: commands, watcher thread, embed thread, state
ui/                   Svelte + TypeScript frontend
docs/                 bases.md, specs, plans
```

Tauri commands are the only frontend-facing API; each is a thin call into
`core`. The frontend receives plain JSON and never touches the filesystem.
Push from backend to frontend (index updated, embed progress, external
change) goes through Tauri events.

## Error handling

Core returns `Result<T, Error>` with a small `thiserror` enum: `Io`,
`Parse`, `Index`, `Embed`, `Base`. The Tauri layer maps errors to a
`{code, message}` JSON the UI shows in a toast. A failing embedding never
blocks editing; a failing watcher falls back to rescanning on window focus; a
corrupt index is deleted and rebuilt with a notice. Writes to notes are
atomic: write a temp file beside the note and rename over it.

## Testing

`core` is tested without a model and without a window:

- parse: every link form, frontmatter types, tags, edge cases (links in code
  blocks are not links).
- vault: rename rewrites links, atomic write, hidden dirs skipped.
- index: rebuild from a temp vault, incremental update, deletion, ambiguity.
- search: fusion order, divider placement, one entry per note.
- memory: decay arithmetic, priming bound, spread selection, sitting split.
- bases: expression parser, each supported function, unsupported reported.

`src-tauri` compiles in CI and has smoke tests for command serialisation. The
frontend has `svelte-check` and vitest for the editor decorations and the
command palette. A manual checklist in `docs/smoke.md` runs on a real vault
before a release.

## Repository

GPL-3.0-only. `AGENTS.md` sets the comment policy and conventions;
`CLAUDE.md` points at it. Dependabot for cargo, npm and github-actions,
grouped weekly. CI: `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test`, `pnpm check`, `pnpm test`. Release: on a `v*` tag,
`tauri-action` builds Windows (`.msi`, `.exe`), Linux (`.AppImage`, `.deb`)
and macOS (`.dmg`) and attaches them to a GitHub release. Semver from
`0.1.0`.

## Roadmap

In order, each its own spec:

1. **Ask.** Question answering over the vault with an OpenAI-compatible
   endpoint, sourced generously from engram's `core/ask`: planned retrieval,
   abstaining out loud, badging claims no excerpt supports.
2. **engram as a storage backend.** The search and memory trait gets a second
   implementation that delegates to a running engram, which brings its
   self-grading and sleep to the vault.
3. Card view for bases and bases embedded in notes.
4. Templates beyond the daily note, and a user stylesheet picker.
