# Smoke checklist

Run on a real vault before tagging a release. Every line is a yes or the tag waits.

1. Open a vault of at least 500 notes. The second open takes under two seconds.
2. The explorer shows folders collapsed and expanded, notes without `.md`.
3. Open a note. Live preview hides `#` on headings not under the cursor.
4. Type `[[`, pick a note; the link renders and a click opens it.
5. Click a link to a missing note: the note is created and opened.
6. Edit, wait one second, read the file in a terminal: the edit is there.
7. Edit the file in a terminal while the tab is clean: the tab reloads.
8. Edit the file in a terminal while the tab is dirty: the conflict bar appears and both choices work.
9. The backlinks pane lists referrers with line context; clicking opens.
10. Rename a note with three referrers: the dialog lists them, all three are rewritten.
11. Ctrl+Shift+F finds a word in a body and a word in a title; the title hit is first.
12. Ctrl+O, part of a title, Enter opens it; an unknown name offers Create.
13. Ctrl+D creates and opens today's daily note.
14. Edit a property in the right pane: the frontmatter in the editor updates.
15. Reading mode renders a callout; a checkbox toggles and writes to disk.
16. Dark and light themes are both readable in editor, reading view and sidebars.
17. Split right and split down from the palette: the note opens beside itself, typing in one pane shows in the other, dividers drag, and the layout survives a restart.
18. Closing the last tab of a pane closes the pane; the last pane stays.
19. Click an image and a PDF in the explorer: both preview; *Open in default app* opens them.
20. `[[Note#Heading]]` and `[[Note#^block]]` open the note scrolled to the line in live and reading mode.
21. List properties show as chips; adding and removing one rewrites the frontmatter.
22. Corrupt the index file while the app is closed: the next open says it rebuilt the index.
23. Right-click a folder: rename it in place; links with its path are rewritten and open tabs follow.
24. Delete a folder: it and its notes go to the trash and their tabs close.
25. Drag a note and a folder onto another folder: both move and links follow; dropping on empty space moves to the root.
26. New folder appears as Untitled, ready to type a name; an empty folder stays listed.
27. `![[pic.png|200]]` and `![caption](img/pic.png)` show images in live preview and reading view.
28. Ctrl+G opens the graph: hover highlights neighbours, click opens a note, wheel zooms, drag pans and moves nodes.
29. Graph filters (search with `tag:` and `path:`, tags, attachments, existing files only, orphans) and forces change the view and survive a restart in `.engram-notes/graph.json`.
30. Open local graph follows the note last active and its depth slider reaches three links.
31. Create new base, add a filter in its source, switch back to the table: rows match, a header click sorts and writes `sort`, a cell edit rewrites the note's frontmatter.
32. A base using `file.ctime` lists it as unsupported and shows no rows.
33. On first open the status bar says *downloading model*, then counts passages
    pending down to nothing; full-text search answers throughout.
34. Search a phrase no note contains word for word: semantic hits appear, the
    divider is drawn above the loose ones and they are smaller.
35. Open two notes from the same search, search again a minute later: the pair
    shows under *Associated* with the query as its cue.
36. Open one note ten times, search for something it matches weakly: it carries
    the *primed* badge and has climbed at most two places.
37. The Related pane lists associated, similar and suggested links; *link*
    inserts `[[Title]]` at the cursor.
38. Turn memory off in the status bar: *Associated* and the badges go, *Similar*
    stays. *Memory: forget everything learned* empties it and the pane with it.
39. Turn on semantic edges in the local graph: dashed edges appear, thicker
    where the association is stronger, and the global graph still has none.
40. *Embedding: choose the model folder* on a folder with the ONNX file and
    tokenizer loads it with no network, and the vectors are rebuilt.
41. The properties of a note are edited in a block above its text and scroll
    away with it; *+ Add property* adds one and it lands in the frontmatter.
42. Back and forward in the tab header walk the notes that tab has shown;
    following a link navigates in place, `+` and Ctrl-click open a tab.
43. The reading toggle in the tab header switches the note and back.
44. The ribbon's six icons run their commands and the gear opens settings;
    changing the theme there takes effect at once and survives a restart.
45. The right sidebar switches between links, properties, all properties with
    counts, and Related; the choice survives a restart.
46. The status bar shows backlinks, properties, words and characters for the
    open note.
47. The graph's nodes are small and pale and show no labels until zoomed out
    past the point where they fit.
48. `cargo test --workspace` and `pnpm check && pnpm test` pass.
49. Select a word and type `[[`: it becomes `[[word]]` with the word still
    selected; `*`, `` ` ``, `(` and `"` wrap the same way.
50. Enter at the end of a list item continues the list; Enter on an empty
    nested item outdents it; on an empty top-level item it removes the bullet.
51. Tab and Shift+Tab on an item with children move the whole subtree;
    Alt+Up/Down swap it with its sibling, and an ordered list stays in
    sequence afterwards.
52. Hover an item with children: a chevron appears in the gutter; click it to
    fold; close and reopen the note and it is still folded; rename the note
    and it stays folded.
53. With the `[[` popup open, Tab accepts the completion; Tab in prose inserts
    the indent width from settings.
54. Delete a note that had folded items and create a new one at the same path:
    it opens with nothing folded.
55. Put `Templates/Meeting.md` with `# {{title}}`, `{{date}} {{time}}` and
    `{{date:dddd}}` in the vault; *Templates: Insert template* in a note lists
    it and inserts it with the note's name, today, the time and the weekday.
56. Set the daily note template to that file: Ctrl+D on a fresh day opens a
    note whose heading is the date.
57. Drop `wide.css` with `:root { --file-line-width: 1000px; }` in
    `.engram-notes/snippets/`, reload in settings, switch it on: the line
    widens at once and is still wide after a restart; off narrows it.
58. A snippet selecting on `body.theme-dark` applies with the theme on
    *System* and the OS dark, and stops when the OS turns light.
59. `{{date:MMMM Do, YYYY}}` in a template inserts *September 14th, 2026*, and
    a token engram does not know, such as `NNN`, comes through as written.
60. A `.css` file the OS cannot read as text sits in settings with its error
    and its switch disabled; every other snippet keeps applying.
61. Select a sentence and click *link* in the Related pane: the link lands at
    the cursor and the sentence is still there. Insert a template with the
    same selection: the template takes its place.
62. Open one note in two panes side by side, put the cursor in each, and
    insert a template: it goes into the pane that was active, not the other.
63. *Import: Logseq graph* on a real graph: pick the graph, pick the vault
    root; the report opens, and the explorer shows the pages and the journals
    in the daily folder.
64. A page that had `id::` blocks: the block ends in `^…`, and a page that
    referenced it shows a link that opens the block.
65. `TODO` and `DONE` items show as checkboxes; a `DOING` item is an unchecked
    box with the word.
66. A namespaced page `a/b` is a folder `a` with `b.md`; `[[a/b]]` links to it.
67. Run the import a second time into the same folder: nothing is written,
    the first report is still there as it was, and the second one
    (`import-report-1.md`) lists every file as already in the vault.
68. Start the import again and pick a folder inside the vault as the graph:
    it is refused, and nothing is written.
69. In a note, type `[[carou` where a note about VAT fraud exists but none is
    named carousel: the popup lists the VAT note marked *meaning* after any
    spelled matches; Enter writes `[[VAT fraud chain]]`.
70. Select a sentence, press Ctrl+Shift+K: the picker lists notes; Enter on
    one lists its blocks with the best one preselected; Enter writes
    `[[Note#^id|the sentence]]` over the selection and the target's line ends
    in ` ^id`. Nothing else in the target changed (`git diff`).
71. Type `[[^^shell`: the picker opens with *shell* as the query; choosing a
    block replaces the typed `[[^^shell` with `[[Note#^id]]`, no alias.
72. In the picker choose a heading block: the link is `[[Note#Heading]]` and
    the target file is unchanged.
73. Link the same paragraph twice: the second link reuses the first `^id`
    and the file is not rewritten.
74. Rest the pointer on a passage link in live preview and in reading mode:
    after a moment a popover shows the note's title, the heading path and
    the passage; the `^id` appears nowhere. Moving off hides it.
75. Point *Embedding model folder* at a folder holding the fp32 model: the
    status shows `dir:<name>`, every passage re-embeds, and search still
    answers meanwhile by full text. Point it at an empty folder: the status
    shows the error, and `[[` completion still lists notes by spelling.
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
    Set it to a value the machine just misses at 20 pairs: `embed_status`
    reports a smaller `rerank_n` and reranking stays on.
80. Ctrl+Shift+K with a paraphrasing selection: the picker's first step
    lists the paraphrased note first.
81. Two notes with nothing in common still sit under the divider: the 0.83
    similarity floor holds for the int8 embedder, or the number that does is
    recorded in `docs/memory.md`.
