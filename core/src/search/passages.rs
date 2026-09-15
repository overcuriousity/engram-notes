//! A note's body as the one passage that gets embedded.

use sha2::{Digest, Sha256};

/// Roughly the model's window in characters; e5-small takes 512 tokens.
pub const MAX_CHARS: usize = 1200;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Passage {
    pub ordinal: usize,
    /// Empty since 0.5; kept so a passage row keeps its shape.
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

/// Truncate at the end to `MAX_CHARS`, preferring a whitespace boundary.
fn truncate(text: &str) -> &str {
    let Some((hard, _)) = text.char_indices().nth(MAX_CHARS) else {
        return text;
    };
    let at = text[..hard].rfind(char::is_whitespace).unwrap_or(hard);
    text[..at].trim_end()
}

/// One passage per note: the body, cut at the end. A note's important part is
/// at its beginning, and one vector per note keeps retrieval note-level.
pub fn split(body: &str) -> Vec<Passage> {
    let text = truncate(body.trim());
    if text.is_empty() {
        return vec![];
    }
    vec![Passage {
        ordinal: 0,
        heading: String::new(),
        line: 1,
        text: text.to_owned(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_one_passage_from_line_one() {
        let p = split("intro line\n\n# One\nalpha\n\n## Two\nbeta\n");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].ordinal, 0);
        assert_eq!(p[0].heading, "");
        assert_eq!(p[0].line, 1);
        assert_eq!(p[0].text, "intro line\n\n# One\nalpha\n\n## Two\nbeta");
    }

    #[test]
    fn a_long_body_is_truncated_at_the_end_on_whitespace() {
        let word = "word ".repeat(400); // 2000 chars
        let p = split(&word);
        assert_eq!(p.len(), 1);
        let n = p[0].text.chars().count();
        assert!(n <= MAX_CHARS && n >= MAX_CHARS - 6, "{n}");
        assert!(p[0].text.ends_with("word"));
    }

    #[test]
    fn a_long_word_is_cut_hard() {
        let p = split(&"x".repeat(1500));
        assert_eq!(p[0].text.chars().count(), MAX_CHARS);
    }

    #[test]
    fn the_hash_follows_the_text() {
        let a = split("# H\nbody\n");
        let b = split("# Other\nbody\n");
        assert_eq!(a[0].embed_text(), "# H\nbody");
        assert_ne!(a[0].hash(), b[0].hash());
        assert_eq!(a[0].hash(), split("# H\nbody\n")[0].hash());
    }

    #[test]
    fn an_empty_body_has_no_passages() {
        assert!(split("   \n\n").is_empty());
    }
}
