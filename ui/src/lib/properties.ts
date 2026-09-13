export type PropKind = "checkbox" | "number" | "date" | "datetime" | "list" | "text";

/** The editor a value gets; the same rule as `value_type` in the index. */
export function propKind(v: unknown): PropKind {
  if (typeof v === "boolean") return "checkbox";
  if (typeof v === "number") return "number";
  if (Array.isArray(v)) return "list";
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}$/.test(v)) return "date";
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(v)) return "datetime";
  return "text";
}

/** A value typed into the new-property row; empty means no value. */
export function parseValue(raw: string): unknown {
  if (raw === "") return null;
  if (raw === "true") return true;
  if (raw === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(raw)) return Number(raw);
  if (raw.startsWith("[")) {
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  return raw;
}

export function addItem(list: unknown[], raw: string): unknown[] {
  const item = raw.trim();
  return item && !list.includes(item) ? [...list, item] : list;
}

export function removeItem(list: unknown[], index: number): unknown[] {
  return list.filter((_, i) => i !== index);
}

export type PropIcon =
  | "text" | "binary" | "calendar" | "clock" | "check-square" | "list" | "tags" | "forward";

const BY_KIND: Record<PropKind, PropIcon> = {
  text: "text",
  number: "binary",
  date: "calendar",
  datetime: "clock",
  checkbox: "check-square",
  list: "list",
};

// Obsidian's widget registry keys these two by name, whatever the value holds.
const BY_KEY: Record<string, PropIcon> = { tags: "tags", aliases: "forward" };

/** The Lucide icon Obsidian puts on a property row. */
export function propIcon(key: string, value: unknown): PropIcon {
  return BY_KEY[key] ?? BY_KIND[propKind(value)];
}
