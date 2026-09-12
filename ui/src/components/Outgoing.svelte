<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { outgoing, createNote, errorMessage, type LinkRow } from "../lib/api";
  let rows = $state<LinkRow[]>([]);
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files;
    if (p) outgoing(p).then((r) => (rows = r.filter((l) => l.kind !== "embed")));
    else rows = [];
  });
  async function open(r: LinkRow) {
    try {
      if (r.target_path) return await app.openNote(r.target_path);
      const path = `${r.target_raw}.md`;
      await createNote(path);
      await app.refresh();
      await app.openNote(path);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
</script>

<div class="pane-title">Outgoing links ({rows.length})</div>
{#each rows as r (r.line + r.target_raw)}
  <button class="linkrow" onclick={() => open(r)}>
    <div class="src">{r.target_raw}{r.target_path ? "" : " (not created)"}</div>
    <div class="ctx">{r.context}</div>
  </button>
{/each}
