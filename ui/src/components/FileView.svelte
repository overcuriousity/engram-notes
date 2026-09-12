<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { app } from "../lib/state.svelte";
  import { attachmentPath, openExternal, errorMessage } from "../lib/api";
  import { fileKind } from "../lib/files";

  let { path }: { path: string } = $props();
  const kind = $derived(fileKind(path));
  const name = $derived(path.split("/").pop());
  const size = $derived(app.files.find((f) => f.path === path)?.size ?? 0);
  let src = $state<string | null>(null);

  $effect(() => {
    attachmentPath(path)
      .then((abs) => (src = convertFileSrc(abs)))
      .catch((e) => app.say(errorMessage(e)));
  });

  function external() {
    openExternal(path).catch((e) => app.say(errorMessage(e)));
  }
</script>

<div class="fileview" class:pdf={kind === "pdf"}>
  {#if src && kind === "image"}
    <img {src} alt={name} />
  {:else if src && kind === "pdf"}
    <iframe {src} title={name}></iframe>
  {:else if kind === "other"}
    <div class="name">{name}</div>
    <div class="size">{(size / 1024).toFixed(1)} KB</div>
  {/if}
  <button class="external" onclick={external}>Open in default app</button>
</div>
