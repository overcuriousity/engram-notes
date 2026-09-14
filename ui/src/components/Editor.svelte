<script lang="ts">
  import { onMount } from "svelte";
  import { Compartment, EditorState, Transaction } from "@codemirror/state";
  import { EditorView, keymap, drawSelection, highlightActiveLine, highlightActiveLineGutter } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
  import { markdown, markdownLanguage, markdownKeymap } from "@codemirror/lang-markdown";
  import { yamlFrontmatter } from "@codemirror/lang-yaml";
  import { languages } from "@codemirror/language-data";
  import { ensureSyntaxTree, foldEffect, indentUnit, unfoldEffect } from "@codemirror/language";
  import { editorTheme, markdownHighlight } from "../editor/theme";
  import { livePreview } from "../editor/livePreview";
  import { completions } from "../editor/completions";
  import { foldKeys, foldRestore, foldTransaction, listOnlyFolding, markdownBrackets, outlineFolding, outlineKeymap } from "../editor/outline";
  import { textDiff } from "../lib/textdiff";

  interface Props {
    text: string;
    mode: "live" | "source";
    focus: boolean;
    jump: { line: number } | null;
    onJumped: () => void;
    insert: { text: string; replace: boolean; n: number } | null;
    onInserted: () => void;
    onchange: (t: string) => void;
    onblur: () => void;
    onFollow: (target: string) => void;
    titles: () => [string, string][];
    tags: () => string[];
    image: (target: string) => string | null;
    indent: number;
    // Fold keys from the index, or null while they load; and where to send changes.
    folds: string[] | null;
    onFolds: (keys: string[]) => void;
  }
  let { text, mode, focus, jump, onJumped, insert, onInserted, onchange, onblur, onFollow, titles, tags, image, indent, folds, onFolds }: Props = $props();
  let host: HTMLDivElement;
  // State, so effects that need the view run again once it exists.
  let view = $state.raw<EditorView>();
  const modeComp = new Compartment();
  const indentComp = new Compartment();
  const forMode = (m: string) => (m === "live" ? livePreview({ onFollow, image }) : []);
  // An empty indent unit makes indentUnit throw, which would leave the pane blank.
  const indentSpaces = (n: number) => " ".repeat(Math.min(8, Math.max(1, Math.round(n) || 1)));
  let alive = true;

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          outlineFolding(),
          highlightSelectionMatches(),
          closeBrackets(),
          markdownBrackets,
          // GFM for tasks and strikethrough; YAML so frontmatter is not read as a heading.
          yamlFrontmatter({ content: markdown({ base: markdownLanguage, codeLanguages: languages, extensions: [listOnlyFolding] }) }),
          editorTheme,
          markdownHighlight,
          EditorView.lineWrapping,
          indentComp.of(indentUnit.of(indentSpaces(indent))),
          modeComp.of(forMode(mode)),
          completions(titles, tags),
          keymap.of([...closeBracketsKeymap, ...outlineKeymap, ...markdownKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString());
            const folded = u.transactions.some((tr) => tr.effects.some((e) => e.is(foldEffect) || e.is(unfoldEffect)));
            if (!folded || u.transactions.some((tr) => tr.annotation(foldRestore))) return;
            if (foldsOpen) onFolds(foldKeys(u.state));
            else userFolded = true;
          }),
          EditorView.domEventHandlers({
            blur: () => {
              onblur();
              return false;
            },
          }),
        ],
      }),
    });
    // Start below the frontmatter so live preview can fold it.
    const fm = /^---\r?\n[\s\S]*?\r?\n---(\r?\n|$)/.exec(text);
    if (fm) view.dispatch({ selection: { anchor: Math.min(fm[0].length, text.length) } });
    // Several panes mount at once, and focusing one makes it the active pane.
    if (focus) view.focus();
    return () => {
      alive = false;
      view?.destroy();
    };
  });

  $effect(() => {
    view?.dispatch({ effects: modeComp.reconfigure(forMode(mode)) });
  });

  // The setting changes under an open pane, so Tab and Enter have to follow it.
  $effect(() => {
    view?.dispatch({ effects: indentComp.reconfigure(indentUnit.of(indentSpaces(indent))) });
  });

  // Folds are applied once, when both the view and the keys exist. The tree has
  // to be parsed that far first or foldable() answers null, and on a long note
  // one budget is not enough, so a null tree waits for the background parse
  // rather than folding half the keys. Until that has happened the pane does
  // not write folds back: the saved set is not in the view yet, so writing what
  // is would drop the rest of it. A user who folds during the wait has taken
  // over, and their state is written instead of the stale one being restored.
  let foldsApplied = false;
  let foldsOpen = false;
  let userFolded = false;
  $effect(() => {
    const v = view;
    const keys = folds;
    if (!v || !keys || foldsApplied) return;
    foldsApplied = true;
    if (keys.length && !userFolded) restoreFolds(v, keys, 0);
    else openFolds(v);
  });

  function openFolds(v: EditorView) {
    foldsOpen = true;
    if (userFolded) onFolds(foldKeys(v.state));
  }

  function restoreFolds(v: EditorView, keys: string[], tries: number) {
    if (!alive) return;
    if (userFolded) return openFolds(v);
    // A small budget per attempt: the whole parse of a long note in one call
    // would hold the frame for as long as it takes.
    if (!ensureSyntaxTree(v.state, v.state.doc.length, 200) && tries < 20) {
      setTimeout(() => restoreFolds(v, keys, tries + 1), 100);
      return;
    }
    const tr = foldTransaction(v.state, keys);
    if (tr) v.dispatch(tr);
    openFolds(v);
  }

  // Another pane's typing or an outside reload changed the note. Only the
  // difference is applied, so this editor keeps its cursor and scroll.
  $effect(() => {
    if (!view) return;
    const current = view.state.doc.toString();
    if (text !== current) {
      view.dispatch({ changes: textDiff(current, text), annotations: Transaction.addToHistory.of(false) });
    }
  });

  $effect(() => {
    if (!view || !jump) return;
    const doc = view.state.doc;
    const line = doc.line(Math.min(Math.max(jump.line, 1), doc.lines));
    view.dispatch({ selection: { anchor: line.from } });
    // The pane is the scroller now, so scroll the line into that, not into
    // CodeMirror's own scroller, which no longer moves.
    const rect = view.coordsAtPos(line.from);
    const box = host.closest(".note");
    if (rect && box) {
      const top = rect.top - box.getBoundingClientRect().top + box.scrollTop;
      box.scrollTo({ top: Math.max(0, top - 24) });
    }
    view.focus();
    onJumped();
  });

  // Put the text where the cursor is. A template takes the place of the
  // selection, as Obsidian's insert does; the Related pane's *link* does not,
  // so a stray click there cannot destroy what the user had highlighted.
  $effect(() => {
    const req = insert;
    const v = view;
    if (!req || !v) return;
    const { from, to } = v.state.selection.main;
    const end = req.replace ? to : from;
    v.dispatch({ changes: { from, to: end, insert: req.text }, selection: { anchor: from + req.text.length } });
    v.focus();
    onInserted();
  });
</script>

<div bind:this={host} style="height:100%"></div>
