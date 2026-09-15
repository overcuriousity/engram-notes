//! Link targets inside a note: blocks, the best block for a query, anchors.

use crate::{Error, Result};
use regex::Regex;
use std::collections::HashSet;
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

/// A line from `split_inclusive('\n')` as its content and its line ending.
fn split_eol(line: &str) -> (&str, &str) {
    match line.strip_suffix("\r\n") {
        Some(c) => (c, "\r\n"),
        None => match line.strip_suffix('\n') {
            Some(c) => (c, "\n"),
            None => (line, ""),
        },
    }
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

/// Characters `[[Note#…]]` cannot carry: `|` would start an alias, `#` a
/// deeper heading, `^` a block id, and a bracket would end the link.
const UNWRITABLE: [char; 5] = ['[', ']', '|', '#', '^'];

/// A heading's text as a link can spell it, whitespace collapsed.
pub fn heading_fragment(text: &str) -> String {
    text.split_whitespace()
        .map(|w| w.replace(UNWRITABLE, ""))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// How a heading and a link fragment are compared: on what a link can spell,
/// so a heading holding `|` still resolves from the link written for it.
pub fn heading_key(text: &str) -> String {
    heading_fragment(text).to_lowercase()
}

/// Query terms: lowercased words of two characters or more.
fn terms(query: &str) -> Vec<String> {
    let mut t: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(|w| w.to_lowercase())
        .collect();
    t.sort();
    t.dedup();
    t
}

/// The block with the most distinct query terms in it; ties to the earlier.
pub fn best_block(blocks: &[Block], query: &str) -> Option<usize> {
    let terms = terms(query);
    if terms.is_empty() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    for (i, b) in blocks.iter().enumerate() {
        let text = b.text.to_lowercase();
        let n = terms.iter().filter(|t| text.contains(t.as_str())).count();
        if n > 0 && best.is_none_or(|(_, m)| n > m) {
            best = Some((i, n));
        }
    }
    best.map(|(i, _)| i)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchored {
    pub text: String,
    pub id: String,
    pub changed: bool,
}

/// Every `^id` already in the file, for uniqueness.
pub fn existing_ids(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|l| BLOCK_ID.captures(l).map(|c| c[1].to_owned()))
        .collect()
}

/// Append ` ^id` to the block's anchor line, or return the id it already has.
/// `body_line` is the file line the body starts on. Nothing else changes.
pub fn anchor(
    text: &str,
    body_line: u32,
    block: &Block,
    fresh: &mut dyn FnMut() -> String,
) -> Result<Anchored> {
    if block.kind == BlockKind::Heading {
        return Err(Error::NotFound("heading blocks take no anchor".into()));
    }
    // Obsidian keys a list item by its own line and a paragraph by its last.
    let body_target = match block.kind {
        BlockKind::List => block.first,
        _ => block.last,
    };
    let target = (body_target + body_line) as usize - 2;
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let line = *lines
        .get(target)
        .ok_or_else(|| Error::NotFound("block is outside the file".into()))?;
    let (content, eol) = split_eol(line);
    // The anchor line, or the lone `^id` line `blocks` absorbed into this block:
    // either already names it, and a second id for one block would break the link.
    let own = BLOCK_ID.captures(content).or_else(|| {
        let tail = (block.last + body_line) as usize - 2;
        lines
            .get(tail)
            .copied()
            .map(split_eol)
            .filter(|(l, _)| tail != target && l.trim_start().starts_with('^'))
            .and_then(|(l, _)| BLOCK_ID.captures(l))
    });
    if let Some(c) = own {
        return Ok(Anchored {
            text: text.to_owned(),
            id: c[1].to_owned(),
            changed: false,
        });
    }
    let used = existing_ids(text);
    let mut id = fresh();
    while used.contains(&id) {
        id = fresh();
    }
    let new_line = format!("{} ^{id}{eol}", content.trim_end());
    lines[target] = &new_line;
    Ok(Anchored {
        text: lines.concat(),
        id,
        changed: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Preview {
    pub heading: String,
    pub text: String,
}

const PREVIEW_LINES: usize = 40;

fn heading_level(line: &str) -> usize {
    let t = line.trim_start();
    t.len() - t.trim_start_matches('#').len()
}

fn heading_path(body: &str, upto_line: u32) -> String {
    let mut stack: Vec<(usize, String)> = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if i as u32 + 1 >= upto_line {
            break;
        }
        if let Some(h) = heading_text(line) {
            let level = heading_level(line);
            stack.retain(|(l, _)| *l < level);
            stack.push((level, h));
        }
    }
    stack
        .into_iter()
        .map(|(_, h)| h)
        .collect::<Vec<_>>()
        .join(" > ")
}

fn cap(text: String) -> String {
    text.lines()
        .take(PREVIEW_LINES)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text a link points at: a block by `^id`, a heading's section, or the
/// note's first block.
pub fn preview(body: &str, fragment: Option<&str>) -> Option<Preview> {
    let all = blocks(body);
    match fragment {
        Some(f) if f.starts_with('^') => {
            let id = &f[1..];
            let line = body.lines().position(|l| {
                BLOCK_ID
                    .captures(l)
                    .is_some_and(|c| c[1].eq_ignore_ascii_case(id))
            })? as u32
                + 1;
            let b = all.iter().find(|b| b.first <= line && line <= b.last)?;
            Some(Preview {
                heading: heading_path(body, b.first),
                text: cap(b.text.clone()),
            })
        }
        Some(f) => {
            // `[[Note#A#B]]` names heading B under A; the last part finds it.
            let want = heading_key(f.rsplit('#').next().unwrap_or(f));
            let i = all
                .iter()
                .position(|b| b.heading.as_deref().is_some_and(|h| heading_key(h) == want))?;
            let level = |b: &Block| {
                body.lines()
                    .nth(b.first as usize - 1)
                    .map_or(7, heading_level)
            };
            let own = level(&all[i]);
            let end = all[i + 1..]
                .iter()
                .find(|b| b.kind == BlockKind::Heading && level(b) <= own)
                .map_or(body.lines().count() as u32, |b| b.first - 1);
            let text = body
                .lines()
                .skip(all[i].first as usize - 1)
                .take((end - all[i].first + 1) as usize)
                .map(strip_id)
                .collect::<Vec<_>>()
                .join("\n");
            Some(Preview {
                heading: heading_path(body, all[i].first),
                text: cap(text.trim_end().to_owned()),
            })
        }
        None => {
            let b = all.first()?;
            Some(Preview {
                heading: String::new(),
                text: cap(b.text.clone()),
            })
        }
    }
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

    #[test]
    fn best_block_is_the_one_with_most_query_terms() {
        let b = blocks(
            "# Shells\n\nthe carousel of shell companies\n\n- VAT fraud chain\n- carousel fraud in the EU\n",
        );
        assert_eq!(best_block(&b, "carousel fraud"), Some(3));
        assert_eq!(best_block(&b, "Shell Companies"), Some(1));
    }

    #[test]
    fn best_block_ties_go_to_the_earlier_and_no_match_is_none() {
        let b = blocks("alpha beta\n\nbeta alpha\n");
        assert_eq!(best_block(&b, "alpha"), Some(0));
        assert_eq!(best_block(&b, "gamma"), None);
        assert_eq!(best_block(&b, "a"), None);
    }

    fn ids(seq: &[&str]) -> impl FnMut() -> String {
        let v: Vec<String> = seq.iter().map(|s| s.to_string()).collect();
        let mut i = 0;
        move || {
            let s = v[i.min(v.len() - 1)].clone();
            i += 1;
            s
        }
    }

    #[test]
    fn a_paragraph_anchor_goes_on_its_last_line() {
        let text = "---\nk: v\n---\npara one\nstill one\n\nnext\n";
        let b = blocks("para one\nstill one\n\nnext\n");
        let a = anchor(text, 4, &b[0], &mut ids(&["ab12cd"])).unwrap();
        assert_eq!(
            a.text,
            "---\nk: v\n---\npara one\nstill one ^ab12cd\n\nnext\n"
        );
        assert_eq!(a.id, "ab12cd");
        assert!(a.changed);
    }

    #[test]
    fn a_list_anchor_goes_on_its_first_line() {
        let text = "- item\n  - child\n- next\n";
        let b = blocks(text);
        let a = anchor(text, 1, &b[0], &mut ids(&["zz9999"])).unwrap();
        assert_eq!(a.text, "- item ^zz9999\n  - child\n- next\n");
    }

    #[test]
    fn an_existing_id_is_reused_and_nothing_changes() {
        let text = "para ^keep01\r\n\r\nother\r\n";
        let b = blocks(text);
        let a = anchor(text, 1, &b[0], &mut ids(&["new001"])).unwrap();
        assert_eq!(a.text, text);
        assert_eq!(a.id, "keep01");
        assert!(!a.changed);
    }

    #[test]
    fn an_id_on_the_blocks_absorbed_line_is_reused() {
        let text = "- item\n^abc123\n- next\n";
        let b = blocks(text);
        assert_eq!(
            lines(&b),
            vec![(BlockKind::List, 1, 2), (BlockKind::List, 3, 3)]
        );
        let a = anchor(text, 1, &b[0], &mut ids(&["new001"])).unwrap();
        assert_eq!(a.text, text);
        assert_eq!(a.id, "abc123");
        assert!(!a.changed);
    }

    #[test]
    fn a_childs_own_id_is_not_taken_for_the_item_above_it() {
        let text = "- item\n  - child ^kid001\n";
        let b = blocks(text);
        let a = anchor(text, 1, &b[0], &mut ids(&["new001"])).unwrap();
        assert_eq!(a.text, "- item ^new001\n  - child ^kid001\n");
        assert_eq!(a.id, "new001");
    }

    #[test]
    fn a_colliding_id_is_drawn_again_and_crlf_is_kept() {
        let text = "one ^ab12cd\r\n\r\ntwo\r\n";
        let b = blocks("one ^ab12cd\n\ntwo\n");
        let a = anchor(text, 1, &b[1], &mut ids(&["ab12cd", "ab12cd", "ef34gh"])).unwrap();
        assert_eq!(a.text, "one ^ab12cd\r\n\r\ntwo ^ef34gh\r\n");
        assert_eq!(a.id, "ef34gh");
    }

    #[test]
    fn a_heading_takes_no_anchor() {
        let text = "# H\nbody\n";
        let b = blocks(text);
        assert!(matches!(
            anchor(text, 1, &b[0], &mut ids(&["x"])),
            Err(crate::Error::NotFound(_))
        ));
    }

    #[test]
    fn a_block_past_the_end_is_not_found() {
        let b = Block {
            kind: BlockKind::Paragraph,
            first: 9,
            last: 9,
            text: "x".into(),
            heading: None,
        };
        assert!(matches!(
            anchor("a\n", 1, &b, &mut ids(&["x"])),
            Err(crate::Error::NotFound(_))
        ));
    }

    #[test]
    fn preview_resolves_block_heading_and_bare_targets() {
        let body = "# A\nintro\n\n## B\nunder b ^blk1\nmore\n\n## C\nunder c\n\n# D\nlast\n";
        let p = preview(body, Some("^blk1")).unwrap();
        assert_eq!(p.heading, "A > B");
        assert_eq!(p.text, "under b\nmore");
        let p = preview(body, Some("B")).unwrap();
        assert_eq!(p.heading, "A");
        assert_eq!(p.text, "## B\nunder b\nmore");
        let p = preview(body, None).unwrap();
        assert_eq!(p.heading, "");
        assert_eq!(p.text, "A");
        assert!(preview(body, Some("^nope")).is_none());
        assert_eq!(
            preview(body, Some("A#B")).unwrap().text,
            "## B\nunder b\nmore"
        );
    }

    #[test]
    fn preview_is_capped_at_forty_lines() {
        let body = (1..=60)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let p = preview(&body, None).unwrap();
        assert_eq!(p.text.lines().count(), 40);
    }

    #[test]
    fn a_heading_a_link_cannot_spell_is_written_and_matched_without_those_chars() {
        assert_eq!(heading_fragment("Pros | Cons"), "Pros Cons");
        assert_eq!(heading_fragment("a [b] #c ^d"), "a b c d");
        assert_eq!(heading_fragment("  keeps   one space "), "keeps one space");
        assert_eq!(heading_key("Pros | Cons"), "pros cons");
        let body = "# A
intro

## Pros | Cons
weighed
";
        let p = preview(body, Some("Pros Cons")).unwrap();
        assert_eq!(p.heading, "A");
        assert_eq!(
            p.text,
            "## Pros | Cons
weighed"
        );
    }
}
