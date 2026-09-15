//! Link targets inside a note: blocks, the best block for a query, anchors.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    Heading,
    List,
    Paragraph,
}

/// One thing a link can point at. Lines are 1-based in the body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Block {
    pub kind: BlockKind,
    pub first: u32,
    pub last: u32,
    /// The lines, `^id` tokens stripped; for a heading, its text.
    pub text: String,
    pub heading: Option<String>,
}

// Obsidian's block id: `^id` ending a line, after a space or alone on it.
static BLOCK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[ \t])\^([A-Za-z0-9-]+)[ \t\r]*$").unwrap());
static LIST_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([ \t]*)(?:[-*+]|\d+[.)])(?:[ \t]|$)").unwrap());

fn indent_width(s: &str) -> usize {
    s.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

fn indent_of(line: &str) -> usize {
    indent_width(&line[..line.len() - line.trim_start().len()])
}

fn heading_text(line: &str) -> Option<String> {
    let t = line.trim_start();
    let hashes = t.len() - t.trim_start_matches('#').len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    rest.starts_with(' ').then(|| rest.trim().to_owned())
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn strip_id(line: &str) -> &str {
    match BLOCK_ID.find(line) {
        Some(m) => line[..m.start()].trim_end(),
        None => line.trim_end(),
    }
}

/// Cut a body into headings, list items with their subtrees, and paragraphs.
/// Fenced code is skipped; a line that is only `^id` belongs to the block above.
pub fn blocks(body: &str) -> Vec<Block> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut fence: Option<u8> = None;
    while i < lines.len() {
        let line = lines[i];
        if is_fence(line) {
            let f = line.trim_start().as_bytes()[0];
            fence = match fence {
                Some(open) if open == f => None,
                other => other.or(Some(f)),
            };
            i += 1;
            continue;
        }
        if fence.is_some() || line.trim().is_empty() {
            i += 1;
            continue;
        }
        let first = i;
        let (kind, heading) = if let Some(h) = heading_text(line) {
            i += 1;
            (BlockKind::Heading, Some(h))
        } else if LIST_ITEM.is_match(line) {
            let depth = indent_of(line);
            i += 1;
            // The subtree: deeper lines, and blank lines with a deeper line after them.
            while i < lines.len() {
                let l = lines[i];
                if l.trim().is_empty() {
                    let next = lines[i + 1..].iter().find(|x| !x.trim().is_empty());
                    match next {
                        Some(n) if indent_of(n) > depth => {
                            i += 1;
                            continue;
                        }
                        _ => break,
                    }
                }
                if indent_of(l) <= depth {
                    break;
                }
                i += 1;
            }
            (BlockKind::List, None)
        } else {
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                if l.trim().is_empty()
                    || heading_text(l).is_some()
                    || LIST_ITEM.is_match(l)
                    || is_fence(l)
                {
                    break;
                }
                i += 1;
            }
            (BlockKind::Paragraph, None)
        };
        let mut last = i;
        // A lone `^id` line after the block is Obsidian's paragraph anchor.
        if last < lines.len()
            && lines[last].trim_start().starts_with('^')
            && BLOCK_ID.is_match(lines[last])
        {
            last += 1;
            i = last;
        }
        let text = match &heading {
            Some(h) => h.clone(),
            None => lines[first..last]
                .iter()
                .map(|l| strip_id(l))
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_owned(),
        };
        out.push(Block {
            kind,
            first: first as u32 + 1,
            last: last as u32,
            text,
            heading,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(b: &[Block]) -> Vec<(BlockKind, u32, u32)> {
        b.iter().map(|x| (x.kind, x.first, x.last)).collect()
    }

    #[test]
    fn headings_paragraphs_and_lists_have_their_lines() {
        let body = "# Title\n\npara one\nstill one\n\n- item\n  - child\n- sibling\n\n> quote\n| a | b |\n";
        let b = blocks(body);
        assert_eq!(
            lines(&b),
            vec![
                (BlockKind::Heading, 1, 1),
                (BlockKind::Paragraph, 3, 4),
                (BlockKind::List, 6, 7),
                (BlockKind::List, 8, 8),
                (BlockKind::Paragraph, 10, 11),
            ]
        );
        assert_eq!(b[0].heading.as_deref(), Some("Title"));
        assert_eq!(b[0].text, "Title");
        assert_eq!(b[1].text, "para one\nstill one");
        assert_eq!(b[2].text, "- item\n  - child");
    }

    #[test]
    fn a_list_item_packs_its_subtree_at_any_depth() {
        let body = "- a\n  - b\n    - c\n\n  after blank, still under a\n- d\n";
        let b = blocks(body);
        assert_eq!(
            lines(&b),
            vec![(BlockKind::List, 1, 5), (BlockKind::List, 6, 6)]
        );
    }

    #[test]
    fn fenced_code_is_skipped_and_ends_a_paragraph() {
        let body = "text\n```\n- not a list\n# not a heading\n```\nmore\n";
        let b = blocks(body);
        assert_eq!(
            lines(&b),
            vec![(BlockKind::Paragraph, 1, 1), (BlockKind::Paragraph, 6, 6)]
        );
    }

    #[test]
    fn a_trailing_id_belongs_to_its_block_and_is_stripped_from_text() {
        let body = "para ^abc123\n\nsecond\n^def456\n\n- item ^li1\n  - child\n";
        let b = blocks(body);
        assert_eq!(
            lines(&b),
            vec![
                (BlockKind::Paragraph, 1, 1),
                (BlockKind::Paragraph, 3, 4),
                (BlockKind::List, 6, 7)
            ]
        );
        assert_eq!(b[0].text, "para");
        assert_eq!(b[1].text, "second");
        assert_eq!(b[2].text, "- item\n  - child");
    }

    #[test]
    fn an_empty_body_has_no_blocks() {
        assert!(blocks("\n\n  \n").is_empty());
    }
}
