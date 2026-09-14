<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app } from "./lib/state.svelte";
  import { allCommands, chord } from "./lib/commands";
  import { errorMessage } from "./lib/api";
  import { fileKind } from "./lib/files";
  import { bandDrag, edgeCursor, resizeEdge } from "./lib/frame";
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

  const headerHeight = () =>
    parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--header-height")) || 40;

  // The band has no overlay of its own: one that covered it swallowed every
  // click on the tab strip, the explorer's actions and the sidebar's icons.
  // A press in the band drags the window only when it landed on no control.
  const onBand = (e: PointerEvent | MouseEvent) => bandDrag(e.target as Element | null, e.clientY, headerHeight());

  const edgeAt = (e: PointerEvent | MouseEvent) => resizeEdge(e.clientX, e.clientY, innerWidth, innerHeight);

  // Undecorated windows get no resize border from the compositor, so the edges
  // are ours too. An edge outranks the band: the top few pixels resize.
  function framePress(e: PointerEvent) {
    if (e.button !== 0) return;
    const edge = edgeAt(e);
    if (edge) {
      e.preventDefault();
      void getCurrentWindow().startResizeDragging(edge);
    } else if (onBand(e)) {
      void getCurrentWindow().startDragging();
    }
  }

  function frameCursor(e: PointerEvent) {
    const cursor = edgeCursor(edgeAt(e));
    if (document.documentElement.dataset.cursor !== cursor) document.documentElement.dataset.cursor = cursor;
  }

  function maximizeFromBand(e: MouseEvent) {
    if (onBand(e)) void getCurrentWindow().toggleMaximize();
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
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="layout"
    class:no-left={!app.showLeft}
    class:no-right={!app.showRight}
    onpointerdown={framePress}
    onpointermove={frameCursor}
    ondblclick={maximizeFromBand}
  >
    <Ribbon />
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
          <button class:active={app.rightPane === "note"} title="Links" onclick={() => (app.rightPane = "note")}><ArrowLeftRight size={18} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "props"} title="Properties" onclick={() => (app.rightPane = "props")}><ListIcon size={18} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "all"} title="All properties" onclick={() => (app.rightPane = "all")}><LayoutList size={18} strokeWidth={1.75} /></button>
          <button class:active={app.rightPane === "related"} title="Related" onclick={() => (app.rightPane = "related")}><CircleDot size={18} strokeWidth={1.75} /></button>
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
