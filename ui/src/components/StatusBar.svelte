<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { backlinks, errorMessage, properties, setConfig } from "../lib/api";
  const words = $derived(app.activeDoc ? app.activeDoc.text.split(/\s+/).filter(Boolean).length : 0);
  const notes = $derived(app.files.filter((f) => f.is_markdown).length);
  const memory = $derived(app.config?.memory.enabled ?? false);
  const embed = $derived(app.embed);
  const chars = $derived(app.activeDoc?.text.length ?? 0);
  let links = $state(0);
  let propCount = $state(0);

  // Both are index reads: on the note and on the index, never on a keystroke.
  $effect(() => {
    const p = app.activeDoc?.path;
    void app.files;
    if (!p) {
      links = 0;
      propCount = 0;
      return;
    }
    backlinks(p).then((r) => (links = r.length)).catch(() => (links = 0));
    properties(p).then((r) => (propCount = Object.keys(r).length)).catch(() => (propCount = 0));
  });

  async function toggleMemory() {
    const cfg = app.config;
    if (!cfg) return;
    cfg.memory.enabled = !cfg.memory.enabled;
    try {
      await setConfig($state.snapshot(cfg));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
</script>

<div class="statusbar">
  <!-- Obsidian keeps every status item at the right edge. -->
  <span style="flex:1"></span>
  <button onclick={() => (app.showLeft = !app.showLeft)} title="Toggle left sidebar">☰</button>
  <span>{notes} notes</span>
  {#if app.activeDoc}
    <span>{links} backlinks</span>
    <span>{propCount} properties</span>
    <span>{words} words</span>
    <span>{chars} characters</span>
  {/if}
  {#if embed.state === "loading"}
    <span>downloading model</span>
  {:else if embed.state === "error"}
    <span title={embed.error ?? ""}>embedding off</span>
  {:else if embed.pending > 0}
    <span>{embed.pending} passages pending</span>
  {/if}
  <button onclick={toggleMemory} title={memory ? "Memory is on" : "Memory is off"}>
    {memory ? "memory on" : "memory off"}
  </button>
  <button onclick={() => (app.showRight = !app.showRight)} title="Toggle right sidebar">☰</button>
</div>
