import MarkdownIt from "markdown-it";
import { findWikilinks, displayText, linkTarget } from "./wikilink";
import { altAndSize, fileKind, imageSize, imageUrl, type ImageResolver } from "./files";

const md = new MarkdownIt({ html: false, linkify: true });

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escapeAttr = (s: string) => escapeHtml(s).replace(/"/g, "&quot;");

// A type alias, so it fits markdown-it's indexable `env`.
export type RenderOptions = { image?: ImageResolver };

function imageHtml(alt: string, src: string | null, size: { width?: number; height?: number }): string {
  if (!src) return `<span class="embed-missing">${escapeHtml(alt)}</span>`;
  const w = size.width ? ` width="${size.width}"` : "";
  const h = size.height ? ` height="${size.height}"` : "";
  return `<img class="embed" src="${escapeAttr(src)}" alt="${escapeAttr(alt)}"${w}${h}>`;
}

function wikilinkHtml(raw: string, opts: RenderOptions): string {
  const l = findWikilinks(raw)[0];
  if (!l) return escapeHtml(raw);
  if (l.embed && fileKind(l.target) === "image") {
    return imageHtml(l.target, opts.image?.(l.target) ?? null, imageSize(l.alias));
  }
  return `<a class="wikilink" data-target="${escapeAttr(linkTarget(l))}">${escapeHtml(displayText(l))}</a>`;
}

// Runs before markdown-it's own link rule, so `[[...]]` is never read as a
// reference link and never escaped.
md.inline.ruler.before("link", "wikilink", (state, silent) => {
  const src = state.src;
  const start = src.charAt(state.pos) === "!" ? state.pos + 1 : state.pos;
  if (src.slice(start, start + 2) !== "[[") return false;
  const end = src.indexOf("]]", start + 2);
  if (end < 0) return false;
  if (!silent) {
    const tok = state.push("html_inline", "", 0);
    tok.content = wikilinkHtml(src.slice(state.pos, end + 2), state.env as RenderOptions);
  }
  state.pos = end + 2;
  return true;
});

// Task items become checkboxes that carry their source line. Registered
// before `tags`, which turns a text token holding a tag into html.
md.core.ruler.push("tasks", (state) => {
  let itemLine = -1;
  for (const tok of state.tokens) {
    if (tok.type === "list_item_open") itemLine = tok.map?.[0] ?? -1;
    if (tok.type === "inline" && itemLine >= 0 && tok.children?.[0]?.type === "text") {
      const first = tok.children[0];
      const m = /^\[([ xX])\]\s/.exec(first.content);
      if (m) {
        first.content = first.content.slice(m[0].length);
        const cb = new state.Token("html_inline", "", 0);
        cb.content = `<input type="checkbox" data-line="${itemLine}"${m[1] === " " ? "" : " checked"}> `;
        tok.children.unshift(cb);
      }
      itemLine = -1;
    }
  }
});

const TAG = /(^|[\s(])#([\p{L}\p{N}_/-]*[\p{L}_/-][\p{L}\p{N}_/-]*)/gu;
md.core.ruler.push("tags", (state) => {
  for (const block of state.tokens) {
    if (block.type !== "inline" || !block.children) continue;
    for (const t of block.children) {
      if (t.type !== "text" || !t.content.includes("#")) continue;
      const plain = escapeHtml(t.content);
      const html = plain.replace(TAG, '$1<span class="tag" data-tag="$2">#$2</span>');
      if (html !== plain) {
        t.type = "html_inline";
        t.content = html;
      }
    }
  }
});

// Callouts: a blockquote whose paragraph starts with `[!type] title`.
// Runs before inline parsing so the stripped marker never reaches it.
md.core.ruler.before("inline", "callouts", (state) => {
  for (let i = 0; i < state.tokens.length - 2; i++) {
    const open = state.tokens[i];
    const inline = state.tokens[i + 2];
    if (open.type !== "blockquote_open" || inline.type !== "inline") continue;
    const m = /^\[!(\w+)\]\s*([^\n]*)\n?/.exec(inline.content);
    if (!m) continue;
    open.attrSet("class", `callout callout-${m[1].toLowerCase()}`);
    open.attrSet("data-title", m[2] || m[1]);
    inline.content = inline.content.slice(m[0].length);
  }
});

// Obsidian hides `^id` in reading view; the id stays in the file. Runs
// before `tags`, which turns text tokens holding a tag into html.
md.core.ruler.before("tags", "block_ids", (state) => {
  for (const tok of state.tokens) {
    const kids = tok.type === "inline" ? tok.children : null;
    const last = kids?.[kids.length - 1];
    if (last?.type === "text") last.content = last.content.replace(/(^|\s)\^[A-Za-z0-9-]+\s*$/, "");
  }
});

// Block elements carry their 0-based source line so a jump can find them.
md.core.ruler.push("source_lines", (state) => {
  for (const tok of state.tokens) {
    if (tok.map && tok.nesting === 1) tok.attrSet("data-source-line", String(tok.map[0]));
  }
});

// Frontmatter lines become blank lines, so rendered task lines keep their
// source line numbers.
function blankFrontmatter(text: string): string {
  const m = /^---\r?\n[\s\S]*?\r?\n---(?=\r?\n|$)/.exec(text);
  return m ? m[0].replace(/[^\n]/g, "") + text.slice(m[0].length) : text;
}

// Vault paths in markdown images load through the same resolver as embeds.
md.renderer.rules.image = (tokens, idx, options, env) => {
  const tok = tokens[idx];
  const url = String(tok.attrGet("src") ?? "");
  const text = md.renderer.renderInlineAsText(tok.children ?? [], options, env);
  const { alt, ...size } = altAndSize(text);
  return imageHtml(alt || url, imageUrl(url, (env as RenderOptions | undefined)?.image), size);
};

export function renderMarkdown(text: string, opts: RenderOptions = {}): string {
  return md.render(blankFrontmatter(text), opts);
}
