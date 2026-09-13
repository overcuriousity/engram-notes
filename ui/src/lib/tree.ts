import type { FileEntry } from "./api";
import { under } from "./layout";

export interface TreeFile { name: string; path: string }
export interface TreeDir { name: string; path: string; dirs: TreeDir[]; files: TreeFile[] }

export const basename = (p: string) => p.slice(p.lastIndexOf("/") + 1);
export const parent = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "");
export const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

/** The explorer's tree: every folder, even empty ones, and notes shown without `.md`. */
export function buildTree(files: FileEntry[], folders: string[]): TreeDir {
  const root: TreeDir = { name: "", path: "", dirs: [], files: [] };
  const dirAt = (path: string): TreeDir => {
    let node = root;
    if (!path) return node;
    const parts = path.split("/");
    parts.forEach((name, i) => {
      let d = node.dirs.find((x) => x.name === name);
      if (!d) {
        d = { name, path: parts.slice(0, i + 1).join("/"), dirs: [], files: [] };
        node.dirs.push(d);
      }
      node = d;
    });
    return node;
  };
  for (const dir of folders) dirAt(dir);
  for (const f of files) {
    const name = basename(f.path);
    dirAt(parent(f.path)).files.push({ name: f.is_markdown ? name.replace(/\.md$/i, "") : name, path: f.path });
  }
  const sort = (n: TreeDir) => {
    n.dirs.sort((a, b) => a.name.localeCompare(b.name));
    n.files.sort((a, b) => a.name.localeCompare(b.name));
    n.dirs.forEach(sort);
  };
  sort(root);
  return root;
}

/** Where `src` lands when dropped on folder `dir`, or null when the drop would change nothing or nest a folder in itself. */
export function dropTarget(src: string, dir: string): string | null {
  if (parent(src) === dir || under(dir, src)) return null;
  return join(dir, basename(src));
}

// The characters Obsidian refuses in a file name.
const FORBIDDEN = /[*"\\/<>:|?]/;

/** The path an inline rename moves to, or null when nothing changes. The explorer hides a note's `.md`, so it is implied. */
export function renameTarget(path: string, name: string, isNote: boolean): string | null {
  const clean = name.trim();
  if (!clean) return null;
  if (FORBIDDEN.test(clean)) throw new Error('A name cannot contain * " \\ / < > : | ?');
  const to = join(parent(path), isNote ? `${clean}.md` : clean);
  return to === path ? null : to;
}

export function freeName(taken: string[], dir: string, base: string, ext = ""): string {
  let n = join(dir, `${base}${ext}`);
  for (let i = 1; taken.includes(n); i++) n = join(dir, `${base} ${i}${ext}`);
  return n;
}

/** Obsidian badges every file that is not markdown with its extension, and shows no icon at all. */
export function fileTag(path: string): string | null {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = name.slice(dot + 1);
  return ext.toLowerCase() === "md" ? null : ext.toUpperCase();
}
