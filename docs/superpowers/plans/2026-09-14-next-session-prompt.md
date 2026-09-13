# Prompt for the next session

Paste everything below the line into a fresh session, on any machine. It
assumes nothing but a clone of this repository.

---

Read, in order: `AGENTS.md`,
`docs/superpowers/plans/2026-09-13-handoff-3.md`, and `docs/memory.md`. The
handoff is the source of truth for where the work stands and has a *Setting up
on another machine* section — follow it if this machine has not built the
project before. The spec is
`docs/superpowers/specs/2026-09-12-engram-notes-design.md`. The older handoffs
(`2026-09-13-handoff.md`, `-2.md`) are superseded; skim the first one's
*Gotchas* only.

Check out `feat/obsidian-parity-ui` — it is twenty-five commits ahead of `master`,
pushed, and unmerged. Continue on it rather than branching again.

Note that `AGENTS.md` changed at the end of the last session: the old "no
abstraction for a future that has not arrived" rule is gone. This is meant to
be a sustainable product that beats Obsidian, so prefer the shape that still
holds at ten times the vault, and build a seam a known roadmap item needs
before that work starts. An abstraction still has to name the real case it
serves.

Do these in order.

1. **Confirm last session's last build.** Two changes shipped without my eyes
   on them: the graph zoom, and a plain click on a graph node. Build the real
   window — `cd ui && pnpm tauri build --debug --no-bundle`, then
   `target/debug/engram-notes ~/engram-demo-vault` — and ask me for
   screenshots. A wheel gesture over the graph should zoom smoothly, and
   clicking a node should open that note in the other split rather than over
   the graph.

2. **Ask me whether to merge `feat/obsidian-parity-ui` locally, open a pull
   request, or keep it.** It has been open for two sessions. Ask before
   starting the next piece of work, not after.

3. **Write and execute a plan for the graph's WebGL renderer**, with
   `superpowers:writing-plans` and `superpowers:executing-plans`. What was
   already agreed, so do not re-litigate it: hand-written WebGL2, not PixiJS —
   nodes as instanced circles in one draw call, edges as instanced line quads
   in another, labels left on a 2D overlay canvas; the existing `paint()` in
   `GraphView.svelte` becomes the fallback for machines without WebGL2. Tell me
   before the plan departs from the spec. Be honest in the plan about what this
   does and does not buy: it is for scale, not for feel, and the force
   simulation stays on the CPU in d3, so at very large vaults the simulation
   becomes the limit rather than the renderer — if that needs solving too, say
   so and treat it as separate work.

Ground rules, which have mattered every session:

- **Match Obsidian by measuring it, not by impression.** Obsidian 1.13.7 is
  installed as a flatpak; extract `app.css` and `app.js` from its
  `obsidian.asar` with the script in
  `docs/superpowers/plans/2026-09-13-obsidian-parity-ui-2.md` and read the real
  numbers. Guessing has produced a worse interface twice.
- Tests stay free of models, windows and network.
- Verify every UI change with `ui/scripts/shot.mjs` in **both** themes before
  calling it done, then build the real window and ask me for screenshots.
  **The headless check has missed every defect that actually mattered** —
  every note being "similar" to every note, an interface smaller than
  Obsidian's, a zoom that never moved. All three passed a green suite and a
  clean screenshot, and all three were obvious the moment I looked at the real
  window beside the real Obsidian.
- Tell me before a plan departs from the spec, and record the departure in
  `docs/memory.md`.
- When the session ends, commit and push what exists, update the handoff and
  write the next prompt.
