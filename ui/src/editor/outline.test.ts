import { describe, expect, it } from "vitest";
import { EditorState, type Transaction } from "@codemirror/state";
import { ensureSyntaxTree, indentUnit, codeFolding } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { closeBrackets, insertBracket } from "@codemirror/autocomplete";
import { markdownBrackets, markdownEnter } from "./outline";

function stateOf(doc: string, cursor: number | { anchor: number; head: number }, indent = 2) {
  const state = EditorState.create({
    doc,
    selection: typeof cursor === "number" ? { anchor: cursor } : cursor,
    extensions: [
      markdown({ base: markdownLanguage }),
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
