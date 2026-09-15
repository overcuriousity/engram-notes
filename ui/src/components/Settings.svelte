<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { errorMessage, forgetMemory, openExternal, setConfig, setModelDir } from "../lib/api";
  import { toggleSnippet } from "../lib/snippets";
  import { open } from "@tauri-apps/plugin-dialog";
  import X from "@lucide/svelte/icons/x";

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

{#snippet row(name: string, desc: string | null, control: import("svelte").Snippet)}
  <div class="setting">
    <div class="info">
      <div class="name">{name}</div>
      {#if desc}<div class="desc">{desc}</div>{/if}
    </div>
    <div class="control">{@render control()}</div>
  </div>
{/snippet}

{#if cfg}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (app.settings = false)}>
    <div class="settings" onclick={(e) => e.stopPropagation()}>
      <div class="settings-head">
        <span>Settings</span>
        <button class="icon" title="Close" aria-label="Close" onclick={() => (app.settings = false)}><X size={18} strokeWidth={1.75} /></button>
      </div>
      <div class="settings-body">
        <h3>Appearance</h3>
        {#snippet theme()}
          <select bind:value={cfg.theme} onchange={save}>
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        {/snippet}
        {@render row("Theme", null, theme)}
        {#snippet snippetButtons()}
          <button class="pick" onclick={() => app.loadSnippets()}>Reload</button>
          <button class="pick" onclick={() => openExternal(".engram-notes/snippets").catch((e) => app.say(errorMessage(e)))}>Open folder</button>
        {/snippet}
        {@render row("CSS snippets", "Drop .css files in .engram-notes/snippets and reload; each one can be turned on here.", snippetButtons)}
        {#each app.snippets as s (s.name)}
          {#snippet toggle()}
            <input type="checkbox" disabled={!!s.error} checked={cfg.css_snippets.includes(s.name)} onchange={(e) => setSnippet(s.name, e.currentTarget.checked)} />
          {/snippet}
          {@render row(s.name, s.error ?? null, toggle)}
        {/each}

        <h3>Editor</h3>
        {#snippet mode()}
          <select bind:value={cfg.editor.default_mode} onchange={save}>
            <option value="live">Live preview</option>
            <option value="source">Source</option>
            <option value="reading">Reading</option>
          </select>
        {/snippet}
        {@render row("Default editing mode", "How a note opens.", mode)}
        {#snippet indent()}<input type="number" min="1" max="8" step="1" bind:value={cfg.editor.indent} onchange={saveIndent} />{/snippet}
        {@render row("Indent width", "Spaces per indentation level.", indent)}

        <h3>Daily notes</h3>
        {#snippet dailyFolder()}<input bind:value={cfg.daily_notes.folder} onchange={save} />{/snippet}
        {@render row("New file location", "Where daily notes are created.", dailyFolder)}
        {#snippet dailyFormat()}<input bind:value={cfg.daily_notes.format} onchange={save} />{/snippet}
        {@render row("Date format", "The file name, in strftime syntax.", dailyFormat)}
        {#snippet dailyTemplate()}
          <input bind:value={cfg.daily_notes.template} placeholder="Templates/Daily.md" onchange={() => { if (cfg.daily_notes.template === "") cfg.daily_notes.template = null; return save(); }} />
        {/snippet}
        {@render row("Template file location", "Filled into a new daily note.", dailyTemplate)}

        <h3>Templates</h3>
        {#snippet tplFolder()}<input bind:value={cfg.templates.folder} onchange={save} />{/snippet}
        {@render row("Template folder location", "Where the template picker looks.", tplFolder)}
        {#snippet tplDate()}<input bind:value={cfg.templates.date_format} onchange={save} />{/snippet}
        {@render row("Date format", "What {{date}} becomes; {{date:YYYY-MM-DD}} picks its own.", tplDate)}
        {#snippet tplTime()}<input bind:value={cfg.templates.time_format} onchange={save} />{/snippet}
        {@render row("Time format", "What {{time}} becomes.", tplTime)}

        <h3>Search and memory</h3>
        {#snippet memory()}<input type="checkbox" bind:checked={cfg.memory.enabled} onchange={save} />{/snippet}
        {@render row("Memory", "Learn from what is opened together and rank it higher.", memory)}
        {#snippet floor()}<input type="number" min="0" max="1" step="0.01" bind:value={cfg.search.similarity_floor} onchange={save} />{/snippet}
        {@render row("Similarity floor", "Below this cosine a semantic match is not shown.", floor)}
        {#snippet model()}<button class="pick" onclick={pickModel}>{cfg.embed.model_dir ?? "Downloaded"}</button>{/snippet}
        {@render row("Embedding model folder", "A local copy of the model; the default is downloaded.", model)}
        {#snippet forget()}<button class="pick" onclick={async () => { await forgetMemory(); app.say("Memory forgotten."); }}>Forget everything</button>{/snippet}
        {@render row("Learned links", "Drop everything memory has learned.", forget)}
      </div>
    </div>
  </div>
{/if}
