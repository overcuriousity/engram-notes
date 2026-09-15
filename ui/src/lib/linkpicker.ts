import type { Block } from "./api";

export function stemOf(path: string): string {
  return path.split("/").pop()!.replace(/\.md$/i, "");
}

/** The link as written: Obsidian's form, the alias after `|` when there is one. */
export function linkText(path: string, block: Block, id: string | null, alias: string | null): string {
  const fragment = block.kind === "heading" ? (block.heading ?? block.text) : `^${id ?? ""}`;
  const tail = alias ? `|${alias}` : "";
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
