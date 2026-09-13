<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { canBack, canForward } from "../lib/history";
  import { fileKind, tabTitle } from "../lib/files";
  import type { Pane } from "../lib/layout";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Pencil from "@lucide/svelte/icons/pencil";

  let { pane }: { pane: Pane } = $props();
  const tab = $derived(pane.active >= 0 ? pane.tabs[pane.active] : null);
  const note = $derived(tab && fileKind(tab.path) === "note" ? tab : null);
  // Obsidian's one icon: reading offers a pencil, editing offers a book.
  const reading = $derived(note?.mode === "reading");
  // Obsidian's breadcrumb: the folders, then the note.
  const crumbs = $derived(
    tab ? [...tab.path.split("/").slice(0, -1), tabTitle(tab.path)].filter(Boolean) : [],
  );

  function toggle() {
    if (!note) return;
    app.setMode(pane.id, note.path, reading ? "live" : "reading");
  }
</script>

<div class="tabheader">
  <button class="icon" title="Back" disabled={!tab || !canBack(tab)} onclick={() => app.step(pane.id, "back")}>
    <ChevronLeft size={18} strokeWidth={2} />
  </button>
  <button class="icon" title="Forward" disabled={!tab || !canForward(tab)} onclick={() => app.step(pane.id, "forward")}>
    <ChevronRight size={18} strokeWidth={2} />
  </button>
  <span class="title">
    {#each crumbs as c, i (i)}{#if i > 0}<span class="sep">/</span>{/if}{c}{/each}
  </span>
  {#if note}
    <button class="icon" title={reading ? "Edit" : "Read"} onclick={toggle}>
      {#if reading}<Pencil size={18} strokeWidth={1.75} />{:else}<BookOpen size={18} strokeWidth={1.75} />{/if}
    </button>
  {/if}
</div>
