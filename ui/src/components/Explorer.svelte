<script lang="ts">
  import { ask, confirm } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/state.svelte";
  import { newBase, newNote } from "../lib/commands";
  import { applyRename, createFolder, deleteFile, errorMessage, planRename } from "../lib/api";
  import { buildTree, dropTarget, fileTag, freeName, parent, renameTarget, type TreeDir } from "../lib/tree";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import ChevronsDownUp from "@lucide/svelte/icons/chevrons-down-up";

  interface Menu { x: number; y: number; path: string; isDir: boolean }

  let collapsed = $state<Record<string, boolean>>({});
  let menu = $state<Menu | null>(null);
  let editing = $state<string | null>(null);
  let dragging = $state<string | null>(null);
  let dropDir = $state<string | null>(null);

  const tree = $derived(buildTree(app.files, app.folders));

  function openMenu(e: MouseEvent, path: string, isDir: boolean) {
    e.preventDefault();
    e.stopPropagation();
    menu = { x: e.clientX, y: e.clientY, path, isDir };
  }

  async function guarded(f: () => Promise<void>) {
    try {
      await f();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  // Rename and drag end here; the dialog lists the files whose links change.
  async function move(from: string, to: string) {
    const plan = await planRename(from, to);
    if (plan.affected.length > 0) {
      const list = plan.affected.join("\n");
      if (!(await ask(`Update links in ${plan.affected.length} file(s)?\n\n${list}`, { title: "Move", kind: "info" }))) return;
    }
    // A pending autosave would otherwise write the old text over the rewrite.
    for (const d of Object.values(app.docs)) await app.save(d);
    await applyRename(plan);
    app.renamed(from, plan.to);
    await app.refresh();
  }

  function commitRename(path: string, isDir: boolean, name: string) {
    editing = null;
    return guarded(async () => {
      const to = renameTarget(path, name, !isDir && /\.md$/i.test(path));
      if (to) await move(path, to);
    });
  }

  function remove(path: string, isDir: boolean) {
    return guarded(async () => {
      const what = isDir ? `the folder "${path}" and everything in it` : `"${path}"`;
      if (!(await confirm(`Move ${what} to the trash?`, { title: "Delete", kind: "warning" }))) return;
      await deleteFile(path);
      app.forget(path);
      await app.refresh();
    });
  }

  // As in Obsidian, a new folder is called Untitled and opens for renaming.
  function newFolderIn(dir: string) {
    return guarded(async () => {
      const path = freeName(app.folders, dir, "Untitled");
      await createFolder(path);
      await app.refresh();
      collapsed[dir] = false;
      editing = path;
    });
  }

  function start(e: DragEvent, path: string) {
    // WebKitGTK starts a drag only when it carries data.
    e.dataTransfer?.setData("text/plain", path);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
    dragging = path;
  }

  function over(e: DragEvent, dir: string) {
    if (!dragging || dropTarget(dragging, dir) === null) return;
    e.preventDefault();
    e.stopPropagation();
    dropDir = dir;
  }

  function drop(e: DragEvent, dir: string) {
    e.preventDefault();
    e.stopPropagation();
    const src = dragging;
    dragging = dropDir = null;
    const to = src ? dropTarget(src, dir) : null;
    if (src && to) void guarded(() => move(src, to));
  }

  function end() {
    dragging = dropDir = null;
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  // Obsidian's collapse-all: every folder in the vault, not only the open ones.
  function collapseAll() {
    const next: Record<string, boolean> = {};
    for (const d of app.folders) next[d] = true;
    collapsed = next;
  }
</script>

<svelte:window onclick={() => (menu = null)} />

{#snippet renameBox(path: string, isDir: boolean, name: string, indent: number)}
  <input
    class="rename"
    style="margin-left:{indent}px;width:calc(100% - {indent + 8}px)"
    value={name}
    use:focusSelect
    onkeydown={(e) => {
      e.stopPropagation();
      if (e.key === "Enter") e.currentTarget.blur();
      if (e.key === "Escape") editing = null;
    }}
    onblur={(e) => {
      if (editing === path) void commitRename(path, isDir, e.currentTarget.value);
    }}
  />
{/snippet}

{#snippet dir(node: TreeDir, depth: number)}
  {#each node.dirs as d (d.path)}
    {#if editing === d.path}
      {@render renameBox(d.path, true, d.name, 24 + depth * 16)}
    {:else}
      <button
        class="folder"
        class:drop={dropDir === d.path}
        style="padding-left:{24 + depth * 16}px;--chev-left:{4 + depth * 16}px"
        draggable="true"
        ondragstart={(e) => start(e, d.path)}
        ondragend={end}
        ondragover={(e) => over(e, d.path)}
        ondrop={(e) => drop(e, d.path)}
        onclick={() => (collapsed[d.path] = !collapsed[d.path])}
        oncontextmenu={(e) => openMenu(e, d.path, true)}
      >
        <span class="chev" class:collapsed={collapsed[d.path]}><ChevronDown size={10} strokeWidth={4} /></span>
        <span class="tname">{d.name}</span>
      </button>
    {/if}
    {#if !collapsed[d.path]}{@render dir(d, depth + 1)}{/if}
  {/each}
  {#each node.files as f (f.path)}
    {#if editing === f.path}
      {@render renameBox(f.path, false, f.name, 24 + depth * 16)}
    {:else}
      <button
        style="padding-left:{24 + depth * 16}px"
        class:active={app.activeTab?.path === f.path}
        draggable="true"
        ondragstart={(e) => start(e, f.path)}
        ondragend={end}
        ondragover={(e) => over(e, parent(f.path))}
        ondrop={(e) => drop(e, parent(f.path))}
        onclick={(e) => guarded(() => app.openNote(f.path, undefined, "open", undefined, e.ctrlKey || e.metaKey))}
        oncontextmenu={(e) => openMenu(e, f.path, false)}
      >
        <span class="tname">{f.name}</span>
        {#if fileTag(f.path)}<span class="ftag">{fileTag(f.path)}</span>{/if}
      </button>
    {/if}
  {/each}
{/snippet}

<div class="explorer-head">
  <button class="icon" title="New note" aria-label="New note" onclick={() => guarded(() => newNote())}><FilePlus size={18} strokeWidth={1.75} /></button>
  <button class="icon" title="New folder" aria-label="New folder" onclick={() => newFolderIn("")}><FolderPlus size={18} strokeWidth={1.75} /></button>
  <button class="icon" title="Collapse all" aria-label="Collapse all" onclick={collapseAll}><ChevronsDownUp size={18} strokeWidth={1.75} /></button>
</div>
<div
  class="tree"
  class:drop={dropDir === ""}
  role="tree"
  tabindex="-1"
  oncontextmenu={(e) => openMenu(e, "", true)}
  ondragover={(e) => over(e, "")}
  ondrop={(e) => drop(e, "")}
>
  {@render dir(tree, 0)}
</div>

{#if menu}
  {@const m = menu}
  <div class="menu" style="left:{m.x}px;top:{m.y}px" role="menu" tabindex="-1">
    {#if m.isDir}
      <button onclick={() => guarded(() => newNote(m.path))}>New note</button>
      <button onclick={() => newFolderIn(m.path)}>New folder</button>
      <button onclick={() => guarded(() => newBase(m.path))}>New base</button>
    {/if}
    {#if m.path}
      <button onclick={() => (editing = m.path)}>Rename…</button>
      <button onclick={() => remove(m.path, m.isDir)}>Delete</button>
    {/if}
  </div>
{/if}
