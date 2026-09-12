<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { createNote, errorMessage } from "../lib/api";
  import { createName, matchCommands, matchNotes } from "../lib/palette";

  interface Item { label: string; detail: string; run: () => void | Promise<void> }

  let q = $state("");
  let sel = $state(0);
  let input = $state<HTMLInputElement>();

  const items = $derived.by((): Item[] => {
    if (app.palette === "commands") {
      return matchCommands(allCommands(), q).map((c) => ({ label: c.name, detail: c.hotkey, run: c.run }));
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
      q = "";
      sel = 0;
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
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, items.length - 1); e.preventDefault(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter") { e.preventDefault(); void choose(sel); }
    else if (e.key === "Escape") { app.palette = "none"; }
    e.stopPropagation();
  }
</script>

{#if app.palette !== "none"}
  <div class="scrim" onclick={() => (app.palette = "none")} role="presentation">
    <div class="palette" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1" onkeydown={() => {}}>
      <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder={app.palette === "files" ? "Open note…" : "Run command…"} />
      <div class="items">
        {#each items as it, i (it.label + it.detail)}
          <button class:active={i === sel} onclick={() => choose(i)}><span>{it.label}</span><span class="detail">{it.detail}</span></button>
        {/each}
      </div>
    </div>
  </div>
{/if}
