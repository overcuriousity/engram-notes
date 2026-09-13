<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, setConfig } from "../lib/api";
  const words = $derived(app.activeDoc ? app.activeDoc.text.split(/\s+/).filter(Boolean).length : 0);
  const notes = $derived(app.files.filter((f) => f.is_markdown).length);
  const memory = $derived(app.config?.memory.enabled ?? false);
  const embed = $derived(app.embed);

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
  <button onclick={() => (app.showLeft = !app.showLeft)} title="Toggle left sidebar">☰</button>
  <span>{notes} notes</span>
  {#if app.activeDoc}<span>{words} words</span>{/if}
  <span style="flex:1"></span>
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
