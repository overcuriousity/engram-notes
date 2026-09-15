<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/state.svelte";
  import { anchorBlock, blocks as fetchBlocks, errorMessage, linkCandidates, type Block, type LinkCandidate } from "../lib/api";
  import { firstLine, linkText } from "../lib/linkpicker";

  interface Props { q: string; sel: number; onSel: (i: number) => void; onDone: () => void; onCount: (n: number) => void }
  let { q, sel, onSel, onDone, onCount }: Props = $props();

  let notes = $state<LinkCandidate[]>([]);
  let chosen = $state<LinkCandidate | null>(null);
  let list = $state<Block[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  // A request in flight cannot be cancelled, only disowned: one that lands
  // after the user has picked a note would put the note count on screen while
  // the blocks are showing, and every row after that is the wrong row.
  let gen = 0;

  $effect(() => {
    if (chosen) return;
    const query = q;
    const mine = ++gen;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        const got = await linkCandidates(query);
        if (mine !== gen) return;
        notes = got;
      } catch (e) {
        if (mine !== gen) return;
        app.say(errorMessage(e));
      }
      onCount(notes.length);
    }, 80);
    return () => clearTimeout(timer);
  });

  function score(b: Block): number {
    const words = q.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((w) => w.length >= 2);
    const t = b.text.toLowerCase();
    return new Set(words.filter((w) => t.includes(w))).size;
  }

  export async function pickNote(i: number) {
    const c = notes[i];
    if (c) await showBlocks(c);
  }

  async function showBlocks(c: LinkCandidate) {
    // Blocks are read from disk, so an open buffer of the target goes there
    // first; otherwise the picker lists lines the user cannot see and the
    // anchor is written under their edit.
    if (!(await app.flush(c.path))) return;
    let found: Block[];
    try {
      found = await fetchBlocks(c.path);
    } catch (e) {
      return app.say(errorMessage(e));
    }
    // An empty note has nothing to link to; a second step listing nothing
    // would leave the user pressing Enter at a blank pane.
    if (!found.length) return app.say(`${c.title} has no passage to link to.`);
    // Only now: until the blocks are on screen a candidates request still
    // describes what is, and the paths above all leave the note list showing.
    gen++;
    list = found;
    chosen = c;
    // The block with the most of the query's words, ties to the earlier one; mirrors core's best_block.
    let best = 0, bestN = 0;
    list.forEach((b, j) => { const n = score(b); if (n > bestN) { best = j; bestN = n; } });
    onSel(best);
    onCount(list.length);
  }

  export async function pickBlock(i: number) {
    const b = list[i];
    const req = app.link;
    if (!b || !chosen || !req) return;
    if (b.kind !== "heading" && !(await app.flush(chosen.path))) return;
    try {
      let id: string | null = null;
      if (b.kind !== "heading") {
        id = await anchorBlock(chosen.path, b.first, b.last);
        // The anchor is on disk; the buffer takes it before the link goes in,
        // so the insert builds on the anchored text and not on what preceded it.
        await app.adopt(chosen.path);
        await tick();
      }
      app.insertAtCursor(req.pane, req.path, linkText(chosen.path, b, id, req.alias), req.replace);
      onDone();
    } catch (e) {
      app.say(errorMessage(e));
      // The block moved under the picker: show the note's blocks as they are now.
      if ((e as { code?: string })?.code === "not_found") await showBlocks(chosen);
    }
  }

  /** Escape in step two goes back to the notes; in step one it closes. */
  export function back(): boolean {
    if (!chosen) return false;
    chosen = null;
    onSel(0);
    onCount(notes.length);
    return true;
  }

  export function choose(i: number) {
    return chosen ? pickBlock(i) : pickNote(i);
  }
</script>

{#if chosen}
  <div class="crumb">{chosen.title}</div>
  <div class="items">
    {#each list as b, i (b.first)}
      <button class:active={i === sel} onclick={() => pickBlock(i)}>
        <span class="kind">{b.kind === "heading" ? "#" : b.kind === "list" ? "•" : "¶"}</span>
        <span>{firstLine(b)}</span>
        {#if b.text.includes("\n")}<span class="detail">{b.text.split("\n").slice(1).join(" ").slice(0, 80)}</span>{/if}
      </button>
    {/each}
  </div>
{:else}
  <div class="items">
    {#each notes as c, i (c.path)}
      <button class:active={i === sel} onclick={() => pickNote(i)}>
        <span>{c.title}</span>
        <span class="detail">{c.path}</span>
        {#if c.kind === "meaning"}<span class="mark">meaning</span>{/if}
        {#if c.primed}<span class="mark">primed</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .crumb { padding: 4px 12px; color: var(--fg-muted); font-size: 0.85em; }
  .kind { width: 1.2em; color: var(--fg-muted); }
  .mark { margin-left: auto; font-size: 0.75em; color: var(--fg-muted); border: 1px solid var(--border); border-radius: 3px; padding: 0 4px; }
</style>
