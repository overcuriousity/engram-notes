<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { resolveFile } from "../lib/files";
  import { app } from "../lib/state.svelte";
  import { findPane } from "../lib/layout";
  import { resolveLink, createNote, anchorLine, tags as apiTags, typing, errorMessage, getFolds, setFolds } from "../lib/api";
  import Editor from "./Editor.svelte";
  import Reading from "./Reading.svelte";
  import PropertiesBlock from "./PropertiesBlock.svelte";

  let { paneId, path }: { paneId: number; path: string } = $props();
  // The asset protocol is scoped to the open vault.
  const image = (target: string) => {
    const p = resolveFile(app.files, target);
    return p && app.root ? convertFileSrc(`${app.root}/${p}`) : null;
  };
  // Read from the store so edits mutate state the store owns.
  const doc = $derived(app.docs[path]);
  const mode = $derived(findPane(app.layout, paneId)?.tabs.find((t) => t.path === path)?.mode ?? "live");
  const jump = $derived(app.jump && app.jump.pane === paneId && app.jump.path === path ? app.jump : null);
  const insert = $derived(
    app.insertion && app.insertion.pane === paneId && app.insertion.path === path ? app.insertion : null,
  );
  const jumped = () => (app.jump = null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let tagList = $state<string[]>([]);
  let folds = $state<string[] | null>(null);
  let foldTimer: ReturnType<typeof setTimeout> | undefined;
  let foldSaid = false;
  // Folding is frequent and cosmetic; one write per pause is enough.
  function onFolds(keys: string[]) {
    clearTimeout(foldTimer);
    foldTimer = setTimeout(() => void setFolds(path, keys).catch(sayOnce), 500);
  }
  // A fold write fails once per pause for as long as the index is unavailable,
  // and the note keeps folding; one message says it without burying the rest.
  function sayOnce(e: unknown) {
    if (foldSaid) return;
    foldSaid = true;
    app.say(errorMessage(e));
  }
  onMount(async () => {
    tagList = (await apiTags()).map((t) => t.tag);
    try {
      folds = (await getFolds(path)) ?? [];
    } catch (e) {
      // The editor waits on this, so it opens unfolded rather than not at all.
      folds = [];
      sayOnce(e);
    }
  });

  // A second pane's editor echoes the change it was given; equal text is not an edit.
  function onChange(text: string) {
    if (text === doc.text) return;
    doc.text = text;
    // The embed queue steps aside while the user types.
    void typing().catch(() => {});
    clearTimeout(timer);
    const d = doc;
    timer = setTimeout(() => app.save(d), 500);
  }

  async function follow(target: string) {
    const hash = target.indexOf("#");
    const name = hash < 0 ? target : target.slice(0, hash);
    const fragment = hash < 0 ? "" : target.slice(hash + 1);
    try {
      // Notes resolve in the index; a base or attachment is found among the files, as Obsidian opens it.
      let found = (await resolveLink(name)) ?? resolveFile(app.files, name);
      if (!found) {
        found = `${name}.md`;
        await createNote(found);
        await app.refresh();
      }
      const line = fragment ? await anchorLine(found, fragment) : null;
      await app.openNote(found, line ?? undefined, "follow_link");
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

</script>

{#if doc}
  {#if doc.conflict}
    <div class="conflict">
      This file changed on disk while you had unsaved edits.
      <button onclick={() => app.resolveConflict(doc, false)}>Reload from disk</button>
      <button onclick={() => app.resolveConflict(doc, true)}>Keep mine</button>
    </div>
  {/if}
  <div class="note">
    <!-- Source mode shows the file as it is, frontmatter included. -->
    {#if mode !== "source"}<PropertiesBlock {path} />{/if}
    {#if mode === "reading"}
      <Reading text={doc.text} {image} {jump} onJumped={jumped} onFollow={follow} onToggleTask={toggleTask} />
    {:else}
      <Editor
        text={doc.text}
        {mode}
        focus={app.activePane === paneId}
        {jump}
        onJumped={jumped}
        {insert}
        onInserted={() => (app.insertion = null)}
        onchange={onChange}
        onblur={() => app.save(doc)}
        onFollow={follow}
        titles={() => app.titles}
        tags={() => tagList}
        {image}
        indent={app.config?.editor.indent ?? 2}
        {folds}
        {onFolds}
      />
    {/if}
  </div>
{/if}
