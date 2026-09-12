//! Markdown to `ParsedNote`. Pure: no I/O, never fails.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Wiki,
    Markdown,
    Embed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub kind: LinkKind,
    pub line: u32,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ParsedNote {
    pub title: Option<String>,
    pub frontmatter: serde_json::Map<String, serde_json::Value>,
    pub body: String,
    pub body_offset: usize,
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
}

static WIKILINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(!?)\[\[([^\[\]\|#]+)(?:#([^\[\]\|]*))?(?:\|([^\[\]]*))?\]\]").unwrap()
});
// A tag starts a word: preceded by start, whitespace or '('. Letters, digits,
// '_', '-', '/' as Obsidian allows; a tag that is only digits is not a tag.
static TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s(])#([\p{L}\p{N}_/\-]*[\p{L}_/\-][\p{L}\p{N}_/\-]*)").unwrap()
});

pub fn parse(source: &str) -> ParsedNote {
    let (frontmatter, body_offset) = split_frontmatter(source);
    let body = &source[body_offset..];
    let title = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    let (code_ranges, headings, md_links) = walk_markdown(body, body_offset, source);

    let mut links = Vec::new();
    for c in WIKILINK.captures_iter(body) {
        let m = c.get(0).unwrap();
        let start = body_offset + m.start();
        if in_ranges(start, &code_ranges) {
            continue;
        }
        links.push(Link {
            target: c[2].trim().to_owned(),
            heading: c
                .get(3)
                .map(|h| h.as_str().trim().to_owned())
                .filter(|h| !h.is_empty()),
            alias: c
                .get(4)
                .map(|a| a.as_str().trim().to_owned())
                .filter(|a| !a.is_empty()),
            kind: if &c[1] == "!" {
                LinkKind::Embed
            } else {
                LinkKind::Wiki
            },
            line: line_of(source, start),
            start,
            end: body_offset + m.end(),
        });
    }
    links.extend(md_links);
    links.sort_by_key(|l| l.start);

    let mut tags: BTreeSet<String> = BTreeSet::new();
    for c in TAG.captures_iter(body) {
        let m = c.get(1).unwrap();
        if !in_ranges(body_offset + m.start(), &code_ranges) {
            tags.insert(m.as_str().to_lowercase());
        }
    }
    if let Some(fm_tags) = frontmatter.get("tags") {
        match fm_tags {
            serde_json::Value::Array(a) => {
                for v in a {
                    if let Some(s) = v.as_str() {
                        tags.insert(s.trim_start_matches('#').to_lowercase());
                    }
                }
            }
            serde_json::Value::String(s) => {
                for t in s.split([',', ' ']).filter(|t| !t.is_empty()) {
                    tags.insert(t.trim_start_matches('#').to_lowercase());
                }
            }
            _ => {}
        }
    }

    ParsedNote {
        title,
        frontmatter,
        body: body.to_owned(),
        body_offset,
        links,
        tags: tags.into_iter().collect(),
        headings,
    }
}

fn split_frontmatter(source: &str) -> (serde_json::Map<String, serde_json::Value>, usize) {
    let empty = serde_json::Map::new();
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return (empty, 0);
    };
    let Some((yaml_end, after_fence)) = find_fence_end(rest) else {
        return (empty, 0);
    };
    let yaml = &rest[..yaml_end];
    let body_offset = source.len() - rest.len() + after_fence;
    match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml) {
        Ok(v) => match serde_json::to_value(v) {
            Ok(serde_json::Value::Object(m)) => (m, body_offset),
            _ => (empty, 0),
        },
        Err(_) => (empty, 0),
    }
}

/// Byte offset of the closing fence line in `rest`, and the offset just past
/// its newline, or `None` when there is no closing fence.
fn find_fence_end(rest: &str) -> Option<(usize, usize)> {
    let mut pos = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some((pos, pos + line.len()));
        }
        pos += line.len();
    }
    None
}

type Walked = (Vec<(usize, usize)>, Vec<Heading>, Vec<Link>);

/// Code spans and blocks as absolute byte ranges, headings, and markdown
/// links pointing at vault files.
fn walk_markdown(body: &str, body_offset: usize, source: &str) -> Walked {
    let mut code = Vec::new();
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut heading: Option<(u8, usize, String)> = None;
    let mut link: Option<(String, usize)> = None;
    let parser = Parser::new_ext(
        body,
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    );
    for (event, range) in parser.into_offset_iter() {
        let abs = (body_offset + range.start, body_offset + range.end);
        match event {
            Event::Code(ref t) => {
                code.push(abs);
                if let Some(h) = heading.as_mut() {
                    h.2.push_str(t);
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code.push(abs),
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((level as u8, abs.0, String::new()))
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, text)) = heading.take() {
                    headings.push(Heading {
                        level,
                        text: text.trim().to_owned(),
                        line: line_of(source, start),
                    });
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => link = Some((dest_url.to_string(), abs.0)),
            Event::End(TagEnd::Link) => {
                if let Some((dest, start)) = link.take()
                    && is_vault_target(&dest)
                {
                    links.push(Link {
                        target: percent_decode(&dest),
                        heading: None,
                        alias: None,
                        kind: LinkKind::Markdown,
                        line: line_of(source, start),
                        start,
                        end: abs.1,
                    });
                }
            }
            Event::Text(t) if heading.is_some() => {
                heading.as_mut().unwrap().2.push_str(&t);
            }
            _ => {}
        }
    }
    (code, headings, links)
}

fn is_vault_target(dest: &str) -> bool {
    !dest.contains("://")
        && !dest.starts_with("mailto:")
        && !dest.starts_with('#')
        && dest.to_lowercase().ends_with(".md")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn in_ranges(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| pos >= *s && pos < *e)
}

fn line_of(source: &str, offset: usize) -> u32 {
    source[..offset].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_split_from_body() {
        let src = "---\ntitle: Hello\ntags: [a, B]\n---\n# H\nbody";
        let n = parse(src);
        assert_eq!(n.title.as_deref(), Some("Hello"));
        assert_eq!(n.frontmatter["tags"], serde_json::json!(["a", "B"]));
        assert_eq!(n.body, "# H\nbody");
        assert_eq!(&src[n.body_offset..], n.body);
    }

    #[test]
    fn broken_frontmatter_stays_in_body() {
        let n = parse("---\n: : not yaml [\n---\ntext");
        assert!(n.frontmatter.is_empty());
        assert!(n.body.starts_with("---"));
    }

    #[test]
    fn wikilink_forms() {
        let src = "a [[Note]] b [[Folder/Other|shown]] c [[Third#Sec]] d ![[img.png]]";
        let n = parse(src);
        let t: Vec<_> = n
            .links
            .iter()
            .map(|l| {
                (
                    l.target.as_str(),
                    l.alias.as_deref(),
                    l.heading.as_deref(),
                    l.kind,
                )
            })
            .collect();
        assert_eq!(
            t,
            vec![
                ("Note", None, None, LinkKind::Wiki),
                ("Folder/Other", Some("shown"), None, LinkKind::Wiki),
                ("Third", None, Some("Sec"), LinkKind::Wiki),
                ("img.png", None, None, LinkKind::Embed),
            ]
        );
        assert_eq!(&src[n.links[0].start..n.links[0].end], "[[Note]]");
    }

    #[test]
    fn markdown_links_to_notes_only() {
        let n = parse("[x](Other.md) [y](https://example.com) [z](sub/My%20Note.md)");
        let t: Vec<_> = n.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(t, vec!["Other.md", "sub/My Note.md"]);
        assert_eq!(n.links[0].kind, LinkKind::Markdown);
    }

    #[test]
    fn links_in_code_are_not_links() {
        let n = parse("`[[inline]]`\n\n```\n[[fenced]]\n```\n[[real]]");
        let t: Vec<_> = n.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(t, vec!["real"]);
        assert_eq!(n.links[0].line, 6);
    }

    #[test]
    fn tags_from_body_and_frontmatter() {
        let n = parse(
            "---\ntags: [Alpha, beta]\n---\nx #Gamma/sub y #beta `#notatag` z#nope\n```\n#code\n```",
        );
        assert_eq!(n.tags, vec!["alpha", "beta", "gamma/sub"]);
    }

    #[test]
    fn headings_with_lines() {
        let n = parse("---\na: 1\n---\n# One\ntext\n## Two");
        let h: Vec<_> = n
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str(), h.line))
            .collect();
        assert_eq!(h, vec![(1, "One", 4), (2, "Two", 6)]);
    }

    #[test]
    fn line_numbers_count_frontmatter() {
        let n = parse("---\na: 1\n---\n\n[[L]]");
        assert_eq!(n.links[0].line, 5);
    }
}
