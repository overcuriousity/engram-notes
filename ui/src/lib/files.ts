export type FileKind = "note" | "image" | "pdf" | "other";

// The image formats Obsidian previews.
const IMAGE = /\.(png|jpe?g|gif|bmp|svg|webp|avif)$/i;

export function fileKind(path: string): FileKind {
  if (/\.md$/i.test(path)) return "note";
  if (IMAGE.test(path)) return "image";
  if (/\.pdf$/i.test(path)) return "pdf";
  return "other";
}
