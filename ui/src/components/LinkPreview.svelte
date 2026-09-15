<script lang="ts">
  import type { Preview } from "../lib/api";
  interface Props { title: string; preview: Preview; x: number; y: number }
  let { title, preview, x, y }: Props = $props();
</script>

<div class="preview" style="left:{x}px; top:{y}px" role="tooltip">
  <div class="title">{title}{#if preview.heading}<span class="path"> › {preview.heading}</span>{/if}</div>
  <pre>{preview.text}</pre>
</div>

<style>
  .preview {
    position: fixed; z-index: 30; max-width: 420px; max-height: 320px; overflow: hidden;
    background: var(--bg-2); color: var(--fg); border: 1px solid var(--border);
    border-radius: 6px; box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25); padding: 8px 10px; font-size: 0.9em;
  }
  .title { font-weight: 600; margin-bottom: 4px; }
  .path { font-weight: 400; color: var(--fg-muted); }
  pre { margin: 0; white-space: pre-wrap; font: inherit; }
</style>
