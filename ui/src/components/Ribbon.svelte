<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { errorMessage } from "../lib/api";

  // Obsidian's ribbon: the commands worth one click, in its order.
  const top = [
    ["new-note", "🖉", "New note"],
    ["switcher", "⌕", "Quick switcher"],
    ["search", "▤", "Search"],
    ["graph", "◍", "Graph view"],
    ["daily", "▦", "Daily note"],
    ["palette", "⌘", "Command palette"],
  ] as const;

  function run(id: string) {
    const c = allCommands().find((x) => x.id === id);
    if (!c) return;
    Promise.resolve(c.run()).catch((e) => app.say(errorMessage(e)));
  }
</script>

<nav class="ribbon">
  {#each top as [id, glyph, title] (id)}
    <button class="icon" {title} onclick={() => run(id)}>{glyph}</button>
  {/each}
  <span class="spacer"></span>
  <button class="icon" title="Settings" onclick={() => (app.settings = true)}>⚙</button>
</nav>
