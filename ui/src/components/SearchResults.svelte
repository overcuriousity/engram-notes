<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { SearchResults } from "../lib/api";

  let { results, sel, onOpen }: {
    results: SearchResults;
    sel: number;
    onOpen: (path: string, line?: number) => void;
  } = $props();

  // The first hit past the fall gets the divider above it.
  const divider = $derived(results.hits.findIndex((h) => h.past_divider));
</script>

<div class="items">
  {#each results.hits as h, i (h.path)}
    {#if i === divider}<div class="divider-line">loose</div>{/if}
    <button class="linkrow" class:active={i === sel} class:loose={h.past_divider} onclick={() => onOpen(h.path, h.line)}>
      <div class="src">
        {h.title}
        {#if h.primed}<span class="badge" title="you reach for this one">primed</span>{/if}
      </div>
      <!-- escaped in core; only <mark> survives -->
      <div class="ctx">{@html h.snippet}</div>
    </button>
  {/each}
  {#if results.associated.length}
    <div class="pane-title sub">Associated</div>
    {#each results.associated as a, j (a.path)}
      <button class="linkrow" class:active={results.hits.length + j === sel} onclick={() => onOpen(a.path)}>
        <div class="src">{a.title}</div>
        <div class="ctx dim">with {a.via}{a.cue ? ` · “${a.cue}”` : ""}</div>
      </button>
    {/each}
  {/if}
  {#if app.embed.state === "loading"}<div class="pane-note">downloading model…</div>{/if}
</div>
