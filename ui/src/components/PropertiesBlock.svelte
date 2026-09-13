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

  let { path }: { path: string } = $props();
  let values = $state<Record<string, unknown>>({});
  let adding = $state(false);
  let newKey = $state("");

  const ICONS: Record<PropIcon, typeof Text> = {
    text: Text, binary: Binary, calendar: Calendar, clock: Clock,
    "check-square": CheckSquare, list: ListIcon, tags: Tags, forward: Forward,
  };

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
  <!-- Obsidian heads the block only once it holds something. -->
  {#if Object.keys(values).length > 0}<div class="propheading">Properties</div>{/if}
  {#each Object.entries(values) as [k, v] (k)}
    {@const Icon = ICONS[propIcon(k, v)]}
    <div class="prow">
      <span class="key" title={k}><Icon size={16} strokeWidth={2} /><span class="kname">{k}</span></span>
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
