import type { Snippet } from "./api";

/** The snippets to apply, in file order: those named in `enabled` that exist. */
export function activeSnippets(all: Snippet[], enabled: string[]): Snippet[] {
  const on = new Set(enabled);
  return all.filter((s) => on.has(s.name));
}

/** `enabled` with `name` switched `on` or off, order kept, no duplicates. */
export function toggleSnippet(enabled: string[], name: string, on: boolean): string[] {
  const rest = enabled.filter((n) => n !== name);
  return on ? [...rest, name] : rest;
}

/** Obsidian's `theme-dark` / `theme-light`: what a snippet can select on when the theme follows the OS. */
export function resolvedTheme(theme: "system" | "light" | "dark", prefersDark: boolean): "light" | "dark" {
  return theme === "system" ? (prefersDark ? "dark" : "light") : theme;
}
