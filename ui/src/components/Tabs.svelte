<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { tabTitle } from "../lib/files";
  import type { Pane } from "../lib/layout";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  let { pane }: { pane: Pane } = $props();
  const dirty = (p: string) => {
    const d = app.docs[p];
    return !!d && d.text !== d.savedText;
  };
</script>

<div class="tabs">
  {#each pane.tabs as t, i (t.path)}
    <button class:active={i === pane.active} onclick={() => app.activate(pane.id, i)}>
      {tabTitle(t.path)}{dirty(t.path) ? " •" : ""}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.closeTab(pane.id, i); }} onkeydown={() => {}}><X size={13} strokeWidth={2} /></span>
    </button>
  {/each}
  <button class="newtab icon" title="New tab" onclick={() => app.newTab()}><Plus size={15} strokeWidth={2} /></button>
</div>
