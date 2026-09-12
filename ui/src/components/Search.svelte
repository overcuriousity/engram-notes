<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { search, type FtsHit } from "../lib/api";
  let q = $state("");
  let hits = $state<FtsHit[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      hits = query.trim() ? await search(query) : [];
    }, 150);
  });
</script>

<div class="pane-title">Search</div>
<div style="padding:0 12px 8px"><input style="width:100%" placeholder="Search…" bind:value={q} /></div>
{#each hits as h (h.path)}
  <button class="linkrow" onclick={() => app.openNote(h.path)}>
    <div class="src">{h.title}</div>
    <!-- escaped in core; only <mark> survives -->
    <div class="ctx">{@html h.snippet}</div>
  </button>
{/each}
