<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { recordEvent, search, type SearchResults } from "../lib/api";
  let q = $state("");
  let out = $state<SearchResults>({ hits: [], associated: [] });
  let timer: ReturnType<typeof setTimeout> | undefined;
  // The first hit past the fall gets the divider above it.
  const divider = $derived(out.hits.findIndex((h) => h.past_divider));
  $effect(() => {
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      out = query.trim() ? await search(query) : { hits: [], associated: [] };
      if (query.trim()) void recordEvent("search", undefined, query).catch(() => {});
    }, 150);
  });
</script>

<div class="pane-title">Search</div>
<div style="padding:0 12px 8px"><input style="width:100%" placeholder="Search…" bind:value={q} /></div>
{#each out.hits as h, i (h.path)}
  {#if i === divider}<div class="divider-line">loose</div>{/if}
  <button class="linkrow" class:loose={h.past_divider} onclick={() => app.openFromSearch(h.path, h.line, q)}>
    <div class="src">
      {h.title}
      {#if h.heading}<span class="dim">— {h.heading}</span>{/if}
      {#if h.primed}<span class="badge" title="you reach for this one">primed</span>{/if}
    </div>
    <!-- escaped in core; only <mark> survives -->
    <div class="ctx">{@html h.snippet}</div>
  </button>
{/each}
{#if out.associated.length}
  <div class="pane-title sub">Associated</div>
  {#each out.associated as a (a.path)}
    <button class="linkrow" onclick={() => app.openFromSearch(a.path, undefined, q)}>
      <div class="src">{a.title}</div>
      <div class="ctx dim">with {a.via}{a.cue ? ` · “${a.cue}”` : ""}</div>
    </button>
  {/each}
{/if}
{#if app.embed.state === "loading"}<div class="pane-note">downloading model…</div>{/if}
