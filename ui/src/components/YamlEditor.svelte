<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection, keymap, lineNumbers } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";
  import { yaml } from "@codemirror/lang-yaml";
  import { editorTheme } from "../editor/theme";

  let { text, onchange }: { text: string; onchange: (t: string) => void } = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          history(),
          drawSelection(),
          lineNumbers(),
          yaml(),
          editorTheme,
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange(u.state.doc.toString());
          }),
        ],
      }),
    });
    view.focus();
    return () => view.destroy();
  });
</script>

<div class="yaml" bind:this={host}></div>
