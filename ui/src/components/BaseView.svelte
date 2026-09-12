<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "../lib/state.svelte";
  import { errorMessage, readNote, runBase, setBaseSort, setProperty, writeNote, type BaseTable, type Column } from "../lib/api";
  import { cellText, nextSort, sortMark } from "../lib/bases";
  import PropertyValue from "./PropertyValue.svelte";
  import YamlEditor from "./YamlEditor.svelte";

  let { path }: { path: string } = $props();
  let view = $state(0);
  let mode = $state<"table" | "source">("table");
  let table = $state.raw<BaseTable | null>(null);
  let failure = $state<string | null>(null);
  let source = $state("");
  let pending = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Bumped after this view writes, so the table reads the file again.
  let version = $state(0);

  const LINKS = ["file.name", "file.basename", "file.path"];
  const say = (e: unknown) => app.say(errorMessage(e));

  $effect(() => {
    void app.files; // the index changed
    void version;
    runBase(path, view)
      .then((t) => {
        table = t;
        failure = null;
      })
      .catch((e) => {
        table = null;
        failure = errorMessage(e);
      });
  });

  async function sortBy(c: Column) {
    if (!table) return;
    try {
      await setBaseSort(path, view, nextSort(table.sort, c.id));
      version++;
    } catch (e) {
      say(e);
    }
  }

  async function edit(note: string, c: Column, value: unknown) {
    try {
      const open = app.docs[note];
      if (open) await app.save(open); // a dirty buffer would conflict with the rewrite
      await setProperty(note, c.id.slice("note.".length), value);
      version++;
    } catch (e) {
      say(e);
    }
  }

  async function flush() {
    clearTimeout(timer);
    if (!pending) return;
    pending = false;
    await writeNote(path, source);
  }

  async function showSource() {
    try {
      source = (await readNote(path)).text;
      mode = "source";
    } catch (e) {
      say(e);
    }
  }

  async function showTable() {
    try {
      await flush();
      version++;
      mode = "table";
    } catch (e) {
      say(e);
    }
  }

  function onSource(text: string) {
    source = text;
    pending = true;
    clearTimeout(timer);
    timer = setTimeout(() => flush().catch(say), 500);
  }

  onDestroy(() => void flush().catch(say));
</script>

<div class="base">
  <div class="modes">
    <button class:active={mode === "table"} onclick={showTable}>table</button>
    <button class:active={mode === "source"} onclick={showSource}>source</button>
    {#if table && mode === "table"}
      {#if table.views.length > 1}
        <select bind:value={view}>
          {#each table.views as name, i (i)}<option value={i}>{name}</option>{/each}
        </select>
      {:else}
        <span class="base-view">{table.views[0]}</span>
      {/if}
      <span class="base-count">{table.rows.length} {table.rows.length === 1 ? "result" : "results"}</span>
    {/if}
  </div>
  {#if mode === "source"}
    <div class="base-source"><YamlEditor text={source} onchange={onSource} /></div>
  {:else}
    {#if failure || table?.errors.length}
      <div class="base-errors">
        {#if failure}<div>{failure}</div>{/if}
        {#each table?.errors ?? [] as e (e)}<div>{e}</div>{/each}
      </div>
    {/if}
    {#if table}
      <div class="base-scroll">
        <table>
          <thead>
            <tr>
              {#each table.columns as c (c.id)}
                <th><button onclick={() => sortBy(c)}>{c.label}{sortMark(table.sort, c.id)}</button></th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each table.rows as r (r.path)}
              <tr>
                {#each table.columns as c, i (c.id)}
                  <td>
                    {#if LINKS.includes(c.id)}
                      <button class="base-link" onclick={() => app.openNote(r.path).catch(say)}>{cellText(r.cells[i])}</button>
                    {:else if c.editable}
                      <PropertyValue value={r.cells[i]} onCommit={(v) => edit(r.path, c, v)} />
                    {:else}
                      {cellText(r.cells[i])}
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</div>
