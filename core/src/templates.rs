//! Obsidian's templates: notes in one folder, inserted with `{{date}}`,
//! `{{time}}` and `{{title}}` filled in. The daily note is made the same way.

use crate::config::AppConfig;
use crate::vault::Vault;
use crate::{Error, Result};
use chrono::NaiveDateTime;
use std::fmt::Write as _;

/// Fills every `{{date}}`, `{{time}}`, `{{title}}`, `{{date:FMT}}` and
/// `{{time:FMT}}`. Anything else in braces is left as it is.
pub fn expand(text: &str, cfg: &AppConfig, now: NaiveDateTime, title: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(len) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = after[..len].trim();
        let (key, arg) = match inner.split_once(':') {
            Some((k, a)) => (k.trim(), Some(a.trim())),
            None => (inner, None),
        };
        let value = match key {
            "date" => Some(format_date(arg.unwrap_or(&cfg.templates.date_format), now)),
            "time" => Some(format_date(arg.unwrap_or(&cfg.templates.time_format), now)),
            "title" if arg.is_none() => Some(title.to_owned()),
            _ => None,
        };
        // Not a placeholder: keep one brace and look again from the next,
        // so `{{{title}}}` gives `{T}` as Obsidian's regex does.
        match value {
            Some(v) => {
                out.push_str(&v);
                rest = &after[len + 2..];
            }
            None => {
                out.push('{');
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Formats with Obsidian's tokens (`YYYY-MM-DD`, `HH:mm`), or with strftime
/// when the pattern carries a `%`, so both kinds of `app.json` read the same
/// way. A pattern chrono rejects comes back as written rather than panicking.
pub fn format_date(fmt: &str, at: NaiveDateTime) -> String {
    let strf = if fmt.contains('%') {
        fmt.to_owned()
    } else {
        moment_to_strftime(fmt)
    };
    let mut s = String::new();
    match write!(s, "{}", at.format(&strf)) {
        Ok(()) => s,
        Err(_) => fmt.to_owned(),
    }
}

// Longest token first, as moment scans. `[text]` is literal, as in moment.
const TOKENS: &[(&str, &str)] = &[
    ("YYYY", "%Y"),
    ("YY", "%y"),
    ("MMMM", "%B"),
    ("MMM", "%b"),
    ("MM", "%m"),
    ("M", "%-m"),
    ("DD", "%d"),
    ("D", "%-d"),
    ("dddd", "%A"),
    ("ddd", "%a"),
    ("d", "%w"),
    ("HH", "%H"),
    ("H", "%-H"),
    ("hh", "%I"),
    ("h", "%-I"),
    ("mm", "%M"),
    ("m", "%-M"),
    ("ss", "%S"),
    ("s", "%-S"),
    ("SSS", "%3f"),
    ("A", "%p"),
    ("a", "%P"),
    ("WW", "%V"),
    ("W", "%-V"),
    ("ww", "%U"),
    ("w", "%-U"),
    ("X", "%s"),
];

fn moment_to_strftime(fmt: &str) -> String {
    let mut out = String::with_capacity(fmt.len() * 2);
    let mut rest = fmt;
    'outer: while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix('[')
            && let Some(end) = inner.find(']')
        {
            out.push_str(&inner[..end].replace('%', "%%"));
            rest = &inner[end + 1..];
            continue;
        }
        for (tok, strf) in TOKENS {
            if let Some(after) = rest.strip_prefix(tok) {
                out.push_str(strf);
                rest = after;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        if c == '%' {
            out.push_str("%%");
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The `.md` files under the templates folder, vault-relative and sorted.
/// A folder that does not exist yet lists nothing rather than failing.
pub fn list(vault: &Vault, cfg: &AppConfig) -> Result<Vec<String>> {
    let folder = cfg.templates.folder.trim_matches('/');
    if folder.is_empty() {
        return Ok(Vec::new());
    }
    let dir = vault.abs(folder);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(&dir)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'));
    for entry in walker {
        let entry = entry.map_err(|e| Error::io(&dir, e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(vault.root()).unwrap();
        let path = rel.to_string_lossy().replace('\\', "/");
        if path.to_lowercase().ends_with(".md") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// The template's text with its placeholders filled for `title`.
pub fn render(
    vault: &Vault,
    cfg: &AppConfig,
    path: &str,
    now: NaiveDateTime,
    title: &str,
) -> Result<String> {
    Ok(expand(&vault.read(path)?, cfg, now, title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 14)
            .unwrap()
            .and_hms_opt(9, 5, 7)
            .unwrap()
    }

    #[test]
    fn the_three_placeholders_take_the_configured_formats() {
        let cfg = AppConfig::default();
        assert_eq!(
            expand("# {{title}}\n{{date}} {{time}}", &cfg, at(), "Meeting"),
            "# Meeting\n2026-09-14 09:05"
        );
    }

    #[test]
    fn a_format_argument_overrides_the_setting_in_either_language() {
        let cfg = AppConfig::default();
        assert_eq!(
            expand("{{date:dddd, MMMM D}}", &cfg, at(), ""),
            "Monday, September 14"
        );
        assert_eq!(
            expand("{{ date : %d.%m.%Y }}", &cfg, at(), ""),
            "14.09.2026"
        );
        assert_eq!(expand("{{time:h:mm A}}", &cfg, at(), ""), "9:05 AM");
        assert_eq!(expand("{{date:[week] W}}", &cfg, at(), ""), "week 38");
    }

    #[test]
    fn unknown_and_unclosed_braces_are_left_alone() {
        let cfg = AppConfig::default();
        assert_eq!(
            expand("{{other}} {{title:x}} {{date", &cfg, at(), "T"),
            "{{other}} {{title:x}} {{date"
        );
        assert_eq!(expand("{}{{{title}}}", &cfg, at(), "T"), "{}{T}");
    }

    #[test]
    fn a_bad_pattern_comes_back_as_written() {
        assert_eq!(format_date("%Q", at()), "%Q");
        assert_eq!(format_date("YYYY", at()), "2026");
    }

    #[test]
    fn lists_markdown_under_the_folder_sorted_and_reads_through_expand() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let cfg = AppConfig::default();
        assert_eq!(list(&v, &cfg).unwrap(), Vec::<String>::new());
        v.write("Templates/b.md", "B {{title}}").unwrap();
        v.write("Templates/sub/a.md", "").unwrap();
        v.write("Templates/notes.txt", "").unwrap();
        v.write("Templates/.hidden.md", "").unwrap();
        v.write("other.md", "").unwrap();
        assert_eq!(
            list(&v, &cfg).unwrap(),
            vec!["Templates/b.md", "Templates/sub/a.md"]
        );
        assert_eq!(
            render(&v, &cfg, "Templates/b.md", at(), "N").unwrap(),
            "B N"
        );
        assert!(matches!(
            render(&v, &cfg, "Templates/none.md", at(), "N"),
            Err(Error::Io { .. })
        ));
    }
}
