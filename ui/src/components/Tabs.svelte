<script lang="ts">
  import { app } from "../lib/state.svelte";
  const title = (p: string) => p.split("/").pop()!.replace(/\.md$/i, "");
</script>

<div class="tabs">
  {#each app.tabs as t, i (t.path)}
    <button class:active={i === app.active} onclick={() => (app.active = i)}>
      {title(t.path)}{t.text !== t.savedText ? " •" : ""}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.closeTab(i); }} onkeydown={() => {}}>×</span>
    </button>
  {/each}
</div>
