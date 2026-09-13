<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { canBack, canForward } from "../lib/history";
  import { fileKind, tabTitle } from "../lib/files";
  import type { Pane } from "../lib/layout";

  let { pane }: { pane: Pane } = $props();
  const tab = $derived(pane.active >= 0 ? pane.tabs[pane.active] : null);
  const note = $derived(tab && fileKind(tab.path) === "note" ? tab : null);
  // Obsidian's one icon: reading offers a pencil, editing offers a book.
  const reading = $derived(note?.mode === "reading");

  function toggle() {
    if (!note) return;
    app.setMode(pane.id, note.path, reading ? "live" : "reading");
  }
</script>

<div class="tabheader">
  <button class="icon" title="Back" disabled={!tab || !canBack(tab)} onclick={() => app.step(pane.id, "back")}>‹</button>
  <button class="icon" title="Forward" disabled={!tab || !canForward(tab)} onclick={() => app.step(pane.id, "forward")}>›</button>
  <span class="title">{tab ? tabTitle(tab.path) : ""}</span>
  {#if note}
    <button class="icon" title={reading ? "Edit" : "Read"} onclick={toggle}>{reading ? "✎" : "▤"}</button>
  {/if}
</div>
