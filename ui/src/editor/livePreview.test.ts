import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { ensureSyntaxTree } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { yamlFrontmatter } from "@codemirror/lang-yaml";
import { buildDecorations, foldFor } from "./livePreview";

function stateOf(doc: string, cursor: number) {
  const state = EditorState.create({
    doc,
    selection: { anchor: cursor },
    extensions: [yamlFrontmatter({ content: markdown({ base: markdownLanguage }) })],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

// Each decoration as the text it covers and a name: widget class, CSS class, or "hide".
function decos(doc: string, cursor = doc.length, focused = true) {
  const out: { text: string; kind: string }[] = [];
  buildDecorations(stateOf(doc, cursor), [{ from: 0, to: doc.length }], focused).between(0, doc.length, (from, to, d) => {
    const spec = d.spec;
    const kind = spec.widget ? spec.widget.constructor.name : (spec.class ?? "hide");
    out.push({ text: doc.slice(from, to), kind });
  });
  return out;
}

describe("live preview decorations", () => {
  it("hides heading marks except on the cursor line", () => {
    expect(decos("# Title\n\ntext")).toContainEqual({ text: "# ", kind: "hide" });
    expect(decos("# Title\n\ntext", 0)).not.toContainEqual({ text: "# ", kind: "hide" });
  });

  it("reveals no source while the editor is not focused", () => {
    expect(decos("# Title\n\ntext", 0, false)).toContainEqual({ text: "# ", kind: "hide" });
    expect(decos("[[Note]] x", 0, false)).toContainEqual({ text: "[[Note]]", kind: "WikiWidget" });
    expect(decos("- [ ] a\n\nz", 3, false).map((x) => x.kind)).toContain("CheckboxWidget");
  });

  it("draws wikilinks, showing the source on the cursor line", () => {
    expect(decos("[[Note|Shown]] x\n\nend")).toContainEqual({ text: "[[Note|Shown]]", kind: "WikiWidget" });
    expect(decos("[[Note|Shown]] x\n\nend", 0)).toContainEqual({ text: "[[Note|Shown]]", kind: "cm-wikilink-src" });
  });

  it("draws task markers as checkboxes", () => {
    const d = decos("- [ ] a\n- [x] b\n\nz");
    expect(d.filter((x) => x.kind === "CheckboxWidget").map((x) => x.text)).toEqual(["[ ]", "[x]"]);
  });

  it("styles callouts and replaces their marker", () => {
    const d = decos("> [!note] Hi\n> body\n\nz");
    expect(d.filter((x) => x.kind === "cm-callout")).toHaveLength(2);
    expect(d).toContainEqual({ text: "[!note]", kind: "CalloutTitle" });
  });

  it("mutes block ids", () => {
    expect(decos("para ^abc\n\nz")).toContainEqual({ text: "^abc", kind: "cm-block-id" });
  });

  it("folds frontmatter unless the cursor is inside it", () => {
    const doc = "---\na: 1\n---\nbody";
    expect(foldFor(stateOf(doc, doc.length)).size).toBe(1);
    expect(foldFor(stateOf(doc, 5)).size).toBe(0);
  });
});
