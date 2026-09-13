<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { parseValue } from "../lib/properties";
  import PropertyValue from "./PropertyValue.svelte";

  let { path }: { path: string } = $props();
  let values = $state<Record<string, unknown>>({});
  let adding = $state(false);
  let newKey = $state("");

  $effect(() => {
    const p = path;
    void app.files; // re-read whenever the index changed
    properties(p).then((r) => (values = r));
  });

  // null removes the key.
  async function commit(key: string, value: unknown) {
    const doc = app.docs[path];
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would conflict with the rewrite
      await setProperty(path, key, value);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  function add(raw: string) {
    if (!newKey) return;
    void commit(newKey, parseValue(raw) ?? "");
    newKey = "";
    adding = false;
  }
</script>

<div class="propblock">
  {#each Object.entries(values) as [k, v] (k)}
    <div class="prow">
      <span class="key" title={k}>{k}</span>
      <PropertyValue value={v} onCommit={(x) => commit(k, x)} />
      <button class="remove" title="Remove property" onclick={() => commit(k, null)}>×</button>
    </div>
  {/each}
  {#if adding}
    <div class="prow">
      <!-- svelte-ignore a11y_autofocus -->
      <input class="key" placeholder="Property" autofocus bind:value={newKey} />
      <input placeholder="Value" onchange={(e) => add(e.currentTarget.value)} />
      <button class="remove" onclick={() => (adding = false)}>×</button>
    </div>
  {:else}
    <button class="addprop" onclick={() => (adding = true)}>+ Add property</button>
  {/if}
</div>
