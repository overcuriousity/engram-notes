import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

export const editorTheme = EditorView.theme({
  "&": { color: "var(--fg)", backgroundColor: "var(--bg)" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "var(--accent-bg) !important" },
  ".cm-activeLine": { backgroundColor: "transparent" },
  ".cm-gutters": { backgroundColor: "transparent", border: "none", color: "var(--fg-muted)" },
  ".cm-foldGutter .cm-gutterElement": { width: "24px", display: "flex", justifyContent: "center", paddingTop: "0.3em" },
  ".cm-fold-marker": { opacity: "0", width: "16px", height: "16px", cursor: "pointer", transition: "opacity 120ms" },
  ".cm-fold-marker svg": { display: "block" },
  ".cm-fold-marker.closed": { opacity: "1", transform: "rotate(-90deg)" },
  ".cm-gutters:hover .cm-fold-marker, .cm-activeLineGutter .cm-fold-marker": { opacity: "0.6" },
  ".cm-foldGutter .cm-gutterElement:hover .cm-fold-marker": { opacity: "1" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" },
  ".cm-foldPlaceholder": { background: "var(--bg-3)", border: "none", color: "var(--fg-muted)", padding: "0 6px", borderRadius: "3px" },
});

export const markdownHighlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.heading1, fontSize: "1.8em", fontWeight: "700" },
    { tag: tags.heading2, fontSize: "1.5em", fontWeight: "700" },
    { tag: tags.heading3, fontSize: "1.25em", fontWeight: "600" },
    { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "600" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.strikethrough, textDecoration: "line-through" },
    { tag: tags.monospace, fontFamily: "var(--font-mono)", fontSize: "0.9em", background: "var(--bg-3)", borderRadius: "3px" },
    { tag: tags.link, color: "var(--accent)" },
    { tag: tags.url, color: "var(--fg-muted)" },
    { tag: tags.quote, color: "var(--fg-muted)", fontStyle: "italic" },
    { tag: tags.processingInstruction, color: "var(--fg-muted)" },
    { tag: tags.meta, color: "var(--fg-muted)" },
  ]),
);
