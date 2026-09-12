<script lang="ts">
  import { app } from "../lib/state.svelte";
  const words = $derived(app.activeTab ? app.activeTab.text.split(/\s+/).filter(Boolean).length : 0);
  const notes = $derived(app.files.filter((f) => f.is_markdown).length);
</script>

<div class="statusbar">
  <button onclick={() => (app.showLeft = !app.showLeft)} title="Toggle left sidebar">☰</button>
  <span>{notes} notes</span>
  {#if app.activeTab}<span>{words} words</span>{/if}
  <span style="flex:1"></span>
  <button onclick={() => (app.showRight = !app.showRight)} title="Toggle right sidebar">☰</button>
</div>
