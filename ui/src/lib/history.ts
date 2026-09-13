import type { TabRef } from "./layout";

export const canBack = (t: TabRef) => (t.back?.length ?? 0) > 0;
export const canForward = (t: TabRef) => (t.fwd?.length ?? 0) > 0;

/** Navigate the tab to `path`, remembering where it was. */
export function visit(t: TabRef, path: string): void {
  if (t.path === path) return;
  t.back = [...(t.back ?? []), t.path];
  t.fwd = [];
  t.path = path;
}

export function back(t: TabRef): string | null {
  const from = t.back ?? [];
  if (from.length === 0) return null;
  t.fwd = [t.path, ...(t.fwd ?? [])];
  t.back = from.slice(0, -1);
  t.path = from[from.length - 1];
  return t.path;
}

export function forward(t: TabRef): string | null {
  const to = t.fwd ?? [];
  if (to.length === 0) return null;
  t.back = [...(t.back ?? []), t.path];
  t.fwd = to.slice(1);
  t.path = to[0];
  return t.path;
}
