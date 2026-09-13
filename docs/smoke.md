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
41. `cargo test --workspace` and `pnpm check && pnpm test` pass.
