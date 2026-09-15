//! Logseq's inline syntax into Obsidian's, one line at a time. Anything with
//! no Obsidian form stays as written and goes into the report.

use crate::import::Report;
use regex::{Captures, Regex};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// uuid (lowercase) → (page link name, anchor without `^`, empty when the
/// uuid is the page's own and the reference is to the page).
pub type Refs = HashMap<String, (String, String)>;

const UUID: &str = r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

static EMBED_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\{{\{{embed \(\(({UUID})\)\)\}}\}}")).unwrap());
static EMBED_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{embed \[\[([^\]]+)\]\]\}\}").unwrap());
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"\(\(({UUID})\)\)")).unwrap());
static TAG_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[\[([^\]]+)\]\]").unwrap());
static IMAGE_SIZE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!\[([^\]]*)\]\(([^)]*)\)\{:(height|width) (\d+), :(height|width) (\d+)\}").unwrap()
});
static TASK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(TODO|DONE|DOING|LATER|NOW|WAITING|CANCELED|CANCELLED)(?: |$)").unwrap()
});
static PRIORITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[#[A-C]\]").unwrap());

fn link(refs: &Refs, uuid: &str, embed: bool) -> Option<String> {
    let (page, anchor) = refs.get(&uuid.to_lowercase())?;
    let bang = if embed { "!" } else { "" };
    let block = if anchor.is_empty() {
        String::new()
    } else {
        format!("#^{anchor}")
    };
    Some(format!("{bang}[[{page}{block}]]"))
}

pub fn rewrite(text: &str, refs: &Refs, file: &str, line: usize, report: &mut Report) -> String {
    let mut unknown = Vec::new();
    // An unresolved `{{embed ((uuid))}}` is left as written, so the plain
    // reference pass sees it again; a uuid is reported once per line.
    let mut seen = HashSet::new();
    let mut resolve = |c: &Captures, embed: bool| match link(refs, &c[1], embed) {
        Some(l) => l,
        None => {
            if seen.insert(c[1].to_lowercase()) {
                unknown.push(c[0].to_owned());
            }
            c[0].to_owned()
        }
    };
    let s = EMBED_REF.replace_all(text, |c: &Captures| resolve(c, true));
    let s = REF.replace_all(&s, |c: &Captures| resolve(c, false));
    for u in unknown {
        report.note(
            file,
            line,
            format!("reference `{u}` has no block in this graph, kept as text"),
        );
    }
    let s = EMBED_PAGE.replace_all(&s, "![[$1]]");
    let s = TAG_LINK.replace_all(&s, "[[$1]]");
    let s = IMAGE_SIZE.replace_all(&s, |c: &Captures| {
        let (w, h) = if &c[3] == "width" {
            (&c[4], &c[6])
        } else {
            (&c[6], &c[4])
        };
        format!("![{}|{w}x{h}]({})", &c[1], &c[2])
    });
    s.into_owned()
}

/// A block's first line: Logseq's marker becomes Obsidian's checkbox.
pub fn task(head: &str, file: &str, line: usize, report: &mut Report) -> String {
    // Only a marker that becomes a checkbox is rewritten; every other head is
    // returned as written, down to the two spaces of a hard line break.
    let out = match TASK.captures(head) {
        Some(c) => {
            let marker = &c[1];
            let rest = &head[c[0].len()..];
            if marker == "CANCELED" || marker == "CANCELLED" {
                report.note(
                    file,
                    line,
                    format!("task marker `{marker}` has no checkbox form, kept as text"),
                );
                head.to_owned()
            } else {
                let (check, body) = match marker {
                    "TODO" => ("[ ]", rest.to_owned()),
                    "DONE" => ("[x]", rest.to_owned()),
                    _ if rest.is_empty() => ("[ ]", marker.to_owned()),
                    _ => ("[ ]", format!("{marker} {rest}")),
                };
                if body.is_empty() {
                    check.to_owned()
                } else {
                    format!("{check} {body}")
                }
            }
        }
        None => head.to_owned(),
    };
    if let Some(p) = PRIORITY.find(&out) {
        report.note(
            file,
            line,
            format!("priority `{}` kept as text", p.as_str()),
        );
    }
    out
}

/// Lines Obsidian has no reading of. `CLOCK:` lines sit inside the logbook
/// and are covered by its entry.
pub fn note_kept(text: &str, file: &str, line: usize, report: &mut Report) {
    for prefix in ["SCHEDULED:", "DEADLINE:", ":LOGBOOK:"] {
        if text.starts_with(prefix) {
            report.note(file, line, format!("`{prefix}` kept as text"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> Refs {
        let mut r = Refs::new();
        r.insert(
            "64f1a2b3-0000-4000-8000-000000000001".into(),
            ("Alpha".into(), "64f1a2b3".into()),
        );
        r
    }

    #[test]
    fn references_and_embeds() {
        let mut rep = Report::default();
        let s = rewrite(
            "see ((64f1a2b3-0000-4000-8000-000000000001)) and {{embed ((64f1a2b3-0000-4000-8000-000000000001))}} and {{embed [[Other Page]]}}",
            &refs(),
            "pages/x.md",
            3,
            &mut rep,
        );
        assert_eq!(
            s,
            "see [[Alpha#^64f1a2b3]] and ![[Alpha#^64f1a2b3]] and ![[Other Page]]"
        );
        assert!(rep.is_empty());
    }

    #[test]
    fn unknown_reference_is_kept_and_reported() {
        let mut rep = Report::default();
        let s = rewrite(
            "((deadbeef-0000-4000-8000-000000000009))",
            &refs(),
            "pages/x.md",
            3,
            &mut rep,
        );
        assert_eq!(s, "((deadbeef-0000-4000-8000-000000000009))");
        assert_eq!(
            rep.entries[0].what,
            "reference `((deadbeef-0000-4000-8000-000000000009))` has no block in this graph, kept as text"
        );
        assert_eq!(
            (rep.entries[0].file.as_str(), rep.entries[0].line),
            ("pages/x.md", 3)
        );
    }

    #[test]
    fn an_unresolvable_embed_is_reported_once() {
        let mut rep = Report::default();
        let s = rewrite(
            "{{embed ((deadbeef-0000-4000-8000-000000000009))}}",
            &refs(),
            "pages/x.md",
            3,
            &mut rep,
        );
        assert_eq!(s, "{{embed ((deadbeef-0000-4000-8000-000000000009))}}");
        let whats: Vec<_> = rep.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(
            whats,
            vec![
                "reference `{{embed ((deadbeef-0000-4000-8000-000000000009))}}` has no block in this graph, kept as text"
            ]
        );
    }

    #[test]
    fn multi_word_tag_becomes_a_link_and_single_word_stays() {
        let mut rep = Report::default();
        let s = rewrite("#[[multi word]] #single", &refs(), "f", 1, &mut rep);
        assert_eq!(s, "[[multi word]] #single");
    }

    #[test]
    fn image_size_hint_becomes_obsidians() {
        let mut rep = Report::default();
        let s = rewrite(
            "![pic](../assets/pic.png){:height 200, :width 300}",
            &refs(),
            "f",
            1,
            &mut rep,
        );
        assert_eq!(s, "![pic|300x200](../assets/pic.png)");
        let s = rewrite(
            "![pic](../assets/pic.png){:width 300, :height 200}",
            &refs(),
            "f",
            1,
            &mut rep,
        );
        assert_eq!(s, "![pic|300x200](../assets/pic.png)");
    }

    #[test]
    fn task_markers() {
        let mut rep = Report::default();
        assert_eq!(task("TODO buy milk", "f", 1, &mut rep), "[ ] buy milk");
        assert_eq!(task("DONE buy milk", "f", 1, &mut rep), "[x] buy milk");
        assert_eq!(
            task("DOING buy milk", "f", 1, &mut rep),
            "[ ] DOING buy milk"
        );
        assert_eq!(task("LATER", "f", 1, &mut rep), "[ ] LATER");
        assert_eq!(
            task("TODOS are plural", "f", 1, &mut rep),
            "TODOS are plural"
        );
        assert_eq!(task("[ ] already", "f", 1, &mut rep), "[ ] already");
        assert!(rep.is_empty());
        assert_eq!(task("CANCELED x", "f", 4, &mut rep), "CANCELED x");
        assert_eq!(
            rep.entries[0].what,
            "task marker `CANCELED` has no checkbox form, kept as text"
        );
        assert_eq!(
            task("TODO [#A] urgent", "f", 5, &mut rep),
            "[ ] [#A] urgent"
        );
        assert_eq!(rep.entries[1].what, "priority `[#A]` kept as text");
    }

    #[test]
    fn a_head_that_is_not_a_task_keeps_its_hard_line_break() {
        let mut rep = Report::default();
        assert_eq!(task("a line  ", "f", 1, &mut rep), "a line  ");
        assert_eq!(task("CANCELED x  ", "f", 1, &mut rep), "CANCELED x  ");
        assert_eq!(task("TODO", "f", 1, &mut rep), "[ ]");
        assert_eq!(task("DONE", "f", 1, &mut rep), "[x]");
    }

    #[test]
    fn scheduling_and_logbook_are_noted() {
        let mut rep = Report::default();
        note_kept("SCHEDULED: <2026-09-20 Sun>", "f", 2, &mut rep);
        note_kept("DEADLINE: <2026-09-21 Mon>", "f", 3, &mut rep);
        note_kept(":LOGBOOK:", "f", 4, &mut rep);
        note_kept("CLOCK: [2026-09-14 Sun 10:00:00]", "f", 5, &mut rep);
        let whats: Vec<_> = rep.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(
            whats,
            vec![
                "`SCHEDULED:` kept as text",
                "`DEADLINE:` kept as text",
                "`:LOGBOOK:` kept as text"
            ]
        );
    }
}
