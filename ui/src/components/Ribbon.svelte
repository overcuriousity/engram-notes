<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { allCommands } from "../lib/commands";
  import { errorMessage } from "../lib/api";
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import Search from "@lucide/svelte/icons/search";
  import PanelLeft from "@lucide/svelte/icons/panel-left";
  import GitFork from "@lucide/svelte/icons/git-fork";
  import CalendarDays from "@lucide/svelte/icons/calendar-days";
  import Command from "@lucide/svelte/icons/command";
  import Settings from "@lucide/svelte/icons/settings";

  // Obsidian's ribbon: the commands worth one click, in its order and its icons.
  const top = [
    { id: "new-note", icon: FilePlus, title: "New note" },
    { id: "switcher", icon: PanelLeft, title: "Quick switcher" },
    { id: "search", icon: Search, title: "Search" },
    { id: "graph", icon: GitFork, title: "Graph view" },
    { id: "daily", icon: CalendarDays, title: "Daily note" },
    { id: "palette", icon: Command, title: "Command palette" },
  ];

  function run(id: string) {
    const c = allCommands().find((x) => x.id === id);
    if (!c) return;
    Promise.resolve(c.run()).catch((e) => app.say(errorMessage(e)));
  }
</script>

<nav class="ribbon">
  {#each top as t (t.id)}
    <button class="icon" title={t.title} onclick={() => run(t.id)}>
      <t.icon size={18} strokeWidth={1.75} />
    </button>
  {/each}
  <span class="spacer"></span>
  <button class="icon" title="Settings" onclick={() => (app.settings = true)}>
    <Settings size={18} strokeWidth={1.75} />
  </button>
</nav>
