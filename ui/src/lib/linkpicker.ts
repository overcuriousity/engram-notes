import type { Block } from "./api";

export function stemOf(path: string): string {
  return path.split("/").pop()!.replace(/\.md$/i, "");
}

/**
 * A heading's text as a link can spell it: `|` would start an alias, `#` a
 * deeper heading, `^` a block id, a bracket would end the link. Core drops the
 * same characters when it resolves the fragment, so the link still follows.
 */
export function headingFragment(text: string): string {
  return text.split(/\s+/).map((w) => w.replace(/[[\]|#^]/g, "")).filter(Boolean).join(" ");
}

/** The link as written: Obsidian's form, the alias after `|` when there is one. */
export function linkText(path: string, block: Block, id: string | null, alias: string | null): string {
  const fragment = block.kind === "heading" ? headingFragment(block.heading ?? block.text) : `^${id ?? ""}`;
  const tail = alias ? `|${alias}` : "";
  // A heading of nothing but those characters leaves no fragment to point at,
  // and `[[Note#]]` reads as the note anyway.
  if (!fragment) return `[[${stemOf(path)}${tail}]]`;
  return `[[${stemOf(path)}#${fragment}${tail}]]`;
}

const TRIGGER = /\[\[\^\^([^\]]*)$/;

/** Obsidian's vault-wide block search: `[[^^` and what was typed after it. */
export function blockTrigger(textBefore: string): { from: number; query: string } | null {
  const m = TRIGGER.exec(textBefore);
  return m ? { from: m.index, query: m[1] } : null;
}

/** What a block row shows: its first line, without a list marker. */
export function firstLine(block: Block): string {
  const line = block.text.split("\n")[0] ?? "";
  return line.replace(/^\s*(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?/, "").trim();
}
