import type { SortKey } from "./api";

// A bare property id in a base file means a note property.
const norm = (p: string) => (p.includes(".") ? p : `note.${p}`);

/** A header click sorts by that column, or flips it when it already leads, as one sort key. */
export function nextSort(sort: SortKey[], property: string): SortKey[] {
  const first = sort[0];
  if (first && norm(first.property) === norm(property)) {
    return [{ property: first.property, direction: first.direction === "ASC" ? "DESC" : "ASC" }];
  }
  return [{ property, direction: "ASC" }];
}

export function sortMark(sort: SortKey[], id: string): string {
  const first = sort[0];
  if (!first || norm(first.property) !== norm(id)) return "";
  return first.direction === "DESC" ? " ↓" : " ↑";
}

export function cellText(v: unknown): string {
  if (v == null) return "";
  if (Array.isArray(v)) return v.map(cellText).join(", ");
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
