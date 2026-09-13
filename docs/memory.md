# Search and memory

What engram-notes borrowed from engram, with the numbers it ships and the
places it does not follow the spec.

## Retrieval

Full-text (FTS5 BM25, title weighted 10) and semantic (cosine over every
passage vector) each fetch `limit × 3` candidates. Reciprocal rank fusion with
`k = 60` folds them into one list, one entry per note, its best passage as the
snippet. With no model loaded the full-text branch answers alone, which is why
search works while the model downloads.

Passages are the body split on headings, then on paragraphs, at most 1 200
characters, the heading path prepended before embedding. A vector is keyed by
the hash of that text, so an unchanged passage in a renamed or re-saved note
keeps it.

**The floor.** A cosine below `similarity_floor` (0.83) says nothing, and the
hit is a stranger. This is not in the spec; it is here because the model needs
it. Measured over the 45 note pairs of a ten-note demo vault,
`multilingual-e5-small` puts every pair between 0.755 and 0.891 — two notes
with nothing whatever in common still score 0.755, and the median is 0.807. Any
rule that reads such a cosine as a similarity calls the whole vault related to
everything, which is what the first build did. Same-folder pairs have a median
of 0.828 and a minimum of 0.772, cross-folder a median of 0.791 and a maximum
of 0.866, so the two populations overlap and no threshold is clean; 0.83 is
where it separates them best on that vault. The number belongs to this model,
and a model loaded from a folder may want another.

A relative band on top of the floor (`max(floor, best - 0.04)`) was tried and
dropped: over the same pairs it changed the answer for two notes of ten and in
both it deleted a real neighbour — Borrowing lost Index at 0.832, Smart
pointers lost Error handling at 0.838 — while never adding one. Tightening
around a standout is the cliff's job already.

The floor is a hard cut in the Related pane and on the graph's dashed edges. In
search it draws the line instead: the hits stay in the list, as the spec wants,
with the strangers below it.

**The divider** is engram's cliff, not the spec's fraction of the top score.
Over the cosine similarities sorted descending, at least three of them, the
largest gap must exceed `3.0 ×` the mean of the other gaps and `0.01 ×` the top
score. It reads similarity, never the fused score, whose first gap is
structurally the largest. The fall is carried back to the list as "after the
last hit that still reaches the cut", so what is marked is always a tail: a
fused list is not in score order, and reading it position by position would cut
away the good hits sitting below a weaker one. Everything from the fall on is
marked loose, kept in rank order, and shown smaller.

The line is the later of the two readings, so whichever leaves more hits
standing wins. The cliff alone put one hit above the line for *who frees the
memory* and called Borrowing and Smart pointers loose; the floor keeps them,
because a fall inside a list of neighbours is not the same thing as the edge of
what the vault knows.

**The model** is `multilingual-e5-small`, unquantized: fastembed 6.1 ships no
quantized e5-small, though the spec asks for one. A quantized ONNX still loads
through *Embedding: choose the model folder*, which exists for machines with no
network. The `query: ` and `passage: ` prefixes are added here, because
fastembed does not add them.

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
Opening a note bumps its activation and associates it with every note reached
in the same sitting within the window — one row per note however many times it
was reached, and the shared query, when there is one, becomes the cue.

Priming is rank-based, decided against the original order in one pass; rows 0
and 1 never move, and only hits above the list's median activation may climb.
Deciding as the list moves would let a row borrow the gap another row's move
opened. engram subtracts a decayed baseline from activation; here a note starts
with no row at all, so there is none.

`activation` and `assoc` have no foreign key to `notes`: a note deleted and
restored keeps what it learned, and queries join `notes` so stale rows never
surface. A rename moves the rows. A **schema bump drops them** with the rest of
the index — memory is the one derived thing the files cannot rebuild, and the
spec's "a schema change rebuilds rather than migrates" wins anyway.

`memory.enabled` off records no events and bumps nothing; priming and spread
are skipped and the Related pane shows *similar* only. What was learned is
kept, and *Memory: forget everything learned* empties it.

## Semantic edges

The graph draws associations and the top-k nearest notes as dashed edges,
thicker where the tie is stronger, and never twice over a pair an explicit link
already draws. Off by default in the global graph, on in the local one; each
graph keeps its own switch in `graph.json`.

## Where search lives

The spec's *Layout* line puts search in the left sidebar. It is reached by
Ctrl+K and by the ribbon's magnifier instead, on the same modal as the command
palette and the quick switcher, so the three behave alike; the sidebar holds
the file explorer alone and hands its actions to the header band, as Obsidian
does with a single-view sidebar. Nothing about what search *does* changed.

## The graph's renderer

Obsidian draws its graph with WebGL — `pixi.min.js` ships inside
`obsidian.asar`. Ours is a 2D canvas, which the spec does not speak to. Rather
than match the renderer, the roughness the user saw was traced to four things
and each was fixed: a hit target smaller than the node (`hitRadius`, at least
ten screen pixels), a guessed opening zoom that left the layout a knot
(`fitView`, run once the simulation settles), a zoom that jumped a whole notch
per wheel event (eased over frames toward a target), and no way to send a node
to the other split (Ctrl-click, through `openInOtherPane`). **A small graph
still leaves margin around itself: `fitView` caps the zoom at 2 so six notes do
not blow up to fill a pane.**
