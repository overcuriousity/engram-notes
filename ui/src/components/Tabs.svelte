<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Pane } from "../lib/layout";
  let { pane }: { pane: Pane } = $props();
  const title = (p: string) => p.split("/").pop()!.replace(/\.md$/i, "");
  const dirty = (p: string) => {
    const d = app.docs[p];
    return !!d && d.text !== d.savedText;
  };
</script>

<div class="tabs">
  {#each pane.tabs as t, i (t.path)}
    <button class:active={i === pane.active} onclick={() => app.activate(pane.id, i)}>
      {title(t.path)}{dirty(t.path) ? " •" : ""}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.closeTab(pane.id, i); }} onkeydown={() => {}}>×</span>
    </button>
  {/each}
</div>
