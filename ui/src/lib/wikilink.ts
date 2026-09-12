export interface WikiLink {
  target: string;
  heading?: string;
  alias?: string;
  embed: boolean;
  from: number;
  to: number;
}

const RE = /(!?)\[\[([^[\]|#]+)(?:#([^[\]|]*))?(?:\|([^[\]]*))?\]\]/g;

export function findWikilinks(text: string): WikiLink[] {
  const out: WikiLink[] = [];
  for (const m of text.matchAll(RE)) {
    out.push({
      target: m[2].trim(),
      heading: m[3]?.trim() || undefined,
      alias: m[4]?.trim() || undefined,
      embed: m[1] === "!",
      from: m.index,
      to: m.index + m[0].length,
    });
  }
  return out;
}

export function displayText(l: WikiLink): string {
  if (l.alias) return l.alias;
  return l.heading ? `${l.target} › ${l.heading}` : l.target;
}
