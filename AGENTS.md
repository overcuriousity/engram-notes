# Working on engram-notes

Read `docs/superpowers/specs/2026-09-12-engram-notes-design.md` first. It is
the design; this file is how to work inside it.

## Comments

Short. A comment says why, not what, and only where the why is not obvious
from the code. One or two lines is the norm; a paragraph is rare and earns
its place by explaining a non-obvious constraint. No comment restates the
function name, no comment narrates control flow, no comment per field of a
struct whose fields are named well. Doc comments on public items are one
sentence unless the item has a contract worth stating. If a comment is
longer than the code it describes, cut it.

## Conventions

- Rust 2024 edition, stable toolchain, `cargo fmt` and `cargo clippy
  -D warnings` clean.
- `core` has no Tauri dependency and is where logic and tests live.
  `src-tauri` is a thin shell. The frontend never touches the filesystem.
- Files are the truth. Anything derived is rebuildable, and a schema bump
  rebuilds rather than migrates.
- KISS. One trait per seam, one implementation until a second exists. No
  abstraction for a future that has not arrived.
- Where a feature is Obsidian-inspired, match Obsidian's behaviour and file
  formats. Where it is engram-inspired, match engram's concepts.
- Tests run without a model, a window or the network. The embedder has a
  deterministic fake.
- Commit messages: conventional prefix (`feat`, `fix`, `docs`, `chore`,
  `test`, `ci`), imperative subject under 72 characters, body only when the
  why is not in the subject.
- Errors are typed in `core::Error`; the UI shows them, never swallows them.
