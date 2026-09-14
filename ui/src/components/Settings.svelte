<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, forgetMemory, setConfig, setModelDir } from "../lib/api";
  import { open } from "@tauri-apps/plugin-dialog";

  const cfg = $derived(app.config);

  // Every field writes app.json; the backend's copy is the only one that matters.
  async function save() {
    if (!cfg) return;
    try {
      await setConfig($state.snapshot(cfg));
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  async function pickModel() {
    const dir = await open({ directory: true });
    if (typeof dir !== "string" || !cfg) return;
    cfg.embed.model_dir = dir;
    await setModelDir(dir);
  }
</script>

{#if cfg}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (app.settings = false)}>
    <div class="settings" onclick={(e) => e.stopPropagation()}>
      <div class="pane-title">Appearance</div>
      <label>Theme
        <select bind:value={cfg.theme} onchange={save}>
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>

      <div class="pane-title">Editor</div>
      <label>Default mode
        <select bind:value={cfg.editor.default_mode} onchange={save}>
          <option value="live">Live preview</option>
          <option value="source">Source</option>
          <option value="reading">Reading</option>
        </select>
      </label>
      <label>Indent width
        <input type="number" min="1" max="8" step="1" bind:value={cfg.editor.indent} onchange={save} />
      </label>

      <div class="pane-title">Daily notes</div>
      <label>Folder <input bind:value={cfg.daily_notes.folder} onchange={save} /></label>
      <label>Date format <input bind:value={cfg.daily_notes.format} onchange={save} /></label>

      <div class="pane-title">Search and memory</div>
      <label>Memory <input type="checkbox" bind:checked={cfg.memory.enabled} onchange={save} /></label>
      <label>Similarity floor
        <input type="number" min="0" max="1" step="0.01" bind:value={cfg.search.similarity_floor} onchange={save} />
      </label>
      <label>Model folder
        <button class="pick" onclick={pickModel}>{cfg.embed.model_dir ?? "Downloaded"}</button>
      </label>
      <label>Learned links
        <button class="pick" onclick={async () => { await forgetMemory(); app.say("Memory forgotten."); }}>Forget everything</button>
      </label>

      <div class="row"><button onclick={() => (app.settings = false)}>Close</button></div>
    </div>
  </div>
{/if}
