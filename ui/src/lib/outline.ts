// A markdown list as an array of lines. Every function here is a plain list
// operation on text, so a file it writes is one any markdown editor reads.

export interface Item {
  indent: number;
  marker: string;
  task: string | null;
  text: string;
}

const ITEM = /^( *)([-*+]|\d+[.)])( +)(\[[ xX]\] +)?(.*)$/;
const WS = /^[ \t]*/;
const ORDERED = /^(\s*)(\d+)([.)])(\s)/;

export const isBlank = (line: string) => line.trim() === "";

// Columns of leading whitespace, a tab counting four as CommonMark does.
export function indentOf(line: string): number {
  let n = 0;
  for (const ch of line) {
    if (ch === " ") n++;
    else if (ch === "\t") n += 4;
    else break;
  }
  return n;
}

// Characters of leading whitespace, which is what a cursor offset counts.
export const indentChars = (line: string) => WS.exec(line)![0].length;

// The indent as spaces, so a tab anywhere in it parses like any other item.
const expandIndent = (line: string) => " ".repeat(indentOf(line)) + line.slice(indentChars(line));

export function parseItem(line: string): Item | null {
  const m = ITEM.exec(expandIndent(line));
  if (!m) return null;
  return { indent: m[1].length, marker: m[2], task: m[4] ? m[4].trim() : null, text: m[5] };
}

// The line after the item and everything indented under it. A blank line
// belongs to the subtree only when something deeper follows it.
export function subtreeEnd(lines: string[], i: number): number {
  const base = indentOf(lines[i]);
  let j = i + 1;
  while (j < lines.length) {
    if (isBlank(lines[j])) {
      let k = j;
      while (k < lines.length && isBlank(lines[k])) k++;
      if (k < lines.length && indentOf(lines[k]) > base) {
        j = k;
        continue;
      }
      break;
    }
    if (indentOf(lines[j]) <= base) break;
    j++;
  }
  return j;
}

export function prevSibling(lines: string[], i: number): number | null {
  const base = indentOf(lines[i]);
  for (let j = i - 1; j >= 0; j--) {
    const l = lines[j];
    if (isBlank(l)) continue;
    const ind = indentOf(l);
    if (ind > base) continue;
    return ind === base && parseItem(l) ? j : null;
  }
  return null;
}

export function nextSibling(lines: string[], i: number): number | null {
  const base = indentOf(lines[i]);
  let k = subtreeEnd(lines, i);
  while (k < lines.length && isBlank(lines[k])) k++;
  if (k >= lines.length) return null;
  return indentOf(lines[k]) === base && parseItem(lines[k]) ? k : null;
}

// Every item line keyed "block:i.j.k" — the nth list in the document, then
// sibling indices per depth. Fold state is stored under these keys, so they
// only have to be stable across edits that do not restructure the list.
export function outlinePaths(lines: string[]): Map<number, string> {
  const out = new Map<number, string>();
  let block = -1;
  let inList = false;
  const stack: { indent: number; count: number }[] = [];
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (isBlank(l)) continue;
    const ind = indentOf(l);
    if (!parseItem(l)) {
      if (inList && stack.length && ind > stack[stack.length - 1].indent) continue;
      inList = false;
      stack.length = 0;
      continue;
    }
    if (!inList) {
      inList = true;
      block++;
    }
    // An indent that drops but stays deeper than the level above it is the same
    // depth as the level it replaces, so the replacement carries that count on.
    let popped: number | null = null;
    while (stack.length && ind < stack[stack.length - 1].indent) popped = stack.pop()!.count;
    if (!stack.length || ind > stack[stack.length - 1].indent) stack.push({ indent: ind, count: popped ?? 0 });
    stack[stack.length - 1].count++;
    out.set(i, `${block}:${stack.map((s) => s.count - 1).join(".")}`);
  }
  return out;
}

// Rewrites the leading whitespace outright, so a tab on a touched line becomes spaces.
function setIndent(line: string, n: number): string {
  return isBlank(line) ? line : " ".repeat(n) + line.replace(/^[ \t]+/, "");
}

export function indentSubtree(lines: string[], i: number, unit: number): string[] {
  const end = subtreeEnd(lines, i);
  const out = lines.slice();
  for (let j = i; j < end; j++) out[j] = setIndent(out[j], indentOf(out[j]) + unit);
  return renumber(out, i);
}

export function outdentSubtree(lines: string[], i: number, unit: number): string[] | null {
  const base = indentOf(lines[i]);
  if (base === 0) return null;
  const delta = Math.min(unit, base);
  const end = subtreeEnd(lines, i);
  const out = lines.slice();
  for (let j = i; j < end; j++) out[j] = setIndent(out[j], Math.max(0, indentOf(out[j]) - delta));
  return renumber(out, i);
}

// Swaps the subtree at `i` with its previous (-1) or next (+1) sibling's. Blank
// lines between the two stay between them.
export function moveSubtree(lines: string[], i: number, dir: -1 | 1): { lines: string[]; line: number } | null {
  if (dir < 0) {
    const p = prevSibling(lines, i);
    if (p === null) return null;
    const endA = subtreeEnd(lines, p);
    const endB = subtreeEnd(lines, i);
    const out = [...lines.slice(0, p), ...lines.slice(i, endB), ...lines.slice(endA, i), ...lines.slice(p, endA), ...lines.slice(endB)];
    return { lines: renumber(out, p), line: p };
  }
  const n = nextSibling(lines, i);
  if (n === null) return null;
  const endA = subtreeEnd(lines, i);
  const endB = subtreeEnd(lines, n);
  const out = [...lines.slice(0, i), ...lines.slice(n, endB), ...lines.slice(endA, n), ...lines.slice(i, endA), ...lines.slice(endB)];
  const line = i + (endB - endA);
  return { lines: renumber(out, line), line };
}

// Ordered items in the list block containing `at` count from one per run of
// siblings. Only that block is touched, so a list elsewhere keeps its numbers.
export function renumber(lines: string[], at: number): string[] {
  const paths = outlinePaths(lines);
  const block = paths.get(at)?.split(":")[0];
  if (block === undefined) return lines;
  const out = lines.slice();
  for (const [i, p] of paths) {
    if (!p.startsWith(`${block}:`)) continue;
    const m = ORDERED.exec(out[i]);
    if (!m) continue;
    const prev = prevSibling(out, i);
    const pm = prev === null ? null : ORDERED.exec(out[prev]);
    const n = pm ? String(Number(pm[2]) + 1) : "1";
    if (n !== m[2]) out[i] = m[1] + n + m[3] + out[i].slice(m[0].length - 1);
  }
  return out;
}
