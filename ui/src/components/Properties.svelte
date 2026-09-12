<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { addItem, parseValue, propKind, removeItem } from "../lib/properties";

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

  const list = (v: unknown) => v as unknown[];
  const text = (v: unknown) => (v == null ? "" : typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if app.activeDoc}
  <div class="pane-title">Properties</div>
  <div class="props">
    {#each Object.entries(props) as [k, v] (k)}
      {@const kind = propKind(v)}
      <span class="key" title={k}>{k}</span>
      {#if kind === "checkbox"}
        <input type="checkbox" checked={v === true} onchange={(e) => commit(k, e.currentTarget.checked)} />
      {:else if kind === "number"}
        <input type="number" value={v as number} onchange={(e) => commit(k, e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
      {:else if kind === "date"}
        <input type="date" value={v as string} onchange={(e) => commit(k, e.currentTarget.value || null)} />
      {:else if kind === "datetime"}
        <input type="datetime-local" value={(v as string).slice(0, 16)} onchange={(e) => commit(k, e.currentTarget.value || null)} />
      {:else if kind === "list"}
        <div class="chips">
          {#each list(v) as item, i (i)}
            <span class="chip">{text(item)}<button title="Remove" onclick={() => commit(k, removeItem(list(v), i))}>×</button></span>
          {/each}
          <input
            class="chip-input"
            placeholder="Add"
            onkeydown={(e) => {
              if (e.key !== "Enter") return;
              void commit(k, addItem(list(v), e.currentTarget.value));
              e.currentTarget.value = "";
            }}
          />
        </div>
      {:else}
        <input value={text(v)} onchange={(e) => commit(k, e.currentTarget.value)} />
      {/if}
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
