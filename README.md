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

That fetches the `latest` build — from the newest commit on `master` — verifies
its checksum and installs it to `~/.local/bin` (`ENGRAM_NOTES_BIN_DIR` moves it).

It installs the AppImage, which carries WebKitGTK with it and needs nothing but
glibc 2.39 or newer and libfuse2. A Tauri application never bundles the webview
into a bare executable: on Linux the webview *is* the system's WebKitGTK. If
this machine already has webkit2gtk 4.1 and GTK 3, `ENGRAM_NOTES_SLIM=1` takes
the 18 MB binary instead of the 100 MB AppImage.

    engram-notes ~/my-vault

On Windows, download `engram-notes-x86_64-pc-windows-msvc.zip` from
[the latest build](https://github.com/overcuriousity/engram-notes/releases/tag/latest)
and run `engram-notes.exe`. It is unsigned, so SmartScreen warns the first
time. macOS builds are planned; until then, build from source.

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
