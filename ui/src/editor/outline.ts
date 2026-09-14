import type { Extension } from "@codemirror/state";
import { insertNewlineContinueMarkupCommand, markdownLanguage } from "@codemirror/lang-markdown";

// closeBrackets reads the pairing set from language data. Obsidian pairs the
// emphasis characters as well as brackets, and wraps a selection with any of them.
export const markdownBrackets: Extension = markdownLanguage.data.of({
  closeBrackets: { brackets: ["(", "[", "{", "'", '"', "*", "_", "`"] },
});

// Enter continues the list; on an empty item it drops one level of markup, as
// Obsidian does. The default would make a two-item list loose instead.
export const markdownEnter = insertNewlineContinueMarkupCommand({ nonTightLists: false });
