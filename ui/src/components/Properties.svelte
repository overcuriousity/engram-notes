<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { parseValue } from "../lib/properties";
  import PropertyValue from "./PropertyValue.svelte";

  let props = $state<Record<string, unknown>>({});
  let newKey = $state("");
  $effect(() => {
    const p = app.activeDoc?.path;
    void app.files; // re-run whenever the index changed
    if (p) properties(p).then((r) => (props = r));
    else props = {};
  });

  // null removes the key.
  async function commit(key: string, value: unknown) {
    const doc = app.activeDoc;
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would otherwise conflict with the rewrite
      await setProperty(doc.path, key, value);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

</script>

{#if app.activeDoc}
  <div class="pane-title">Properties</div>
  <div class="props">
    {#each Object.entries(props) as [k, v] (k)}
      <span class="key" title={k}>{k}</span>
      <PropertyValue value={v} onCommit={(x) => commit(k, x)} />
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    {/each}
    <input placeholder="New property" bind:value={newKey} />
    <input
      placeholder="Value"
      onchange={(e) => {
        if (!newKey) return;
        void commit(newKey, parseValue(e.currentTarget.value) ?? "");
        newKey = "";
        e.currentTarget.value = "";
      }}
    />
    <span></span>
  </div>
{/if}
