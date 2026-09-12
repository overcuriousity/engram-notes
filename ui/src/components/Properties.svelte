<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  let props = $state<Record<string, unknown>>({});
  let newKey = $state("");
  $effect(() => {
    const p = app.activeTab?.path;
    void app.files;
    if (p) properties(p).then((r) => (props = r));
    else props = {};
  });
  function parse(v: string): unknown {
    if (v === "") return null;
    if (v === "true") return true;
    if (v === "false") return false;
    if (/^-?\d+(\.\d+)?$/.test(v)) return Number(v);
    if (v.startsWith("[")) {
      try { return JSON.parse(v); } catch { return v; }
    }
    return v;
  }
  async function commit(key: string, raw: string) {
    const doc = app.activeDoc;
    if (!doc) return;
    try {
      await app.save(doc); // a dirty buffer would otherwise conflict with the rewrite
      await setProperty(doc.path, key, parse(raw));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
  const show = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if app.activeDoc}
<div class="pane-title">Properties</div>
<div class="props">
  {#each Object.entries(props) as [k, v] (k)}
    <span>{k}</span>
    <input value={show(v)} onchange={(e) => commit(k, e.currentTarget.value)} />
  {/each}
  <input placeholder="new key" bind:value={newKey} />
  <input placeholder="value" onchange={(e) => { if (newKey) { void commit(newKey, e.currentTarget.value); newKey = ""; } }} />
</div>
{/if}
