# Importing a Logseq graph

*Import: Logseq graph* in the command palette asks for two folders: the
Logseq graph, which has to be outside this vault, and the folder in this
vault to import into (the vault root is fine). The graph is only read;
nothing in it is written, moved or deleted.
When it is done, `import-report.md` opens from the destination and names
every file and line the importer kept as text or left out. Read it once; it
is the whole list of what did not map. An import that stops on an error opens
its report too: the error carries the path to it.

Nothing already in the vault is overwritten, the report included. A page
whose target exists is skipped and reported, so running the import twice
writes nothing the second time. Where `import-report.md` is taken — by a page
of the graph, by an earlier import's report, or by a note of the user's own —
the report becomes `import-report-1.md`, and so on.

## Where things go

| In the graph | In the vault |
| --- | --- |
| `pages/Name.md` | `<destination>/Name.md` |
| `pages/a___b.md` (a namespaced page) | `<destination>/a/b.md` |
| a file name that is not one Obsidian can use | the nearest name that is, reported; it never leaves the destination |
| two pages whose file names differ only in case | both are imported, and reported: a filesystem that ignores case keeps only one |
| `journals/2026_09_14.md` | the daily-notes folder, named by the daily-note format: `Daily/2026-09-14.md` |
| `journals/<not a date>.md` | `<destination>/journals/<name>.md`, reported |
| `assets/**` | `<destination>/assets/**`, copied |
| `logseq/`, `bak/`, `version-files/` | ignored |
| anything else (`whiteboards/*.edn`, `.org` pages) | not imported, reported |

Journals go to the daily-notes folder rather than under the destination
because a journal is a daily note, and a daily note lives where *Open today's
daily note* looks.

## What changes inside a page

| Logseq | engram-notes |
| --- | --- |
| `key:: value` lines at the top of a page | YAML frontmatter. `tags::` and `alias::` become lists (`alias` is written as `aliases`), with `[[ ]]` and `#` stripped from each entry; `true`, `false` and numbers are typed, but only where the number writes back as it was written, so `0012345` and `1.10` stay text; everything else is text as written. |
| the same property twice | two lists are one list; any other repeat keeps the first value and is reported |
| `title:: Other Name` | the file is `Other Name.md`, and `title` leaves the frontmatter. A title that cannot be a file name stays a property, is added to `aliases` so links written with it still resolve, and is reported. |
| `id:: <uuid>` under a block | `^<anchor>` at the end of the block's last line, where Obsidian reads it, eight characters of the uuid (more when the page already uses them). A block that ends in a code block can carry no anchor and is reported. |
| `id:: <uuid>` on the page | the page's name is its key in the vault, so `((uuid))` becomes `[[Page]]` and the property leaves the frontmatter |
| `((uuid))` | `[[Page#^anchor]]`; the page is named by its file name, or by its path when another imported page or a note already in the vault shares that name. |
| the same `id::` on two blocks | the first block gets the anchor and every reference points at it; the second is reported |
| `{{embed ((uuid))}}` | `![[Page#^anchor]]` |
| `{{embed [[Page]]}}` | `![[Page]]` |
| `collapsed:: true` | dropped |
| any other `key:: value` under a block | kept as text, reported (Obsidian has no block properties), though a reference in its value is rewritten like any other. A property needs a space after `::`, so `std::mem::take(x)` in a line is prose and is left alone. |
| `TODO x`, `DONE x` | `[ ] x`, `[x] x` |
| `DOING x`, `LATER x`, `NOW x`, `WAITING x` | `[ ] DOING x` and so on, the word kept |
| `CANCELED x` | kept as text, reported |
| `[#A]`, `SCHEDULED:`, `DEADLINE:`, `:LOGBOOK:` | kept as text, reported |
| `#[[multi word]]` | `[[multi word]]` (a Logseq tag is a page reference); `#single` stays a tag |
| `![x](../assets/pic.png){:height 200, :width 300}` | `![x\|300x200](../assets/pic.png)`; the path is left alone, since it resolves by name |
| tab indentation | the vault's indent width in spaces; bullets stay bullets, so a page that is one list stays one list. The graph is read in Logseq's own unit, so the vault's setting cannot change where a block begins or end a code fence early. |
| a reference to a uuid the graph does not define | kept as text, reported |

The importer's output is its own fixed point: importing an imported folder
again produces the same files.
