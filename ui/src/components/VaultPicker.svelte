<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { recentVaults, errorMessage } from "../lib/api";
  import { app } from "../lib/state.svelte";

  let recent = $state<string[]>([]);
  onMount(async () => {
    recent = await recentVaults();
  });

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: "Open vault folder" });
    if (typeof dir === "string") await openIt(dir);
  }
  async function openIt(dir: string) {
    try {
      await app.open(dir);
    } catch (e) {
      app.say(errorMessage(e));
    }
  }
</script>

<div style="display:grid;place-items:center;height:100%">
  <div style="width:420px">
    <h1 style="font-weight:600">engram-notes</h1>
    <button onclick={pick} style="padding:8px 14px;background:var(--accent);color:white;border-radius:var(--radius)">
      Open folder as vault
    </button>
    {#if recent.length}
      <div class="pane-title" style="margin-top:24px;padding-left:0">Recent</div>
      <div class="tree">
        {#each recent as r (r)}<button onclick={() => openIt(r)}>{r}</button>{/each}
      </div>
    {/if}
  </div>
</div>
