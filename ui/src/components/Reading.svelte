<script lang="ts">
  import { renderMarkdown } from "../lib/render";
  let { text, onFollow, onToggleTask }: { text: string; onFollow: (t: string) => void; onToggleTask: (line: number) => void } = $props();
  const html = $derived(renderMarkdown(text));

  function onClick(e: MouseEvent) {
    const el = e.target as HTMLElement;
    const a = el.closest("a.wikilink") as HTMLElement | null;
    if (a?.dataset.target) {
      e.preventDefault();
      onFollow(a.dataset.target);
      return;
    }
    if (el instanceof HTMLInputElement && el.type === "checkbox") onToggleTask(Number(el.dataset.line));
  }
</script>

<!-- markdown-it runs with html disabled; only our own tags reach here -->
<div class="reading" onclick={onClick} role="presentation">{@html html}</div>

<style>
  .reading { max-width: 760px; margin: 0 auto; padding: 24px 32px; font-family: var(--font-text); font-size: 16px; }
  .reading :global(a.wikilink) { color: var(--accent); cursor: pointer; }
  .reading :global(.tag) { color: var(--accent); background: var(--accent-bg); border-radius: 10px; padding: 0 6px; font-size: .9em; }
  .reading :global(pre) { background: var(--bg-2); padding: 12px; border-radius: var(--radius); overflow: auto; font-family: var(--font-mono); font-size: .9em; }
  .reading :global(code) { font-family: var(--font-mono); font-size: .9em; background: var(--bg-3); border-radius: 3px; padding: 0 3px; }
  .reading :global(blockquote) { border-left: 3px solid var(--border); margin: 0; padding-left: 12px; color: var(--fg-muted); }
  .reading :global(.callout) { border-left-color: var(--accent); background: var(--accent-bg); padding: 8px 12px; border-radius: var(--radius); color: var(--fg); }
  .reading :global(.callout)::before { content: attr(data-title); display: block; font-weight: 600; margin-bottom: 4px; }
  .reading :global(table) { border-collapse: collapse; }
  .reading :global(td), .reading :global(th) { border: 1px solid var(--border); padding: 4px 8px; }
</style>
