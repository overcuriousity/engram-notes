export interface WikiLink {
  target: string;
  heading?: string;
  block?: string;
  alias?: string;
  embed: boolean;
  from: number;
  to: number;
}

const RE = /(!?)\[\[([^[\]|#]+)(?:#([^[\]|]*))?(?:\|([^[\]]*))?\]\]/g;

export function findWikilinks(text: string): WikiLink[] {
  const out: WikiLink[] = [];
  for (const m of text.matchAll(RE)) {
    const fragment = m[3]?.trim() || undefined;
    const block = fragment?.startsWith("^") ? fragment.slice(1) : undefined;
    out.push({
      target: m[2].trim(),
      heading: block ? undefined : fragment,
      block,
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
  if (l.block) return `${l.target} › ^${l.block}`;
  return l.heading ? `${l.target} › ${l.heading}` : l.target;
}

/** The link as `follow` takes it: target plus `#heading` or `#^block`. */
export function linkTarget(l: WikiLink): string {
  if (l.block) return `${l.target}#^${l.block}`;
  return l.heading ? `${l.target}#${l.heading}` : l.target;
}
