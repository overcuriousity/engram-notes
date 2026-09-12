<script lang="ts">
  import { onMount } from "svelte";
  import { Compartment, EditorState, Transaction } from "@codemirror/state";
  import { EditorView, keymap, drawSelection, highlightActiveLine } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
  import { yamlFrontmatter } from "@codemirror/lang-yaml";
  import { languages } from "@codemirror/language-data";
  import { editorTheme, markdownHighlight } from "../editor/theme";
  import { livePreview } from "../editor/livePreview";
  import { completions } from "../editor/completions";
  import { textDiff } from "../lib/textdiff";

  interface Props {
    text: string;
    mode: "live" | "source";
    focus: boolean;
    onchange: (t: string) => void;
    onblur: () => void;
    onFollow: (target: string) => void;
    titles: () => [string, string][];
    tags: () => string[];
  }
  let { text, mode, focus, onchange, onblur, onFollow, titles, tags }: Props = $props();
  let host: HTMLDivElement;
  let view: EditorView | undefined;
  const modeComp = new Compartment();
  const forMode = (m: string) => (m === "live" ? livePreview({ onFollow }) : []);

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          // GFM for tasks and strikethrough; YAML so frontmatter is not read as a heading.
          yamlFrontmatter({ content: markdown({ base: markdownLanguage, codeLanguages: languages }) }),
          editorTheme,
          markdownHighlight,
          EditorView.lineWrapping,
          modeComp.of(forMode(mode)),
          completions(titles, tags),
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString());
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
    return () => view?.destroy();
  });

  $effect(() => {
    view?.dispatch({ effects: modeComp.reconfigure(forMode(mode)) });
  });

  // Another pane's typing or an outside reload changed the note. Only the
  // difference is applied, so this editor keeps its cursor and scroll.
  $effect(() => {
    if (!view) return;
    const current = view.state.doc.toString();
    if (text !== current) {
      view.dispatch({ changes: textDiff(current, text), annotations: Transaction.addToHistory.of(false) });
    }
  });
</script>

<div bind:this={host} style="height:100%"></div>
