<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/state.svelte";
  import { allCommands, importLogseq } from "../lib/commands";
  import { errorMessage, recentVaults } from "../lib/api";
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import Search from "@lucide/svelte/icons/search";
  import PanelLeft from "@lucide/svelte/icons/panel-left";
  import GitFork from "@lucide/svelte/icons/git-fork";
  import CalendarDays from "@lucide/svelte/icons/calendar-days";
  import Command from "@lucide/svelte/icons/command";
  import Settings from "@lucide/svelte/icons/settings";
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
  import Vault from "@lucide/svelte/icons/vault";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Import from "@lucide/svelte/icons/import";

  // Obsidian's ribbon: the commands worth one click, in its order and its icons.
  const top = [
    { id: "new-note", icon: FilePlus, title: "New note" },
    { id: "switcher", icon: PanelLeft, title: "Quick switcher" },
    { id: "search", icon: Search, title: "Search" },
    { id: "graph", icon: GitFork, title: "Graph view" },
    { id: "daily", icon: CalendarDays, title: "Daily note" },
    { id: "palette", icon: Command, title: "Command palette" },
  ];

  // Obsidian's vault switcher is its own window; ours is a menu off the same button.
  let vaults = $state<{ x: number; y: number; recent: string[] } | null>(null);

  function run(id: string) {
    const c = allCommands().find((x) => x.id === id);
    if (!c) return;
    Promise.resolve(c.run()).catch((e) => app.say(errorMessage(e)));
  }

  async function openVaults(e: MouseEvent) {
    e.stopPropagation();
    const b = e.currentTarget as HTMLElement;
    const r = b.getBoundingClientRect();
    let recent: string[] = [];
    try {
      recent = await recentVaults();
    } catch (err) {
      app.say(errorMessage(err));
    }
    vaults = { x: r.right + 4, y: r.top, recent };
  }

  const name = (root: string) => root.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || root;

  async function switchTo(root: string) {
    vaults = null;
    if (root === app.root) return;
    try {
      await app.switchVault(root);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function pickFolder() {
    vaults = null;
    const dir = await open({ directory: true, multiple: false, title: "Open folder as vault" });
    if (typeof dir === "string") await switchTo(dir);
  }

  function runImport() {
    vaults = null;
    importLogseq().catch((e) => app.say(errorMessage(e)));
  }
</script>

<svelte:window onclick={() => (vaults = null)} />

<nav class="ribbon">
  <button class="icon corner" title="Toggle left sidebar" onclick={() => (app.showLeft = !app.showLeft)}>
    <PanelLeftClose size={20} strokeWidth={1.75} />
  </button>
  {#each top as t (t.id)}
    <button class="icon" title={t.title} onclick={() => run(t.id)}>
      <t.icon size={20} strokeWidth={1.75} />
    </button>
  {/each}
  <span class="spacer"></span>
  <button class="icon" title="Open another vault" onclick={openVaults}>
    <Vault size={20} strokeWidth={1.75} />
  </button>
  <button class="icon" title="Settings" onclick={() => (app.settings = true)}>
    <Settings size={20} strokeWidth={1.75} />
  </button>
</nav>

{#if vaults}
  {@const v = vaults}
  <div class="menu vaults" style="left:{v.x}px;bottom:{Math.max(8, innerHeight - v.y - 36)}px" role="menu" tabindex="-1">
    {#if v.recent.length}
      <div class="pane-title">Vaults</div>
      {#each v.recent as r (r)}
        <button class:active={r === app.root} title={r} onclick={() => switchTo(r)}>
          <span class="vname">{name(r)}</span><span class="vpath">{r}</span>
        </button>
      {/each}
      <hr />
    {/if}
    <button onclick={pickFolder}><FolderOpen size={15} strokeWidth={1.75} /> Open folder as vault…</button>
    <button onclick={runImport}><Import size={15} strokeWidth={1.75} /> Import Logseq graph…</button>
  </div>
{/if}
