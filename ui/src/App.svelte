<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app } from "./lib/state.svelte";
  import { allCommands, chord } from "./lib/commands";
  import { errorMessage } from "./lib/api";
  import { fileKind } from "./lib/files";
  import VaultPicker from "./components/VaultPicker.svelte";
  import Ribbon from "./components/Ribbon.svelte";
  import Explorer from "./components/Explorer.svelte";
  import Workspace from "./components/Workspace.svelte";
  import Backlinks from "./components/Backlinks.svelte";
  import Outgoing from "./components/Outgoing.svelte";
  import Properties from "./components/Properties.svelte";
  import Related from "./components/Related.svelte";
  import AllProperties from "./components/AllProperties.svelte";
  import Palette from "./components/Palette.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import Settings from "./components/Settings.svelte";
  import WindowControls from "./components/WindowControls.svelte";
  import ArrowLeftRight from "@lucide/svelte/icons/arrow-left-right";
  import ListIcon from "@lucide/svelte/icons/list";
  import LayoutList from "@lucide/svelte/icons/layout-list";
  import CircleDot from "@lucide/svelte/icons/circle-dot";

  $effect(() => {
    const t = app.config?.theme ?? "system";
    if (t === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  });

  $effect(() => {
    const p = app.activeTab?.path;
    if (p && fileKind(p) === "note") app.lastNote = p;
  });

  // Only a press on the band's own background drags; a press on a control does not.
  function dragWindow(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, input, a")) return;
    void getCurrentWindow().startDragging();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      app.palette = "none";
      app.settings = false;
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
    <Ribbon />
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="bandgrip" onpointerdown={dragWindow} ondblclick={() => getCurrentWindow().toggleMaximize()}></div>
    <WindowControls />
    <aside class="sidebar">
      {#if app.showLeft}<Explorer />{/if}
    </aside>
    <main class="centre">
      <Workspace node={app.layout} />
    </main>
    <aside class="sidebar right">
      {#if app.showRight}
        <div class="panestrip">
          <button class:active={app.rightPane === "note"} title="Links" onclick={() => (app.rightPane = "note")}><ArrowLeftRight size={15} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "props"} title="Properties" onclick={() => (app.rightPane = "props")}><ListIcon size={15} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "all"} title="All properties" onclick={() => (app.rightPane = "all")}><LayoutList size={15} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "related"} title="Related" onclick={() => (app.rightPane = "related")}><CircleDot size={15} strokeWidth={1.75} /></button>
        </div>
        {#if app.rightPane === "note"}<Backlinks /><Outgoing />
        {:else if app.rightPane === "props"}<Properties />
        {:else if app.rightPane === "all"}<AllProperties />
        {:else}<Related />{/if}
      {/if}
    </aside>
    <StatusBar />
  </div>
  <Palette />
  {#if app.settings}<Settings />{/if}
{/if}
{#if app.toast}<div class="toast">{app.toast}</div>{/if}
