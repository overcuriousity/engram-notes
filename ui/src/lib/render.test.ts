import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./render";

describe("renderMarkdown", () => {
  it("renders wikilinks as anchors with targets", () => {
    const h = renderMarkdown("see [[Note#Sec|shown]] and ![[pic.png]]");
    expect(h).toContain('<a class="wikilink" data-target="Note#Sec">shown</a>');
    expect(h).toContain('<img data-embed="pic.png"');
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
