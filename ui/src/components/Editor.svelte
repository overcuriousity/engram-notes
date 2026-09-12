<script lang="ts">
  import { onMount } from "svelte";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, keymap, drawSelection, highlightActiveLine } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
  import { markdown } from "@codemirror/lang-markdown";
  import { languages } from "@codemirror/language-data";
  import { editorTheme, markdownHighlight } from "../editor/theme";
  import { livePreview } from "../editor/livePreview";
  import { completions } from "../editor/completions";

  interface Props {
    text: string;
    mode: "live" | "source";
    onchange: (t: string) => void;
    onblur: () => void;
    onFollow: (target: string) => void;
    titles: () => [string, string][];
    tags: () => string[];
  }
  let { text, mode, onchange, onblur, onFollow, titles, tags }: Props = $props();
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
          markdown({ codeLanguages: languages }),
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
    view.focus();
    return () => view?.destroy();
  });

  $effect(() => {
    view?.dispatch({ effects: modeComp.reconfigure(forMode(mode)) });
  });

  // An external reload replaces the document. Typing does not loop through
  // here because `text` then already equals the editor's content.
  $effect(() => {
    if (view && text !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
  });
</script>

<div bind:this={host} style="height:100%"></div>
