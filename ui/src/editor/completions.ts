import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";

export function wikiCompletion(titles: () => [string, string][]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const m = ctx.matchBefore(/\[\[([^\]|#]*)$/);
    if (!m) return null;
    const q = m.text.slice(2).toLowerCase();
    const options = titles()
      .filter(([p, t]) => !q || t.toLowerCase().includes(q) || p.toLowerCase().includes(q))
      .slice(0, 50)
      .map(([p, t]) => {
        const stem = p.split("/").pop()!.replace(/\.md$/i, "");
        return { label: t, detail: p, apply: `[[${stem}]]` };
      });
    return { from: m.from, options, filter: false };
  };
}

export function tagCompletion(tags: () => string[]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const m = ctx.matchBefore(/(^|\s)#([\w/-]*)$/);
    if (!m) return null;
    const from = m.from + m.text.indexOf("#");
    return { from, options: tags().map((t) => ({ label: `#${t}` })), validFor: /^#[\w/-]*$/ };
  };
}

export const completions = (titles: () => [string, string][], tags: () => string[]) =>
  autocompletion({ override: [wikiCompletion(titles), tagCompletion(tags)], icons: false });
