import { syntaxTree } from "@codemirror/language";
import { type EditorState, RangeSetBuilder, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";
import { findWikilinks, displayText, linkTarget } from "../lib/wikilink";

// Lezer-markdown node names whose text is hidden on lines the cursor is not on.
const HIDDEN = new Set(["HeaderMark", "EmphasisMark", "CodeMark", "StrikethroughMark", "QuoteMark", "LinkMark", "URL"]);
const hide = Decoration.replace({});
const CALLOUT = /^\s*>\s*\[!(\w+)\][-+]?/;
const BLOCK_ID = /(^|[ \t])(\^[A-Za-z0-9-]+)[ \t]*$/gm;
const blockId = Decoration.mark({ class: "cm-block-id" });

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

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean, readonly pos: number) { super(); }
  eq(o: CheckboxWidget) { return o.checked === this.checked && o.pos === this.pos; }
  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-task-box";
    box.checked = this.checked;
    box.onmousedown = (e) => {
      e.preventDefault();
      view.dispatch({ changes: { from: this.pos + 1, to: this.pos + 2, insert: this.checked ? " " : "x" } });
    };
    return box;
  }
  ignoreEvent() { return true; }
}

class CalloutTitle extends WidgetType {
  constructor(readonly kind: string) { super(); }
  eq(o: CalloutTitle) { return o.kind === this.kind; }
  toDOM() {
    const s = document.createElement("span");
    s.className = "cm-callout-type";
    s.textContent = this.kind;
    return s;
  }
}

function activeLines(state: EditorState): Set<number> {
  const s = new Set<number>();
  for (const r of state.selection.ranges) {
    const a = state.doc.lineAt(r.from).number;
    const b = state.doc.lineAt(r.to).number;
    for (let i = a; i <= b; i++) s.add(i);
  }
  return s;
}

export function buildDecorations(state: EditorState, ranges: readonly { from: number; to: number }[]): DecorationSet {
  const active = activeLines(state);
  const marks: { from: number; to: number; deco: Decoration }[] = [];
  const push = (from: number, to: number, deco: Decoration) => marks.push({ from, to, deco });

  for (const { from, to } of ranges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter(node) {
        const name = node.name;
        if (name === "FencedCode" || name === "CodeBlock" || name === "Frontmatter") return false;
        const line = state.doc.lineAt(node.from).number;
        // `[!note]` and other bracketed text without a URL is not a link.
        if ((name === "Link" || name === "Image") && !node.node.getChild("URL")) return false;
        if (name === "Blockquote") {
          const first = state.doc.lineAt(node.from);
          const m = CALLOUT.exec(first.text);
          const cls = m ? "cm-callout" : "cm-quote";
          for (let pos = node.from; pos <= node.to; ) {
            const l = state.doc.lineAt(pos);
            push(l.from, l.from, Decoration.line({ class: cls }));
            pos = l.to + 1;
          }
          if (m && !active.has(first.number)) {
            const start = first.from + first.text.indexOf("[!");
            push(start, first.from + m[0].length, Decoration.replace({ widget: new CalloutTitle(m[1]) }));
          }
        }
        if (name === "TaskMarker") {
          const overlaps = state.selection.ranges.some((r) => r.to >= node.from && r.from <= node.to);
          if (!overlaps) {
            const checked = state.sliceDoc(node.from + 1, node.from + 2).toLowerCase() === "x";
            push(node.from, node.to, Decoration.replace({ widget: new CheckboxWidget(checked, node.from) }));
          }
          return false;
        }
        if (HIDDEN.has(name) && !active.has(line)) {
          const trailingSpace = name === "HeaderMark" && state.sliceDoc(node.to, node.to + 1) === " ";
          push(node.from, trailingSpace ? node.to + 1 : node.to, hide);
        }
        return true;
      },
    });
    const text = state.sliceDoc(from, to);
    for (const l of findWikilinks(text)) {
      const a = from + l.from;
      const b = from + l.to;
      const line = state.doc.lineAt(a).number;
      const target = linkTarget(l);
      push(
        a,
        b,
        active.has(line)
          ? Decoration.mark({ class: "cm-wikilink-src" })
          : Decoration.replace({ widget: new WikiWidget(displayText(l), target) }),
      );
    }
    for (const m of text.matchAll(BLOCK_ID)) {
      const start = from + m.index + m[1].length;
      push(start, start + m[2].length, blockId);
    }
  }
  // Line decorations are zero-width and sort before anything starting on their line.
  marks.sort((x, y) => x.from - y.from || x.to - y.to);
  const b = new RangeSetBuilder<Decoration>();
  let last = -1;
  for (const m of marks) {
    if (m.from < last) continue; // overlapping ranges are dropped, never nested
    b.add(m.from, m.to, m.deco);
    last = m.to;
  }
  return b.finish();
}

// The frontmatter block, as the character range of its fences, or null.
function frontmatterRange(state: EditorState): { from: number; to: number } | null {
  const doc = state.doc;
  if (doc.lines < 2 || doc.line(1).text !== "---") return null;
  for (let i = 2; i <= doc.lines; i++) {
    if (doc.line(i).text === "---") return { from: 0, to: doc.line(i).to };
  }
  return null;
}

class PropertiesFold extends WidgetType {
  eq() { return true; }
  toDOM(view: EditorView) {
    const d = document.createElement("div");
    d.className = "cm-props-fold";
    d.textContent = "Properties";
    d.onmousedown = (e) => {
      e.preventDefault();
      view.dispatch({ selection: { anchor: 4 } });
      view.focus();
    };
    return d;
  }
  ignoreEvent() { return true; }
}

// Block replacements span lines, which only a state field may provide.
const frontmatterFold = StateField.define<DecorationSet>({
  create: (state) => foldFor(state),
  update: (value, tr) => (tr.docChanged || tr.selection ? foldFor(tr.state) : value),
  provide: (f) => EditorView.decorations.from(f),
});

export function foldFor(state: EditorState): DecorationSet {
  const r = frontmatterRange(state);
  if (!r) return Decoration.none;
  const inside = state.selection.ranges.some((s) => s.from <= r.to && s.to >= r.from);
  if (inside) return Decoration.none;
  return Decoration.set([Decoration.replace({ widget: new PropertiesFold(), block: true }).range(r.from, r.to)]);
}

export function livePreview(opts: { onFollow: (target: string) => void }) {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) { this.decorations = buildDecorations(view.state, view.visibleRanges); }
      update(u: ViewUpdate) {
        if (u.docChanged || u.viewportChanged || u.selectionSet) this.decorations = buildDecorations(u.view.state, u.view.visibleRanges);
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
      opts.onFollow(linkTarget(hit));
      return true;
    },
  });
  const style = EditorView.baseTheme({
    ".cm-wikilink, .cm-wikilink-src": { color: "var(--accent)", cursor: "pointer" },
    ".cm-wikilink:hover": { textDecoration: "underline" },
    ".cm-task-box": { margin: "0 6px 0 0", verticalAlign: "middle", cursor: "pointer", accentColor: "var(--accent)" },
    ".cm-quote": { borderLeft: "3px solid var(--border)", paddingLeft: "12px !important", color: "var(--fg-muted)" },
    ".cm-callout": { borderLeft: "3px solid var(--accent)", background: "var(--accent-bg)", paddingLeft: "12px !important" },
    ".cm-block-id": { color: "var(--fg-muted)", fontSize: "0.85em" },
    ".cm-callout-type": { fontWeight: "600", textTransform: "capitalize", color: "var(--accent)" },
    ".cm-props-fold": {
      color: "var(--fg-muted)", fontSize: "12px", textTransform: "uppercase", letterSpacing: ".06em",
      borderBottom: "1px solid var(--border)", padding: "2px 0 6px", marginBottom: "8px", cursor: "pointer",
    },
  });
  return [plugin, frontmatterFold, clicks, style];
}
