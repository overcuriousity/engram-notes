<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allProperties, errorMessage, type PropertyCount } from "../lib/api";

  let rows = $state<PropertyCount[]>([]);
  $effect(() => {
    void app.files; // re-read whenever the index changed
    allProperties()
      .then((r) => (rows = r))
      .catch((e) => app.say(errorMessage(e)));
  });
</script>

<div class="pane-title">All properties</div>
{#each rows as r (r.key)}
  <div class="countrow"><span>{r.key}</span><span class="n">{r.count}</span></div>
{:else}
  <div class="pane-note">No properties in this vault.</div>
{/each}
