import { describe, expect, it } from "vitest";
import { EditorState, type Transaction } from "@codemirror/state";
import { ensureSyntaxTree, indentUnit, codeFolding, foldable, foldEffect } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { closeBrackets, insertBracket } from "@codemirror/autocomplete";
import { enterInList, foldKeys, foldRestore, foldTransaction, indentItem, listOnlyFolding, markdownBrackets, markdownEnter, moveItemDown, moveItemUp, outdentItem } from "./outline";

function stateOf(doc: string, cursor: number | { anchor: number; head: number }, indent = 2) {
  const state = EditorState.create({
    doc,
    selection: typeof cursor === "number" ? { anchor: cursor } : cursor,
    extensions: [
      markdown({ base: markdownLanguage, extensions: [listOnlyFolding] }),
      markdownBrackets,
      indentUnit.of(" ".repeat(indent)),
      closeBrackets(),
      codeFolding(),
    ],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return state;
}

// Runs a state command and reports the document and cursor it left behind.
function run(cmd: (t: { state: EditorState; dispatch: (tr: Transaction) => void }) => boolean, state: EditorState) {
  let out = state;
  const ok = cmd({ state, dispatch: (tr) => (out = tr.state) });
  return { ok, doc: out.doc.toString(), head: out.selection.main.head };
}

describe("brackets", () => {
  it("wraps a selection instead of replacing it, twice for a wikilink", () => {
    const s = stateOf("say word here", { anchor: 4, head: 8 });
    const once = insertBracket(s, "[")!;
    expect(once.state.doc.toString()).toBe("say [word] here");
    const twice = insertBracket(once.state, "[")!;
    expect(twice.state.doc.toString()).toBe("say [[word]] here");
    expect([twice.state.selection.main.from, twice.state.selection.main.to]).toEqual([6, 10]);
  });

  it("wraps with the markdown emphasis characters too", () => {
    const s = stateOf("say word here", { anchor: 4, head: 8 });
    for (const [ch, want] of [["*", "say *word* here"], ["_", "say _word_ here"], ["`", "say `word` here"], ['"', 'say "word" here'], ["(", "say (word) here"]]) {
      expect(insertBracket(s, ch)!.state.doc.toString()).toBe(want);
    }
  });
});

describe("enter", () => {
  it("continues a list, and removes the marker of an empty top-level item", () => {
    expect(run(markdownEnter, stateOf("- a", 3)).doc).toBe("- a\n- ");
    expect(run(markdownEnter, stateOf("1. a", 4)).doc).toBe("1. a\n2. ");
    expect(run(markdownEnter, stateOf("- [ ] a", 7)).doc).toBe("- [ ] a\n- [ ] ");
    expect(run(markdownEnter, stateOf("- a\n- ", 6)).doc).toBe("- a\n");
  });
});

describe("outline commands", () => {
  it("outdents an empty nested item on Enter and leaves the rest to the markdown keymap", () => {
    expect(run(enterInList, stateOf("- a\n  - ", 8))).toEqual({ ok: true, doc: "- a\n- ", head: 6 });
    expect(run(enterInList, stateOf("- a\n  - [ ] ", 12))).toEqual({ ok: true, doc: "- a\n- [ ] ", head: 10 });
    expect(run(enterInList, stateOf("- a\n- ", 6)).ok).toBe(false);
    expect(run(enterInList, stateOf("- a\n  - b", 9)).ok).toBe(false);
  });

  it("keeps the cursor on its character when a tab indent is rewritten as spaces", () => {
    expect(run(indentItem, stateOf("\t- ab", 4))).toEqual({ ok: true, doc: "      - ab", head: 9 });
  });

  it("indents and outdents the subtree under the cursor, keeping the cursor on its text", () => {
    expect(run(indentItem, stateOf("- a\n- b\n  - c", 5))).toEqual({ ok: true, doc: "- a\n  - b\n    - c", head: 7 });
    expect(run(outdentItem, stateOf("- a\n  - b\n    - c", 7))).toEqual({ ok: true, doc: "- a\n- b\n  - c", head: 5 });
    expect(run(outdentItem, stateOf("- a", 1)).ok).toBe(true);
    expect(run(indentItem, stateOf("prose", 2)).ok).toBe(false);
  });

  it("moves a subtree among its siblings", () => {
    expect(run(moveItemDown, stateOf("- a\n  - a1\n- b", 1))).toEqual({ ok: true, doc: "- b\n- a\n  - a1", head: 5 });
    expect(run(moveItemUp, stateOf("- b\n- a\n  - a1", 5))).toEqual({ ok: true, doc: "- a\n  - a1\n- b", head: 1 });
    expect(run(moveItemUp, stateOf("- a\n- b", 1)).ok).toBe(true);
  });
});

describe("folding", () => {
  it("folds list items and headings, never paragraphs", () => {
    const s = stateOf("- a\n  - b\n\npara one\npara two\n# H\ntext", 0);
    const l = (n: number) => s.doc.line(n);
    const item = foldable(s, l(1).from, l(1).to)!;
    expect(item.from).toBe(3);
    expect(s.doc.sliceString(0, item.to).trimEnd()).toBe("- a\n  - b");
    expect(foldable(s, l(4).from, l(4).to)).toBeNull();
    expect(foldable(s, l(6).from, l(6).to)).toEqual({ from: l(6).to, to: s.doc.length });
    expect(foldable(s, l(2).from, l(2).to)).toBeNull();
  });

  it("names folded items by outline path and headings by ordinal, and restores them", () => {
    const s = stateOf("- a\n  - b\n- c\n  - d\n# H\ntext", 0);
    const range = foldable(s, s.doc.line(3).from, s.doc.line(3).to)!;
    const folded = s.update({ effects: foldEffect.of(range) }).state;
    expect(foldKeys(folded)).toEqual(["0:1"]);
    const restored = s.update(foldTransaction(s, ["0:1", "h1"])!).state;
    expect(foldKeys(restored)).toEqual(["0:1", "h1"]);
    expect(foldTransaction(s, ["9:9"])).toBeNull();
  });

  it("does not count a hash inside a fenced code block as a heading", () => {
    const s = stateOf("# One\n\n```sh\n# not a heading\n```\n\n# Two\ntext", 0);
    const range = foldable(s, s.doc.line(7).from, s.doc.line(7).to)!;
    expect(foldKeys(s.update({ effects: foldEffect.of(range) }).state)).toEqual(["h2"]);
  });

  it("does not let a bullet inside a fenced code block shift the list keys", () => {
    const key = (doc: string, line: number) => {
      const s = stateOf(doc, 0);
      const range = foldable(s, s.doc.line(line).from, s.doc.line(line).to)!;
      return foldKeys(s.update({ effects: foldEffect.of(range) }).state);
    };
    expect(key("```sh\necho hi\n```\n\n- real\n  - child\n", 5)).toEqual(["0:0"]);
    expect(key("```sh\n- x\n```\n\n- real\n  - child\n", 5)).toEqual(["0:0"]);
  });

  it("marks a restore, so the fold it applies is not read back as the user's", () => {
    const s = stateOf("- a\n  - b\n- c", 0);
    const tr = s.update(foldTransaction(s, ["0:0"])!);
    expect(tr.annotation(foldRestore)).toBe(true);
  });
});
