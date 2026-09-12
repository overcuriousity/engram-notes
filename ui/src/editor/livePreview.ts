import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";
import { findWikilinks, displayText } from "../lib/wikilink";

// Lezer-markdown node names whose text is hidden on lines the cursor is not on.
const HIDDEN = new Set(["HeaderMark", "EmphasisMark", "CodeMark", "StrikethroughMark", "QuoteMark", "LinkMark", "URL"]);
const hide = Decoration.replace({});

class WikiWidget extends WidgetType {
  constructor(readonly text: string, readonly target: string) { super(); }
  eq(o: WikiWidget) { return o.text === this.text && o.target === this.target; }
  toDOM() {
    const a = document.createElement("a");
    a.className = "cm-wikilink";
    a.textContent = this.text;
    a.dataset.target = this.target;
    return a;
  }
  ignoreEvent() { return false; }
}

function activeLines(view: EditorView): Set<number> {
  const s = new Set<number>();
  for (const r of view.state.selection.ranges) {
    const a = view.state.doc.lineAt(r.from).number;
    const b = view.state.doc.lineAt(r.to).number;
    for (let i = a; i <= b; i++) s.add(i);
  }
  return s;
}

function build(view: EditorView): DecorationSet {
  const active = activeLines(view);
  const marks: { from: number; to: number; deco: Decoration }[] = [];
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter(node) {
        if (node.name === "FencedCode" || node.name === "CodeBlock") return false;
        const line = view.state.doc.lineAt(node.from).number;
        if (HIDDEN.has(node.name) && !active.has(line)) {
          const trailingSpace = node.name === "HeaderMark" && view.state.sliceDoc(node.to, node.to + 1) === " ";
          marks.push({ from: node.from, to: trailingSpace ? node.to + 1 : node.to, deco: hide });
        }
        return true;
      },
    });
    const text = view.state.sliceDoc(from, to);
    for (const l of findWikilinks(text)) {
      const a = from + l.from;
      const b = from + l.to;
      const line = view.state.doc.lineAt(a).number;
      const target = l.heading ? `${l.target}#${l.heading}` : l.target;
      marks.push(
        active.has(line)
          ? { from: a, to: b, deco: Decoration.mark({ class: "cm-wikilink-src" }) }
          : { from: a, to: b, deco: Decoration.replace({ widget: new WikiWidget(displayText(l), target) }) },
      );
    }
  }
  marks.sort((x, y) => x.from - y.from || x.to - y.to);
  const b = new RangeSetBuilder<Decoration>();
  let last = -1;
  for (const m of marks) {
    if (m.from < last) continue; // overlapping marks are dropped, never nested
    b.add(m.from, m.to, m.deco);
    last = m.to;
  }
  return b.finish();
}

export function livePreview(opts: { onFollow: (target: string) => void }) {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) { this.decorations = build(view); }
      update(u: ViewUpdate) {
        if (u.docChanged || u.viewportChanged || u.selectionSet) this.decorations = build(u.view);
      }
    },
    { decorations: (v) => v.decorations },
  );
  const clicks = EditorView.domEventHandlers({
    mousedown(e, view) {
      const el = (e.target as HTMLElement).closest?.("a.cm-wikilink") as HTMLElement | null;
      if (el?.dataset.target) { e.preventDefault(); opts.onFollow(el.dataset.target); return true; }
      if (!(e.ctrlKey || e.metaKey)) return false;
      const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
      if (pos == null) return false;
      const line = view.state.doc.lineAt(pos);
      const hit = findWikilinks(line.text).find((l) => pos >= line.from + l.from && pos <= line.from + l.to);
      if (!hit) return false;
      e.preventDefault();
      opts.onFollow(hit.heading ? `${hit.target}#${hit.heading}` : hit.target);
      return true;
    },
  });
  const style = EditorView.baseTheme({
    ".cm-wikilink, .cm-wikilink-src": { color: "var(--accent)", cursor: "pointer" },
    ".cm-wikilink:hover": { textDecoration: "underline" },
  });
  return [plugin, clicks, style];
}
