<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/state.svelte";
  import { findPane } from "../lib/layout";
  import { resolveLink, createNote, tags as apiTags, errorMessage } from "../lib/api";
  import Editor from "./Editor.svelte";
  import Reading from "./Reading.svelte";

  let { paneId, path }: { paneId: number; path: string } = $props();
  // Read from the store so edits mutate state the store owns.
  const doc = $derived(app.docs[path]);
  const mode = $derived(findPane(app.layout, paneId)?.tabs.find((t) => t.path === path)?.mode ?? "live");
  let timer: ReturnType<typeof setTimeout> | undefined;
  let tagList = $state<string[]>([]);
  onMount(async () => {
    tagList = (await apiTags()).map((t) => t.tag);
  });

  // A second pane's editor echoes the change it was given; equal text is not an edit.
  function onChange(text: string) {
    if (text === doc.text) return;
    doc.text = text;
    clearTimeout(timer);
    const d = doc;
    timer = setTimeout(() => app.save(d), 500);
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
    const lines = doc.text.split("\n");
    if (line < 0 || line >= lines.length) return;
    lines[line] = lines[line].replace(/^(\s*[-*+]\s+)\[( |x|X)\]/, (_, p, c) => `${p}[${c === " " ? "x" : " "}]`);
    onChange(lines.join("\n"));
  }

  const modes = ["live", "source", "reading"] as const;
</script>

{#if doc}
  {#if doc.conflict}
    <div class="conflict">
      This file changed on disk while you had unsaved edits.
      <button onclick={() => app.resolveConflict(doc, false)}>Reload from disk</button>
      <button onclick={() => app.resolveConflict(doc, true)}>Keep mine</button>
    </div>
  {/if}
  <div class="modes">
    {#each modes as m (m)}
      <button class:active={mode === m} onclick={() => app.setMode(paneId, path, m)}>{m}</button>
    {/each}
  </div>
  <div class="note">
    {#if mode === "reading"}
      <Reading text={doc.text} onFollow={follow} onToggleTask={toggleTask} />
    {:else}
      <Editor
        text={doc.text}
        {mode}
        focus={app.activePane === paneId}
        onchange={onChange}
        onblur={() => app.save(doc)}
        onFollow={follow}
        titles={() => app.titles}
        tags={() => tagList}
      />
    {/if}
  </div>
{/if}
