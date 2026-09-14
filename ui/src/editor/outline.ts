import type { Extension } from "@codemirror/state";
import { markdownLanguage } from "@codemirror/lang-markdown";

// closeBrackets reads the pairing set from language data. Obsidian pairs the
// emphasis characters as well as brackets, and wraps a selection with any of them.
export const markdownBrackets: Extension = markdownLanguage.data.of({
  closeBrackets: { brackets: ["(", "[", "{", "'", '"', "*", "_", "`"] },
});
