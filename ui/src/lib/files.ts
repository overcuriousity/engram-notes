import type { FileEntry } from "./api";

export type FileKind = "note" | "image" | "pdf" | "base" | "graph" | "other";

// The image formats Obsidian previews.
const IMAGE = /\.(png|jpe?g|gif|bmp|svg|webp|avif)$/i;

// Graph tabs have pseudo-paths; Obsidian forbids `:` in file names, so none collides.
export function fileKind(path: string): FileKind {
  if (path.startsWith("graph:")) return "graph";
  if (/\.md$/i.test(path)) return "note";
  if (/\.base$/i.test(path)) return "base";
  if (IMAGE.test(path)) return "image";
  if (/\.pdf$/i.test(path)) return "pdf";
  return "other";
}

/** A link target as a vault file: exact path, then the shortest path with that name; case-insensitive. */
export function resolveFile(files: FileEntry[], target: string): string | null {
  const t = target.replace(/\\/g, "/").replace(/^\.?\//, "").toLowerCase();
  const exact = files.find((f) => f.path.toLowerCase() === t);
  if (exact) return exact.path;
  const name = t.slice(t.lastIndexOf("/") + 1);
  const hits = files
    .filter((f) => f.path.slice(f.path.lastIndexOf("/") + 1).toLowerCase() === name)
    .sort((a, b) => a.path.length - b.path.length || a.path.localeCompare(b.path));
  return hits[0]?.path ?? null;
}

/** Obsidian's `![[pic.png|300]]` and `|300x200`; any other alias is not a size. */
export function imageSize(alias?: string): { width?: number; height?: number } {
  const m = /^(\d+)(?:x(\d+))?$/.exec(alias?.trim() ?? "");
  if (!m) return {};
  return m[2] ? { width: Number(m[1]), height: Number(m[2]) } : { width: Number(m[1]) };
}

export function tabTitle(path: string): string {
  if (path === "") return "New tab";
  if (path === "graph:global") return "Graph view";
  if (path === "graph:local") return "Local graph";
  return path.slice(path.lastIndexOf("/") + 1).replace(/\.(md|base)$/i, "");
}

export type ImageResolver = (target: string) => string | null;

/** `![caption|300](url)`: Obsidian reads a trailing size off the alt text. */
export function altAndSize(text: string): { alt: string; width?: number; height?: number } {
  const i = text.lastIndexOf("|");
  const size = i >= 0 ? imageSize(text.slice(i + 1)) : {};
  return size.width ? { alt: text.slice(0, i), ...size } : { alt: text };
}

/** A markdown image's URL for the webview: remote URLs pass, vault paths go through `image`. */
export function imageUrl(url: string, image?: ImageResolver): string | null {
  if (/^[a-z][a-z0-9+.-]*:/i.test(url)) return url;
  let path = url;
  try {
    path = decodeURIComponent(url);
  } catch {
    // a stray % is part of the name
  }
  return image?.(path) ?? null;
}
