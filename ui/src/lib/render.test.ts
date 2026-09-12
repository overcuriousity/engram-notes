import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./render";

describe("renderMarkdown", () => {
  it("renders wikilinks as anchors with targets", () => {
    const h = renderMarkdown("see [[Note#Sec|shown]] and ![[Other]]");
    expect(h).toContain('<a class="wikilink" data-target="Note#Sec">shown</a>');
    expect(h).toContain('<a class="wikilink" data-target="Other">Other</a>');
  });
  it("embeds images through the resolver with Obsidian's sizes", () => {
    const image = (t: string) => (t.startsWith("gone") ? null : `asset://${t}`);
    const h = renderMarkdown(
      "![[pic.png|300]] ![[wide.png|300x200]] ![cap|120](img/my%20x.png) ![[gone.png]] ![r](https://e.com/a.png)",
      { image },
    );
    expect(h).toContain('<img class="embed" src="asset://pic.png" alt="pic.png" width="300">');
    expect(h).toContain('alt="wide.png" width="300" height="200"');
    expect(h).toContain('<img class="embed" src="asset://img/my x.png" alt="cap" width="120">');
    expect(h).toContain('<span class="embed-missing">gone.png</span>');
    expect(h).toContain('src="https://e.com/a.png"');
  });
  it("renders tags and checkboxes", () => {
    const h = renderMarkdown("- [x] done #work\n- [ ] todo");
    expect(h).toContain('<input type="checkbox" data-line="0" checked>');
    expect(h).toContain('<input type="checkbox" data-line="1">');
    expect(h).toContain('<span class="tag" data-tag="work">#work</span>');
  });
  it("renders callouts", () => {
    const h = renderMarkdown("> [!warning] Careful\n> body");
    expect(h).toContain('class="callout callout-warning"');
    expect(h).toContain('data-title="Careful"');
    expect(h).not.toContain("[!warning]");
  });
  it("hides frontmatter and keeps task line numbers", () => {
    const h = renderMarkdown("---\ntitle: T\ntags: [a]\n---\n- [ ] task");
    expect(h).not.toContain("title");
    expect(h).not.toContain("<hr>");
    expect(h).toContain('<input type="checkbox" data-line="4">');
  });
  it("escapes html", () => {
    expect(renderMarkdown("<script>x</script>")).not.toContain("<script>");
  });
  it("hides block ids and marks source lines", () => {
    const h = renderMarkdown("# Top\n\nsome text ^abc\n\n- item ^li");
    expect(h).not.toContain("^abc");
    expect(h).not.toContain("^li");
    expect(h).toContain('<h1 data-source-line="0">');
    expect(h).toContain('<p data-source-line="2">some text</p>');
  });
});
