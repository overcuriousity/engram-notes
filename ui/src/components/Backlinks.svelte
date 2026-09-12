<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { backlinks, type LinkRow } from "../lib/api";
  let rows = $state<LinkRow[]>([]);
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files; // re-run whenever the index changed
    if (p) backlinks(p).then((r) => (rows = r));
    else rows = [];
  });
</script>

<div class="pane-title">Backlinks ({rows.length})</div>
{#each rows as r (r.src_path + r.line)}
  <button class="linkrow" onclick={() => app.openNote(r.src_path)}>
    <div class="src">{r.src_path.replace(/\.md$/i, "")}</div>
    <div class="ctx">{r.context}</div>
  </button>
{/each}
