<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, forgetMemory, openExternal, setConfig, setModelDir } from "../lib/api";
  import { toggleSnippet } from "../lib/snippets";
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

  // A number input hands back anything that was typed, min and max included,
  // and an indent of zero is one the editor cannot build.
  function saveIndent() {
    if (!cfg) return;
    const n = cfg.editor.indent;
    cfg.editor.indent = Number.isFinite(n) ? Math.min(8, Math.max(1, Math.round(n))) : 2;
    return save();
  }

  function setSnippet(name: string, on: boolean) {
    if (!cfg) return;
    cfg.css_snippets = toggleSnippet(cfg.css_snippets, name, on);
    return save();
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
      <div class="hint">
        <span>CSS snippets in <code>.engram-notes/snippets</code></span>
        <span>
          <button class="pick" onclick={() => app.loadSnippets()}>Reload</button>
          <button class="pick" onclick={() => openExternal(".engram-notes/snippets").catch((e) => app.say(errorMessage(e)))}>Open folder</button>
        </span>
      </div>
      {#each app.snippets as s (s.name)}
        <label>{s.name}
          <input type="checkbox" disabled={!!s.error} checked={cfg.css_snippets.includes(s.name)} onchange={(e) => setSnippet(s.name, e.currentTarget.checked)} />
        </label>
        {#if s.error}<div class="hint muted">{s.error}</div>{/if}
      {:else}
        <div class="hint muted">No snippets yet. Drop a <code>.css</code> file in the folder and reload.</div>
      {/each}

      <div class="pane-title">Editor</div>
      <label>Default mode
        <select bind:value={cfg.editor.default_mode} onchange={save}>
          <option value="live">Live preview</option>
          <option value="source">Source</option>
          <option value="reading">Reading</option>
        </select>
      </label>
      <label>Indent width
        <input type="number" min="1" max="8" step="1" bind:value={cfg.editor.indent} onchange={saveIndent} />
      </label>

      <div class="pane-title">Daily notes</div>
      <label>Folder <input bind:value={cfg.daily_notes.folder} onchange={save} /></label>
      <label>Date format <input bind:value={cfg.daily_notes.format} onchange={save} /></label>
      <label>Template <input bind:value={cfg.daily_notes.template} placeholder="Templates/Daily.md" onchange={() => { if (cfg.daily_notes.template === "") cfg.daily_notes.template = null; return save(); }} /></label>

      <div class="pane-title">Templates</div>
      <label>Folder <input bind:value={cfg.templates.folder} onchange={save} /></label>
      <label>Date format <input bind:value={cfg.templates.date_format} onchange={save} /></label>
      <label>Time format <input bind:value={cfg.templates.time_format} onchange={save} /></label>
      <div class="hint muted">Filled into <code>{"{{date}}"}</code>, <code>{"{{time}}"}</code> and <code>{"{{title}}"}</code>; <code>{"{{date:YYYY-MM-DD}}"}</code> picks its own.</div>

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
