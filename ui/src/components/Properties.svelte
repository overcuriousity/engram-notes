<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { properties, setProperty, errorMessage } from "../lib/api";
  import { parseValue, propIcon } from "../lib/properties";
  import PropertyValue from "./PropertyValue.svelte";
  import Text from "@lucide/svelte/icons/text";
  import Binary from "@lucide/svelte/icons/binary";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Clock from "@lucide/svelte/icons/clock";
  import CheckSquare from "@lucide/svelte/icons/check-square";
  import ListIcon from "@lucide/svelte/icons/list";
  import Tags from "@lucide/svelte/icons/tags";
  import Forward from "@lucide/svelte/icons/forward";
  import type { PropIcon } from "../lib/properties";

  let props = $state<Record<string, unknown>>({});
  let newKey = $state("");

  const ICONS: Record<PropIcon, typeof Text> = {
    text: Text, binary: Binary, calendar: Calendar, clock: Clock,
    "check-square": CheckSquare, list: ListIcon, tags: Tags, forward: Forward,
  };
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
      {@const Icon = ICONS[propIcon(k, v)]}
      <span class="key" title={k}><Icon size={16} strokeWidth={2} /><span class="kname">{k}</span></span>
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
