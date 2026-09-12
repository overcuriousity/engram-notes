import type { Command } from "./commands";

export interface NoteMatch { path: string; title: string }

function score(title: string, needle: string): number {
  const t = title.toLowerCase();
  return t === needle ? 3 : t.startsWith(needle) ? 2 : t.includes(needle) ? 1 : 0;
}

/** Notes whose title or path contains the query, best title matches first. */
export function matchNotes(titles: [string, string][], query: string): NoteMatch[] {
  const needle = query.toLowerCase().trim();
  return titles
    .filter(([p, t]) => !needle || t.toLowerCase().includes(needle) || p.toLowerCase().includes(needle))
    .sort((a, b) => score(b[1], needle) - score(a[1], needle))
    .slice(0, 30)
    .map(([path, title]) => ({ path, title }));
}

/** The note name a Create entry offers, or null when a title already matches exactly. */
export function createName(matches: NoteMatch[], query: string): string | null {
  const name = query.trim();
  if (!name || matches.some((m) => m.title.toLowerCase() === name.toLowerCase())) return null;
  return name;
}

export function matchCommands(cmds: Command[], query: string): Command[] {
  const needle = query.toLowerCase().trim();
  return cmds.filter((c) => c.name.toLowerCase().includes(needle));
}
