# Bases

A `.base` file is Obsidian's YAML. engram-notes reads the same files and
implements the table view over notes. What it does not support is listed in
the view as unsupported; a filter it cannot read shows no rows rather than a
guess.

## The file

    filters:            # an expression, or and / or / not holding a list
      and:
        - file.hasTag("book")
        - 'status != "done"'
    formulas:
      pages_left: "pages - read"
    properties:
      note.status:
        displayName: Status
    views:
      - type: table
        name: Reading
        filters:        # combined with the global filters by AND
          not:
            - file.inFolder("Archive")
        order: [file.name, note.status, formula.pages_left]
        sort:
          - property: note.status
            direction: ASC
        limit: 50

`not` holds when none of its filters hold. A view without `order` shows
`file.name`. Clicking a column header sorts by it and writes `sort` back to
the file. Note property cells edit the note's frontmatter; file fields and
formulas are read-only. Rows are notes; other files are not listed.

## Expressions

| Supported | Notes |
| --- | --- |
| `"text"`, `'text'`, `12`, `2.5`, `true`, `false`, `null` | |
| `+ - * / %`, `( )` | `+` joins text when either side is text |
| `== != > < >= <=` | a date compares with a string naming a date |
| `! && \|\|` | |
| `note.status`, `status`, `note["due date"]` | a missing property is `null` |
| `formula.name` | |
| `file.name`, `file.basename`, `file.path`, `file.folder`, `file.ext` | the vault root folder is `/` |
| `file.size`, `file.mtime` | |
| `file.tags` | with `#`, body and frontmatter tags |
| `file.links` | resolved note paths |
| `file.hasTag("a", "b")` | any of them, nested tags included |
| `file.inFolder("x")` | sub-folders included |
| `file.hasLink("Note")` | by path or name, as a link resolves |
| `file.hasProperty("x")` | |
| `date("2026-09-13")`, `date("2026-09-13 10:30")`, `now()`, `today()` | |
| `date + "1 week"`, `now() - "2d"` | units `y M w d h m s` and their words; `date - date` is milliseconds |
| `if(condition, then, else?)` | |
| `.contains(x)` | substring of text, element of a list |
| `.isEmpty()` | `null`, empty text or an empty list |

Not supported yet, and reported when used: every other function and method
(`link()`, `list()`, `.lower()`, `.format()`, `.map()` …), fields such as
`.length`, indexing with `[ ]` other than `note["…"]`, `this`, `file.ctime`,
`file.backlinks`, `file.embeds`, `file.properties`, views other than
`table`, `groupBy` and `summaries`.
