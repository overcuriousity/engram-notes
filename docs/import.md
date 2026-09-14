# Importing a Logseq graph

*Import: Logseq graph* in the command palette asks for two folders: the
Logseq graph, and the folder in this vault to import into (the vault root is
fine). The graph is only read; nothing in it is written, moved or deleted.
When it is done, `import-report.md` opens from the destination and names
every file and line the importer kept as text or left out. Read it once; it
is the whole list of what did not map.

Nothing already in the vault is overwritten. A page whose target exists is
skipped and reported, so running the import twice writes nothing the second
time. The report is the one file the importer replaces, since it wrote it.

## Where things go

| In the graph | In the vault |
| --- | --- |
| `pages/Name.md` | `<destination>/Name.md` |
| `pages/a___b.md` (a namespaced page) | `<destination>/a/b.md` |
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
| `key:: value` lines at the top of a page | YAML frontmatter. `tags::` and `alias::` become lists (`alias` is written as `aliases`), with `[[ ]]` and `#` stripped from each entry; `true`, `false` and numbers are typed; everything else is text as written. |
| `title:: Other Name` | the file is `Other Name.md`, and `title` leaves the frontmatter. A title that cannot be a file name stays a property and is reported. |
| `id:: <uuid>` under a block | `^<anchor>` at the end of the block's first line, eight characters of the uuid (more when the page already uses them). |
| `((uuid))` | `[[Page#^anchor]]`; the page is named by its file name, or by its path when two imported pages share one. |
| `{{embed ((uuid))}}` | `![[Page#^anchor]]` |
| `{{embed [[Page]]}}` | `![[Page]]` |
| `collapsed:: true` | dropped |
| any other `key:: value` under a block | kept as text, reported (Obsidian has no block properties) |
| `TODO x`, `DONE x` | `[ ] x`, `[x] x` |
| `DOING x`, `LATER x`, `NOW x`, `WAITING x` | `[ ] DOING x` and so on, the word kept |
| `CANCELED x` | kept as text, reported |
| `[#A]`, `SCHEDULED:`, `DEADLINE:`, `:LOGBOOK:` | kept as text, reported |
| `#[[multi word]]` | `[[multi word]]` (a Logseq tag is a page reference); `#single` stays a tag |
| `![x](../assets/pic.png){:height 200, :width 300}` | `![x\|300x200](../assets/pic.png)`; the path is left alone, since it resolves by name |
| tab indentation | the vault's indent width in spaces; bullets stay bullets, so a page that is one list stays one list |
| a reference to a uuid the graph does not define | kept as text, reported |

The importer's output is its own fixed point: importing an imported folder
again produces the same files.
