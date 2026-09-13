<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import Copy from "@lucide/svelte/icons/copy";
  import X from "@lucide/svelte/icons/x";

  const win = getCurrentWindow();
  let maximized = $state(false);
  // The frame is ours, so the restore glyph has to track the real state.
  $effect(() => {
    void win.isMaximized().then((m) => (maximized = m));
    const off = win.onResized(() => void win.isMaximized().then((m) => (maximized = m)));
    return () => void off.then((f) => f());
  });
</script>

<div class="wincontrols">
  <button class="icon" title="Minimise" aria-label="Minimise" onclick={() => win.minimize()}><Minus size={16} strokeWidth={2} /></button>
  <button class="icon" title={maximized ? "Restore" : "Maximise"} aria-label="Maximise" onclick={() => win.toggleMaximize()}>
    {#if maximized}<Copy size={14} strokeWidth={2} />{:else}<Square size={14} strokeWidth={2} />{/if}
  </button>
  <button class="icon close" title="Close" aria-label="Close" onclick={() => win.close()}><X size={17} strokeWidth={2} /></button>
</div>
