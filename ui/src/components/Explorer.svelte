<script lang="ts">
  import { ask, confirm } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/state.svelte";
  import { newNote } from "../lib/commands";
  import { createFolder, deleteFile, planRename, applyRename, errorMessage } from "../lib/api";

  interface Node { name: string; path: string; dirs: Node[]; files: { name: string; path: string }[] }
  interface Menu { x: number; y: number; path: string; isDir: boolean }

  let collapsed = $state<Record<string, boolean>>({});
  let menu = $state<Menu | null>(null);

  const tree = $derived.by((): Node => {
    const root: Node = { name: "", path: "", dirs: [], files: [] };
    for (const f of app.files) {
      const parts = f.path.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        let d = node.dirs.find((x) => x.name === parts[i]);
        if (!d) {
          d = { name: parts[i], path: parts.slice(0, i + 1).join("/"), dirs: [], files: [] };
          node.dirs.push(d);
        }
        node = d;
      }
      const name = parts[parts.length - 1];
      node.files.push({ name: f.is_markdown ? name.replace(/\.md$/i, "") : name, path: f.path });
    }
    const sort = (n: Node) => {
      n.dirs.sort((a, b) => a.name.localeCompare(b.name));
      n.files.sort((a, b) => a.name.localeCompare(b.name));
      n.dirs.forEach(sort);
    };
    sort(root);
    return root;
  });

  function openMenu(e: MouseEvent, path: string, isDir: boolean) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, path, isDir };
  }

  async function rename(path: string) {
    const to = window.prompt("New path", path);
    if (!to || to === path) return;
    try {
      const plan = await planRename(path, to);
      const ok =
        plan.affected.length === 0 ||
        (await ask(`Update links in ${plan.affected.length} file(s)?\n\n${plan.affected.join("\n")}`, { title: "Rename", kind: "info" }));
      if (!ok) return;
      await applyRename(plan);
      const tab = app.tabs.find((t) => t.path === path);
      if (tab) tab.path = plan.to;
      await app.refresh();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function remove(path: string) {
    if (!(await confirm(`Move "${path}" to the trash?`, { title: "Delete", kind: "warning" }))) return;
    try {
      await deleteFile(path);
      const i = app.tabs.findIndex((t) => t.path === path);
      if (i >= 0) app.closeTab(i);
      await app.refresh();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function newFolderIn(dir: string) {
    const name = window.prompt("Folder name", "New folder");
    if (!name) return;
    try {
      await createFolder(dir ? `${dir}/${name}` : name);
      await app.refresh();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function guarded(f: () => Promise<void>) {
    try {
      await f();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
</script>

<svelte:window onclick={() => (menu = null)} />

{#snippet dir(node: Node, depth: number)}
  {#each node.dirs as d (d.path)}
    <button
      style="padding-left:{8 + depth * 12}px;font-weight:500"
      onclick={() => (collapsed[d.path] = !collapsed[d.path])}
      oncontextmenu={(e) => openMenu(e, d.path, true)}
    >
      {collapsed[d.path] ? "▸" : "▾"} {d.name}
    </button>
    {#if !collapsed[d.path]}{@render dir(d, depth + 1)}{/if}
  {/each}
  {#each node.files as f (f.path)}
    <button
      style="padding-left:{20 + depth * 12}px"
      class:active={app.activeTab?.path === f.path}
      onclick={() => guarded(() => app.openNote(f.path))}
      oncontextmenu={(e) => openMenu(e, f.path, false)}
    >
      {f.name}
    </button>
  {/each}
{/snippet}

<div class="pane-title" style="display:flex;justify-content:space-between;align-items:center">
  <span>Files</span>
  <button title="New note" onclick={() => guarded(() => newNote())}>＋</button>
</div>
<div class="tree" oncontextmenu={(e) => { if (e.target === e.currentTarget) openMenu(e, "", true); }} role="tree" tabindex="-1">
  {@render dir(tree, 0)}
</div>

{#if menu}
  <div class="menu" style="left:{menu.x}px;top:{menu.y}px" role="menu" tabindex="-1">
    {#if menu.isDir}
      <button onclick={() => guarded(() => newNote(menu!.path))}>New note</button>
      <button onclick={() => newFolderIn(menu!.path)}>New folder</button>
    {:else}
      <button onclick={() => rename(menu!.path)}>Rename…</button>
      <button onclick={() => remove(menu!.path)}>Delete</button>
    {/if}
  </div>
{/if}
