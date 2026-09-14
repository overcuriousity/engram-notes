//! A Logseq page: `key:: value` lines at the top, then an outline of `- `
//! blocks indented with tabs, each block's extra lines indented two more.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Text(String),
    Property {
        line: usize,
        key: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// 1-based line of the bullet in the source.
    pub line: usize,
    pub depth: usize,
    /// The first line, after the bullet.
    pub head: String,
    pub body: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    pub properties: Vec<(String, String)>,
    /// Lines before the first block that are not page properties, verbatim.
    pub preamble: Vec<String>,
    pub blocks: Vec<Block>,
}

/// Logseq separates the key from the value with a space, so `std::mem::take`
/// in prose is not a property and must not be rewritten as one.
static PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_.\-]+)::(?:[ \t](.*))?$").unwrap());

pub fn is_property(line: &str) -> Option<(String, String)> {
    let c = PROPERTY.captures(line)?;
    let value = c.get(2).map_or("", |m| m.as_str());
    Some((c[1].to_owned(), value.trim_end().to_owned()))
}

/// Leading whitespace as outline levels: a tab is one, `indent` spaces are one.
fn level(ws: &str, indent: usize) -> usize {
    let mut depth = 0;
    let mut spaces = 0;
    for ch in ws.chars() {
        if ch == '\t' {
            depth += 1;
            spaces = 0;
        } else {
            spaces += 1;
            if spaces == indent {
                depth += 1;
                spaces = 0;
            }
        }
    }
    depth
}

fn bullet(line: &str) -> Option<(&str, &str)> {
    let ws_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[ws_len..];
    if rest == "-" {
        Some((&line[..ws_len], ""))
    } else {
        rest.strip_prefix("- ").map(|h| (&line[..ws_len], h))
    }
}

/// A continuation line without its block's indentation and the two spaces
/// Logseq adds under a bullet. What is left is the line as the writer saw it.
fn continuation(line: &str, depth: usize, indent: usize) -> &str {
    let mut rest = line;
    let unit = " ".repeat(indent);
    for _ in 0..depth {
        if let Some(r) = rest.strip_prefix('\t') {
            rest = r;
        } else if let Some(r) = rest.strip_prefix(unit.as_str()) {
            rest = r;
        } else {
            break;
        }
    }
    rest.strip_prefix("  ").unwrap_or(rest)
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

pub fn parse(text: &str, indent: usize) -> Page {
    let mut page = Page::default();
    let mut in_fence = false;
    let mut in_frontmatter = false;
    for (i, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        let n = i + 1;
        if page.blocks.is_empty() {
            if i == 0 && line == "---" {
                in_frontmatter = true;
                page.preamble.push(line.to_owned());
                continue;
            }
            if in_frontmatter {
                in_frontmatter = line != "---";
                page.preamble.push(line.to_owned());
                continue;
            }
        }
        if !in_fence && let Some((ws, head)) = bullet(line) {
            page.blocks.push(Block {
                line: n,
                depth: level(ws, indent),
                head: head.to_owned(),
                body: Vec::new(),
            });
            in_fence = is_fence(head);
            continue;
        }
        match page.blocks.last_mut() {
            Some(b) => {
                let c = continuation(line, b.depth, indent);
                if is_fence(c) {
                    in_fence = !in_fence;
                }
                b.body.push(match is_property(c) {
                    Some((key, value)) if !in_fence => Line::Property {
                        line: n,
                        key,
                        value,
                    },
                    _ => Line::Text(c.to_owned()),
                });
            }
            None => match is_property(line) {
                Some(kv) if page.preamble.is_empty() => page.properties.push(kv),
                _ => page.preamble.push(line.to_owned()),
            },
        }
    }
    // A trailing newline is not an empty last line.
    match page.blocks.last_mut() {
        Some(b) if b.body.last() == Some(&Line::Text(String::new())) => {
            b.body.pop();
        }
        None if page.preamble.last().is_some_and(|l| l.is_empty()) => {
            page.preamble.pop();
        }
        _ => {}
    }
    lift_property_block(&mut page);
    page
}

/// Older graphs wrote the page properties as the first block; they are the
/// page's when nothing else is in that block.
fn lift_property_block(page: &mut Page) {
    if !page.properties.is_empty() || !page.preamble.iter().all(|l| l.is_empty()) {
        return;
    }
    let Some(first) = page.blocks.first() else {
        return;
    };
    let Some(head) = is_property(&first.head) else {
        return;
    };
    if first.depth != 0
        || !first
            .body
            .iter()
            .all(|l| matches!(l, Line::Property { .. }))
    {
        return;
    }
    let first = page.blocks.remove(0);
    page.properties.push(head);
    for l in first.body {
        if let Line::Property { key, value, .. } = l {
            page.properties.push((key, value));
        }
    }
}

pub fn render(page: &Page, frontmatter: &str, indent: usize) -> String {
    let mut out = String::from(frontmatter);
    for l in &page.preamble {
        out.push_str(l);
        out.push('\n');
    }
    for b in &page.blocks {
        let pad = " ".repeat(b.depth * indent);
        out.push_str(&pad);
        if b.head.is_empty() {
            out.push('-');
        } else {
            out.push_str("- ");
            out.push_str(&b.head);
        }
        out.push('\n');
        for l in &b.body {
            let text = match l {
                Line::Text(t) => t.clone(),
                Line::Property { key, value, .. } if value.is_empty() => format!("{key}::"),
                Line::Property { key, value, .. } => format!("{key}:: {value}"),
            };
            if !text.is_empty() {
                out.push_str(&pad);
                out.push_str("  ");
            }
            out.push_str(&text);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_lines(b: &Block) -> Vec<&str> {
        b.body
            .iter()
            .filter_map(|l| match l {
                Line::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn page_properties_then_blocks_with_tabs() {
        let src = "title:: My Page\ntags:: a, b\n\n- first\n\tid:: 64f1a2b3-0000-4000-8000-000000000001\n\t- child\n\t  continued\n\t\t- grandchild\n- second";
        let p = parse(src, 2);
        assert_eq!(
            p.properties,
            vec![
                ("title".into(), "My Page".into()),
                ("tags".into(), "a, b".into())
            ]
        );
        assert_eq!(p.preamble, vec![""]);
        let depths: Vec<_> = p
            .blocks
            .iter()
            .map(|b| (b.depth, b.head.as_str(), b.line))
            .collect();
        assert_eq!(
            depths,
            vec![
                (0, "first", 4),
                (1, "child", 6),
                (2, "grandchild", 8),
                (0, "second", 9)
            ]
        );
        assert_eq!(
            p.blocks[0].body,
            vec![Line::Property {
                line: 5,
                key: "id".into(),
                value: "64f1a2b3-0000-4000-8000-000000000001".into()
            }]
        );
        assert_eq!(text_lines(&p.blocks[1]), vec!["continued"]);
    }

    #[test]
    fn first_block_of_properties_is_the_page_properties() {
        let p = parse("- title:: Old Style\n  public:: true\n- body", 2);
        assert_eq!(
            p.properties,
            vec![
                ("title".into(), "Old Style".into()),
                ("public".into(), "true".into())
            ]
        );
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0].head, "body");
    }

    #[test]
    fn frontmatter_and_prose_are_preamble() {
        let p = parse("---\ntitle: X\n---\n\nprose\n- item", 2);
        assert!(p.properties.is_empty());
        assert_eq!(p.preamble, vec!["---", "title: X", "---", "", "prose"]);
        assert_eq!(p.blocks[0].head, "item");
    }

    #[test]
    fn spaces_count_as_levels_and_fences_do_not_start_blocks() {
        let p = parse(
            "- a\n  - b\n    ```\n    - not a block\n    ```\n    - c",
            2,
        );
        let heads: Vec<_> = p
            .blocks
            .iter()
            .map(|b| (b.depth, b.head.as_str()))
            .collect();
        assert_eq!(heads, vec![(0, "a"), (1, "b"), (2, "c")]);
        assert_eq!(
            text_lines(&p.blocks[1]),
            vec!["```", "- not a block", "```"]
        );
    }

    #[test]
    fn render_writes_spaces_and_keeps_order() {
        let p = parse(
            "- first\n\tid:: u\n\tfoo:: bar\n\t- child\n\t  continued\n\t\t- grandchild",
            2,
        );
        let out = render(&p, "", 2);
        assert_eq!(
            out,
            "- first\n  id:: u\n  foo:: bar\n  - child\n    continued\n    - grandchild\n"
        );
        let again = parse(&out, 2);
        assert_eq!(render(&again, "", 2), out);
    }

    #[test]
    fn render_puts_frontmatter_first() {
        let p = parse("title:: T\n\n- a", 2);
        assert_eq!(
            render(&p, "---\ntitle: T\n---\n", 2),
            "---\ntitle: T\n---\n\n- a\n"
        );
    }

    #[test]
    fn a_path_with_colons_is_not_a_block_property() {
        let p = parse("- code\n  std::mem::take(&mut x);\n  foo::\n", 2);
        assert_eq!(text_lines(&p.blocks[0]), vec!["std::mem::take(&mut x);"]);
        assert_eq!(
            render(&p, "", 2),
            "- code\n  std::mem::take(&mut x);\n  foo::\n"
        );
    }

    #[test]
    fn empty_item_and_crlf() {
        let p = parse("-\r\n- x\r\n", 2);
        assert_eq!(p.blocks[0].head, "");
        assert_eq!(p.blocks[1].head, "x");
        assert_eq!(render(&p, "", 2), "-\n- x\n");
    }
}
