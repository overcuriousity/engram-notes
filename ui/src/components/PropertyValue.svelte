<script lang="ts">
  import { addItem, propKind, removeItem } from "../lib/properties";

  let { value, onCommit }: { value: unknown; onCommit: (v: unknown) => void } = $props();
  const kind = $derived(propKind(value));
  const list = $derived(Array.isArray(value) ? value : []);
  const text = (v: unknown) => (v == null ? "" : typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if kind === "checkbox"}
  <input type="checkbox" checked={value === true} onchange={(e) => onCommit(e.currentTarget.checked)} />
{:else if kind === "number"}
  <input type="number" value={value as number} onchange={(e) => onCommit(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
{:else if kind === "date"}
  <input type="date" value={value as string} onchange={(e) => onCommit(e.currentTarget.value || null)} />
{:else if kind === "datetime"}
  <input type="datetime-local" value={(value as string).slice(0, 16)} onchange={(e) => onCommit(e.currentTarget.value || null)} />
{:else if kind === "list"}
  <div class="chips">
    {#each list as item, i (i)}
      <span class="chip">{text(item)}<button title="Remove" onclick={() => onCommit(removeItem(list, i))}>×</button></span>
    {/each}
    <input
      class="chip-input"
      placeholder="Add"
      onkeydown={(e) => {
        if (e.key !== "Enter") return;
        onCommit(addItem(list, e.currentTarget.value));
        e.currentTarget.value = "";
      }}
    />
  </div>
{:else}
  <input value={text(value)} onchange={(e) => onCommit(e.currentTarget.value)} />
{/if}
