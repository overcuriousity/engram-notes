# engram-notes

A markdown note taking application: a folder of files, links and backlinks,
a graph, database views over frontmatter, and search by meaning that
remembers what you reach for. One executable for Linux, Windows and macOS.

It takes the folder-of-markdown, links, graph and bases from Obsidian, the
open licence from Logseq, and the semantic search and memory concepts from
[engram](https://github.com/overcuriousity/engram), reimplemented locally so
nothing runs but the app.

Status: in development, no tagged release yet. See
[the design](docs/superpowers/specs/2026-09-12-engram-notes-design.md).

## Install

Linux, x86_64:

    curl -fsSL https://raw.githubusercontent.com/overcuriousity/engram-notes/master/install.sh | sh

That fetches the `latest` release — the binary built from the newest commit on
`master` — verifies its checksum and installs it to `~/.local/bin`. Set
`ENGRAM_NOTES_BIN_DIR` to install elsewhere. It needs webkit2gtk 4.1 and GTK 3
at runtime: `webkit2gtk4.1 gtk3` on Fedora, `libwebkit2gtk-4.1-0 libgtk-3-0` on
Debian and Ubuntu.

    engram-notes ~/my-vault

Windows and macOS builds are planned; until then, build from source.

## Build

Rust stable, Node 22, pnpm 11, and on Linux the webkit2gtk development
packages: `webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel
dbus-devel` on Fedora, `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev` on
Debian and Ubuntu.

    cd ui && pnpm install && pnpm tauri dev      # run
    cd ui && pnpm tauri build                    # installers under target/release/bundle
    cargo test --workspace                       # the core tests need no window

## Licence

GPL-3.0-only. See [LICENSE](LICENSE).
