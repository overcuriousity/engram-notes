<script lang="ts">
  import { app } from "../lib/state.svelte";
  import type { Pane } from "../lib/layout";
  import Tabs from "./Tabs.svelte";
  import NoteView from "./NoteView.svelte";
  import FileView from "./FileView.svelte";
  import GraphView from "./GraphView.svelte";
  import BaseView from "./BaseView.svelte";
  import { fileKind } from "../lib/files";

  let { pane }: { pane: Pane } = $props();
  const tab = $derived(pane.active >= 0 ? pane.tabs[pane.active] : null);
  const kind = $derived(tab ? fileKind(tab.path) : "other");
</script>

<!-- Pointer and focus only mark the active pane; the controls inside stay reachable. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<section
  class="pane"
  class:focused={app.activePane === pane.id && app.paneCount > 1}
  onfocusin={() => app.focusPane(pane.id)}
  onpointerdown={() => app.focusPane(pane.id)}
>
  <Tabs {pane} />
  {#if tab}
    {#key tab.path}
      {#if kind === "note"}
        <NoteView paneId={pane.id} path={tab.path} />
      {:else if kind === "graph"}
        <GraphView local={tab.path === "graph:local"} />
      {:else if kind === "base"}
        <BaseView path={tab.path} />
      {:else}
        <FileView path={tab.path} />
      {/if}
    {/key}
  {:else}
    <div class="note empty">No note open</div>
  {/if}
</section>
