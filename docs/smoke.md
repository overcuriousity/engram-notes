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
17. `cargo test --workspace` and `pnpm check && pnpm test` pass.
