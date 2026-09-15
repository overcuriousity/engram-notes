import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import type { LinkCandidate } from "../lib/api";
import { blockTrigger } from "../lib/linkpicker";

/** Hybrid `[[` completion: the backend lists spelling matches, then meaning matches. */
export function wikiCompletion(
  candidates: (q: string) => Promise<LinkCandidate[]>,
  onBlockSearch: (from: number, to: number, query: string) => void,
) {
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const lineFrom = ctx.state.doc.lineAt(ctx.pos).from;
    const trigger = blockTrigger(ctx.state.sliceDoc(lineFrom, ctx.pos));
    if (trigger) {
      onBlockSearch(lineFrom + trigger.from, ctx.pos, trigger.query);
      return null;
    }
    const m = ctx.matchBefore(/\[\[([^\]|#]*)$/);
    if (!m) return null;
    const found = await candidates(m.text.slice(2));
    const options = found.map((c) => {
      const stem = c.path.split("/").pop()!.replace(/\.md$/i, "");
      return { label: c.title, detail: c.kind === "meaning" ? `meaning · ${c.path}` : c.path, apply: `[[${stem}]]` };
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

export const completions = (
  candidates: (q: string) => Promise<LinkCandidate[]>,
  onBlockSearch: (from: number, to: number, query: string) => void,
  tags: () => string[],
) => autocompletion({ override: [wikiCompletion(candidates, onBlockSearch), tagCompletion(tags)], icons: false });
