//! A note's body cut into the pieces that get embedded.

use sha2::{Digest, Sha256};

/// Roughly the model's window in characters; e5-small takes 512 tokens.
pub const MAX_CHARS: usize = 1200;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Passage {
    pub ordinal: usize,
    /// The headings above it, joined with ` > `; empty above the first.
    pub heading: String,
    /// 1-based line in the body, not in the file.
    pub line: u32,
    pub text: String,
}

impl Passage {
    /// What is embedded: the heading path gives the passage its context.
    pub fn embed_text(&self) -> String {
        if self.heading.is_empty() {
            self.text.clone()
        } else {
            format!("{}\n\n{}", self.heading, self.text)
        }
    }

    pub fn hash(&self) -> String {
        hex::encode(Sha256::digest(self.embed_text().as_bytes()))
    }
}

fn heading_of(line: &str) -> Option<(u8, String)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    rest.starts_with(' ')
        .then(|| (hashes as u8, rest.trim().to_owned()))
}

fn path_of(stack: &[(u8, String)]) -> String {
    stack
        .iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join(" > ")
}

fn flush(buf: &mut String, line: u32, heading: &str, out: &mut Vec<Passage>) {
    let text = buf.trim();
    if !text.is_empty() {
        for piece in cut(text) {
            out.push(Passage {
                ordinal: out.len(),
                heading: heading.to_owned(),
                line,
                text: piece.to_owned(),
            });
        }
    }
    buf.clear();
}

/// Cut `text` so no piece is longer than `MAX_CHARS`, preferring whitespace.
fn cut(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.chars().count() > MAX_CHARS {
        let hard = rest
            .char_indices()
            .nth(MAX_CHARS)
            .map_or(rest.len(), |(i, _)| i);
        let at = rest[..hard].rfind(char::is_whitespace).unwrap_or(hard);
        let (head, tail) = rest.split_at(at);
        out.push(head.trim_end());
        rest = tail.trim_start();
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

/// Split on headings, then on paragraphs, into pieces of at most `MAX_CHARS`.
pub fn split(body: &str) -> Vec<Passage> {
    let mut out: Vec<Passage> = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut buf = String::new();
    let mut buf_line = 0u32;
    let mut fence: Option<u8> = None;

    for (i, line) in body.lines().enumerate() {
        let no = i as u32 + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let f = trimmed.as_bytes()[0];
            fence = match fence {
                Some(open) if open == f => None,
                other => other.or(Some(f)),
            };
        }
        if fence.is_none() {
            if let Some((level, text)) = heading_of(trimmed) {
                flush(&mut buf, buf_line, &path_of(&stack), &mut out);
                stack.retain(|(l, _)| *l < level);
                stack.push((level, text));
                buf_line = 0;
                continue;
            }
            // A blank line closes a paragraph; a full buffer closes it early.
            if line.trim().is_empty() {
                if buf.chars().count() >= MAX_CHARS / 2 {
                    flush(&mut buf, buf_line, &path_of(&stack), &mut out);
                    buf_line = 0;
                }
                if !buf.is_empty() {
                    buf.push('\n');
                }
                continue;
            }
        }
        if buf_line == 0 {
            buf_line = no;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    flush(&mut buf, buf_line, &path_of(&stack), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_group_under_their_heading_path() {
        let body = "intro line\n\n# One\nalpha\n\n## Two\nbeta\n\ngamma\n";
        let p = split(body);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].heading, "");
        assert_eq!(p[0].text, "intro line");
        assert_eq!(p[0].line, 1);
        assert_eq!(p[1].heading, "One");
        assert_eq!(p[1].text, "alpha");
        assert_eq!(p[1].line, 4);
        assert_eq!(p[2].heading, "One > Two");
        assert_eq!(p[2].text, "beta\n\ngamma");
        assert_eq!(p[2].ordinal, 2);
    }

    #[test]
    fn a_hash_inside_a_fence_is_not_a_heading() {
        let body = "# H\n```\n# not a heading\n```\ntail\n";
        let p = split(body);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "H");
        assert!(p[0].text.contains("# not a heading"));
    }

    #[test]
    fn long_text_is_cut_at_whitespace_below_the_limit() {
        let word = "word ".repeat(400); // 2000 chars
        let p = split(&word);
        assert!(p.len() >= 2);
        assert!(p.iter().all(|x| x.text.chars().count() <= MAX_CHARS));
        assert_eq!(p[1].ordinal, 1);
    }

    #[test]
    fn the_heading_path_is_embedded_and_hashed_with_the_text() {
        let a = split("# H\nbody\n");
        let b = split("# Other\nbody\n");
        assert_eq!(a[0].embed_text(), "H\n\nbody");
        assert_ne!(a[0].hash(), b[0].hash());
        assert_eq!(a[0].hash(), split("# H\nbody\n")[0].hash());
    }

    #[test]
    fn an_empty_body_has_no_passages() {
        assert!(split("   \n\n").is_empty());
    }
}
