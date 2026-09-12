<script lang="ts">
  import { onMount } from "svelte";
  import { app, type Tab } from "../lib/state.svelte";
  import { resolveLink, createNote, tags as apiTags, errorMessage } from "../lib/api";
  import Editor from "./Editor.svelte";
  import Reading from "./Reading.svelte";

  let { path }: { path: string } = $props();
  // Read from the store so edits mutate state the store owns.
  const tab = $derived(app.tabs.find((t) => t.path === path) as Tab);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let tagList = $state<string[]>([]);
  onMount(async () => {
    tagList = (await apiTags()).map((t) => t.tag);
  });

  function onChange(text: string) {
    tab.text = text;
    clearTimeout(timer);
    timer = setTimeout(() => app.save(tab), 500);
  }

  async function follow(target: string) {
    const [name] = target.split("#");
    try {
      let found = await resolveLink(name);
      if (!found) {
        found = `${name}.md`;
        await createNote(found);
        await app.refresh();
      }
      await app.openNote(found);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  function toggleTask(line: number) {
    const lines = tab.text.split("\n");
    if (line < 0 || line >= lines.length) return;
    lines[line] = lines[line].replace(/^(\s*[-*+]\s+)\[( |x|X)\]/, (_, p, c) => `${p}[${c === " " ? "x" : " "}]`);
    onChange(lines.join("\n"));
  }

  const modes = ["live", "source", "reading"] as const;
</script>

{#if tab.conflict}
  <div class="conflict">
    This file changed on disk while you had unsaved edits.
    <button onclick={() => app.resolveConflict(tab, false)}>Reload from disk</button>
    <button onclick={() => app.resolveConflict(tab, true)}>Keep mine</button>
  </div>
{/if}
<div class="modes">
  {#each modes as m (m)}
    <button class:active={tab.mode === m} onclick={() => (tab.mode = m)}>{m}</button>
  {/each}
</div>
<div class="note">
  {#if tab.mode === "reading"}
    <Reading text={tab.text} onFollow={follow} onToggleTask={toggleTask} />
  {:else}
    <Editor
      text={tab.text}
      mode={tab.mode}
      onchange={onChange}
      onblur={() => app.save(tab)}
      onFollow={follow}
      titles={() => app.titles}
      tags={() => tagList}
    />
  {/if}
</div>
