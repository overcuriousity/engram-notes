import { EditorSelection, type EditorState, type Extension, type StateCommand } from "@codemirror/state";
import type { KeyBinding } from "@codemirror/view";
import { indentLess, indentMore } from "@codemirror/commands";
import { acceptCompletion } from "@codemirror/autocomplete";
import { codeFolding, foldGutter, foldNodeProp, getIndentUnit } from "@codemirror/language";
import { insertNewlineContinueMarkupCommand, markdownLanguage } from "@codemirror/lang-markdown";
import * as O from "../lib/outline";

// closeBrackets reads the pairing set from language data. Obsidian pairs the
// emphasis characters as well as brackets, and wraps a selection with any of them.
export const markdownBrackets: Extension = markdownLanguage.data.of({
  closeBrackets: { brackets: ["(", "[", "{", "'", '"', "*", "_", "`"] },
});

// Enter continues the list; on an empty item it drops one level of markup, as
// Obsidian does. The default would make a two-item list loose instead.
export const markdownEnter = insertNewlineContinueMarkupCommand({ nonTightLists: false });

const lineArray = (state: EditorState) => state.doc.toString().split("\n");

function cursorLine(state: EditorState) {
  const l = state.doc.lineAt(state.selection.main.head);
  return { line: l.number - 1, col: state.selection.main.head - l.from };
}

// Replaces only the span of lines that differ and puts the cursor on `line`
// at `col`. Outline operations never change the line count, which is what
// lets the diff be a plain replacement.
function applyLines(state: EditorState, oldLines: string[], newLines: string[], line: number, col: number) {
  let a = 0;
  while (a < oldLines.length && oldLines[a] === newLines[a]) a++;
  let b = oldLines.length;
  while (b > a && oldLines[b - 1] === newLines[b - 1]) b--;
  const changes = b > a
    ? { from: state.doc.line(a + 1).from, to: state.doc.line(b).to, insert: newLines.slice(a, b).join("\n") }
    : [];
  const head = newLines.slice(0, line).reduce((n, l) => n + l.length + 1, 0) + Math.min(col, newLines[line].length);
  return state.update({ changes, selection: EditorSelection.cursor(head), scrollIntoView: true, userEvent: "input" });
}

// A command that runs only with a single cursor on a list item. `f` returning
// null means the operation does not apply here (top of the list, no sibling);
// the key is still consumed, as Obsidian does, so nothing else fires.
function onItem(f: (lines: string[], i: number, unit: number) => { lines: string[]; line: number } | null): StateCommand {
  return ({ state, dispatch }) => {
    if (!state.selection.main.empty) return false;
    const { line, col } = cursorLine(state);
    const lines = lineArray(state);
    if (!O.parseItem(lines[line])) return false;
    const r = f(lines, line, getIndentUnit(state));
    if (!r) return true;
    const shift = O.indentOf(r.lines[r.line]) - O.indentOf(lines[line]);
    dispatch(applyLines(state, lines, r.lines, r.line, Math.max(0, col + shift)));
    return true;
  };
}

export const indentItem = onItem((lines, i, unit) => ({ lines: O.indentSubtree(lines, i, unit), line: i }));
export const outdentItem = onItem((lines, i, unit) => {
  const out = O.outdentSubtree(lines, i, unit);
  return out && { lines: out, line: i };
});
export const moveItemUp = onItem((lines, i) => O.moveSubtree(lines, i, -1));
export const moveItemDown = onItem((lines, i) => O.moveSubtree(lines, i, 1));

// Enter on an empty nested item outdents it with whatever sits under it;
// markdownEnter handles the rest, including an empty top-level item.
export const enterInList: StateCommand = ({ state, dispatch }) => {
  if (!state.selection.main.empty) return false;
  const { line } = cursorLine(state);
  const lines = lineArray(state);
  const item = O.parseItem(lines[line]);
  if (!item || item.text.trim() !== "" || item.indent === 0) return false;
  const out = O.outdentSubtree(lines, line, getIndentUnit(state))!;
  dispatch(applyLines(state, lines, out, line, out[line].length));
  return true;
};

// The spec's Tab precedence: completion, then the outline, then indentation.
export const outlineKeymap: KeyBinding[] = [
  { key: "Tab", run: acceptCompletion },
  { key: "Tab", run: indentItem },
  { key: "Tab", run: indentMore },
  { key: "Shift-Tab", run: outdentItem },
  { key: "Shift-Tab", run: indentLess },
  { key: "Alt-ArrowUp", run: moveItemUp },
  { key: "Alt-ArrowDown", run: moveItemDown },
  { key: "Enter", run: enterInList },
  { key: "Enter", run: markdownEnter },
];

// lang-markdown makes every block foldable. Obsidian folds headings and list
// items only; a later prop source wins, so the rest are switched off here.
export const listOnlyFolding = {
  props: [
    foldNodeProp.add({
      Paragraph: () => null,
      Blockquote: () => null,
      FencedCode: () => null,
      CodeBlock: () => null,
      HTMLBlock: () => null,
      Table: () => null,
      LinkReference: () => null,
      CommentBlock: () => null,
      ProcessingInstructionBlock: () => null,
    }),
  ],
};

const CHEVRON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>';

export function outlineFolding(): Extension {
  return [
    codeFolding(),
    foldGutter({
      markerDOM(open) {
        const s = document.createElement("span");
        s.className = `cm-fold-marker ${open ? "open" : "closed"}`;
        s.innerHTML = CHEVRON;
        return s;
      },
    }),
  ];
}
