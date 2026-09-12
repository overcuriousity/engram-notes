import MarkdownIt from "markdown-it";
import { findWikilinks, displayText } from "./wikilink";

const md = new MarkdownIt({ html: false, linkify: true });

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escapeAttr = (s: string) => escapeHtml(s).replace(/"/g, "&quot;");

function wikilinkHtml(raw: string): string {
  const l = findWikilinks(raw)[0];
  if (!l) return escapeHtml(raw);
  const target = l.heading ? `${l.target}#${l.heading}` : l.target;
  if (l.embed && /\.(png|jpe?g|gif|svg|webp)$/i.test(l.target)) {
    return `<img data-embed="${escapeAttr(l.target)}" alt="${escapeAttr(l.target)}">`;
  }
  return `<a class="wikilink" data-target="${escapeAttr(target)}">${escapeHtml(displayText(l))}</a>`;
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
    tok.content = wikilinkHtml(src.slice(state.pos, end + 2));
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

// Frontmatter lines become blank lines, so rendered task lines keep their
// source line numbers.
function blankFrontmatter(text: string): string {
  const m = /^---\r?\n[\s\S]*?\r?\n---(?=\r?\n|$)/.exec(text);
  return m ? m[0].replace(/[^\n]/g, "") + text.slice(m[0].length) : text;
}

export function renderMarkdown(text: string): string {
  return md.render(blankFrontmatter(text));
}
