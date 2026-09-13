<script lang="ts">
  import { app } from "./lib/state.svelte";
  import { allCommands, chord } from "./lib/commands";
  import { errorMessage } from "./lib/api";
  import { fileKind } from "./lib/files";
  import VaultPicker from "./components/VaultPicker.svelte";
  import Explorer from "./components/Explorer.svelte";
  import Search from "./components/Search.svelte";
  import Workspace from "./components/Workspace.svelte";
  import Backlinks from "./components/Backlinks.svelte";
  import Outgoing from "./components/Outgoing.svelte";
  import Properties from "./components/Properties.svelte";
  import Related from "./components/Related.svelte";
  import Palette from "./components/Palette.svelte";
  import StatusBar from "./components/StatusBar.svelte";

  $effect(() => {
    const t = app.config?.theme ?? "system";
    if (t === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  });

  $effect(() => {
    const p = app.activeTab?.path;
    if (p && fileKind(p) === "note") app.lastNote = p;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      app.palette = "none";
      return;
    }
    const cmd = allCommands().find((x) => x.hotkey === chord(e));
    if (!cmd) return;
    e.preventDefault();
    Promise.resolve(cmd.run()).catch((err) => app.say(errorMessage(err)));
  }
</script>

<svelte:window onkeydown={onKey} />

{#if !app.root}
  <VaultPicker />
{:else}
  <div class="layout" class:no-left={!app.showLeft} class:no-right={!app.showRight}>
    <aside class="sidebar">
      {#if app.showLeft}
        <div class="panestrip">
          <button class:active={app.leftPane === "files"} onclick={() => (app.leftPane = "files")}>Files</button>
          <button class:active={app.leftPane === "search"} onclick={() => (app.leftPane = "search")}>Search</button>
        </div>
        {#if app.leftPane === "files"}<Explorer />{:else}<Search />{/if}
      {/if}
    </aside>
    <main class="centre">
      <Workspace node={app.layout} />
    </main>
    <aside class="sidebar right">
      {#if app.showRight}<Backlinks /><Outgoing /><Properties /><Related />{/if}
    </aside>
    <StatusBar />
  </div>
  <Palette />
{/if}
{#if app.toast}<div class="toast">{app.toast}</div>{/if}
