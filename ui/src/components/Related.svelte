<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { templateTarget } from "../lib/commands";
  import { errorMessage, related, type Related } from "../lib/api";

  let data = $state<Related>({ associated: [], similar: [], suggested: [] });
  const path = $derived(app.lastNote);

  $effect(() => {
    const p = path;
    void app.files;
    if (!p) {
      data = { associated: [], similar: [], suggested: [] };
      return;
    }
    related(p)
      .then((r) => (data = r))
      .catch((e) => app.say(errorMessage(e)));
  });

  // A link goes in at the cursor and leaves any selection alone: this is a
  // sidebar button, not an editing command, and it should not eat a paragraph.
  function link(title: string) {
    const into = templateTarget();
    if (!into) return app.say("Open a note in an editing mode to insert a link.");
    app.insertAtCursor(into.pane, into.path, `[[${title}]]`);
  }
</script>

<div class="pane-title">Related</div>
{#if data.associated.length}
  <div class="pane-title sub">Associated</div>
  {#each data.associated as a (a.path)}
    <button class="linkrow" onclick={() => app.openNote(a.path)}>
      <div class="src">{a.title}</div>
      {#if a.cue}<div class="ctx dim">“{a.cue}”</div>{/if}
    </button>
  {/each}
{/if}
{#if data.similar.length}
  <div class="pane-title sub">Similar</div>
  {#each data.similar as s (s.path)}
    <button class="linkrow" onclick={() => app.openNote(s.path)}>
      <div class="src">{s.title}</div>
      <div class="ctx">{s.text}</div>
    </button>
  {/each}
{/if}
{#if data.suggested.length}
  <div class="pane-title sub">Suggested links</div>
  {#each data.suggested as s (s.path)}
    <div class="linkrow suggest">
      <button class="plain" onclick={() => app.openNote(s.path)}>{s.title}</button>
      <button class="chip" onclick={() => link(s.title)}>link</button>
    </div>
  {/each}
{/if}
{#if !data.associated.length && !data.similar.length && !data.suggested.length}
  <div class="pane-note">
    {app.embed.state === "ready" ? "Nothing related yet." : "The model is not ready."}
  </div>
{/if}
