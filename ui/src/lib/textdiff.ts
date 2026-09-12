/** The smallest single replacement turning `a` into `b`, so an editor keeps its cursor and scroll. */
export function textDiff(a: string, b: string): { from: number; to: number; insert: string } {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++;
  return { from: start, to: a.length - end, insert: b.slice(start, b.length - end) };
}
