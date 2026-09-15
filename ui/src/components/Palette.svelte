<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { createNote, errorMessage, recordEvent, renderTemplate, search, type SearchResults } from "../lib/api";
  import SearchResults_ from "./SearchResults.svelte";
  import { createName, matchCommands, matchNotes, matchTemplates } from "../lib/palette";
  import { templateTarget } from "../lib/commands";
  import LinkPicker from "./LinkPicker.svelte";

  interface Item { label: string; detail: string; run: () => void | Promise<void> }

  let q = $state("");
  let sel = $state(0);
  let input = $state<HTMLInputElement>();
  let found = $state<SearchResults>({ hits: [], associated: [] });
  let picker = $state<LinkPicker>();
  let linkRows = $state(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const rows = $derived(found.hits.length + found.associated.length);

  $effect(() => {
    if (app.palette !== "search") return;
    const query = q;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      found = query.trim() ? await search(query) : { hits: [], associated: [] };
      if (query.trim()) void recordEvent("search", undefined, query).catch(() => {});
    }, 150);
    return () => clearTimeout(timer);
  });

  // A search row opens its note; the other two modes run their item.
  function openHit(path: string, line?: number) {
    app.palette = "none";
    void app.openFromSearch(path, line, q).catch((e) => app.say(errorMessage(e)));
  }

  function chooseHit(i: number) {
    const hit = found.hits[i];
    if (hit) return openHit(hit.path, hit.line);
    const assoc = found.associated[i - found.hits.length];
    if (assoc) return openHit(assoc.path);
  }

  const items = $derived.by((): Item[] => {
    if (app.palette === "search") return [];
    if (app.palette === "commands") {
      return matchCommands(allCommands(), q).map((c) => ({ label: c.name, detail: c.hotkey, run: c.run }));
    }
    if (app.palette === "templates") {
      // The target is read when the item runs: the note may have changed since the picker opened.
      return matchTemplates(app.templates, app.config?.templates.folder ?? "", q).map((m) => ({
        label: m.title,
        detail: m.path,
        run: async () => {
          const into = templateTarget();
          if (!into) return app.say("Open a note in an editing mode to insert a template.");
          app.insertAtCursor(into.pane, into.path, await renderTemplate(m.path, into.path), true);
        },
      }));
    }
    const matches = matchNotes(app.titles, q);
    const notes: Item[] = matches.map((m) => ({ label: m.title, detail: m.path, run: () => app.openNote(m.path) }));
    const name = createName(matches, q);
    if (name) {
      notes.push({
        label: `Create "${name}"`,
        detail: `${name}.md`,
        run: async () => {
          await createNote(`${name}.md`);
          await app.refresh();
          await app.openNote(`${name}.md`);
        },
      });
    }
    return notes;
  });

  $effect(() => {
    if (app.palette !== "none") {
      q = app.palette === "link" ? (app.link?.query ?? "") : "";
      sel = 0;
      found = { hits: [], associated: [] };
      linkRows = 0;
      setTimeout(() => input?.focus());
    }
  });

  async function choose(i: number) {
    const it = items[i];
    app.palette = "none";
    if (!it) return;
    try {
      await it.run();
    } catch (e) {
      app.say(errorMessage(e));
    }
  }

  function onKey(e: KeyboardEvent) {
    const last = (app.palette === "search" ? rows : app.palette === "link" ? linkRows : items.length) - 1;
    // An empty list puts `last` at -1, and a negative sel makes Enter a no-op.
    if (e.key === "ArrowDown") { sel = Math.max(0, Math.min(sel + 1, last)); e.preventDefault(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter") {
      e.preventDefault();
      if (app.palette === "search") chooseHit(sel);
      else if (app.palette === "link") void picker?.choose(sel);
      else void choose(sel);
    }
    else if (e.key === "Escape") { if (!(app.palette === "link" && picker?.back())) app.palette = "none"; }
    e.stopPropagation();
  }
</script>

{#if app.palette !== "none"}
  <div class="scrim" onclick={() => (app.palette = "none")} role="presentation">
    <div class="palette" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1" onkeydown={() => {}}>
      <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder={app.palette === "files" ? "Open note…" : app.palette === "search" ? "Search the vault…" : app.palette === "templates" ? "Insert template…" : app.palette === "link" ? "Link to a passage…" : "Run command…"} />
      {#if app.palette === "search"}
        <SearchResults_ results={found} {sel} onOpen={openHit} />
      {:else if app.palette === "link"}
        <LinkPicker bind:this={picker} {q} {sel} onSel={(i) => (sel = i)} onCount={(n) => { linkRows = n; sel = Math.max(0, Math.min(sel, n - 1)); }} onDone={() => (app.palette = "none")} />
      {:else}
        <div class="items">
          {#each items as it, i (it.label + it.detail)}
            <button class:active={i === sel} onclick={() => choose(i)}><span>{it.label}</span><span class="detail">{it.detail}</span></button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}
