//! What an import could not map, and the markdown that names it.

use std::fmt::Write as _;

/// One construct kept as written or left out. `line` is 1-based; `0` means
/// the whole file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unmapped {
    pub file: String,
    pub line: usize,
    pub what: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub entries: Vec<Unmapped>,
}

impl Report {
    pub fn note(&mut self, file: &str, line: usize, what: impl Into<String>) {
        self.entries.push(Unmapped {
            file: file.to_owned(),
            line,
            what: what.into(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Markdown: the summary, then every entry under its file in line order.
    pub fn render(&self, summary: &str) -> String {
        let mut out = format!("# Import report\n\n{summary}\n");
        if self.entries.is_empty() {
            out.push_str("\nEverything was mapped.\n");
            return out;
        }
        out.push_str("\n## Not mapped\n");
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        let mut current: Option<&str> = None;
        for e in &sorted {
            if current != Some(e.file.as_str()) {
                let _ = write!(out, "\n### `{}`\n\n", e.file);
                current = Some(&e.file);
            }
            if e.line == 0 {
                let _ = writeln!(out, "- {}", e.what);
            } else {
                let _ = writeln!(out, "- line {}: {}", e.line, e.what);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_grouped_by_file_in_line_order() {
        let mut r = Report::default();
        r.note("pages/b.md", 7, "priority `[#A]` kept as text");
        r.note("pages/a.md", 0, "not imported");
        r.note("pages/b.md", 2, "block property `x:: 1` kept as text");
        let s = r.render("Imported 2 pages.");
        assert_eq!(
            s,
            "# Import report\n\nImported 2 pages.\n\n## Not mapped\n\n### `pages/a.md`\n\n- not imported\n\n### `pages/b.md`\n\n- line 2: block property `x:: 1` kept as text\n- line 7: priority `[#A]` kept as text\n"
        );
    }

    #[test]
    fn empty_report_says_so() {
        let s = Report::default().render("Imported 1 page.");
        assert_eq!(
            s,
            "# Import report\n\nImported 1 page.\n\nEverything was mapped.\n"
        );
        assert!(Report::default().is_empty());
    }
}
