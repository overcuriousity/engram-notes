//! Obsidian's templates: notes in one folder, inserted with `{{date}}`,
//! `{{time}}` and `{{title}}` filled in. The daily note is made the same way.

use crate::config::AppConfig;
use crate::vault::Vault;
use crate::{Error, Result};
use chrono::{Datelike, NaiveDateTime, Timelike};
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
        moment_to_strftime(fmt, at)
    };
    let mut s = String::new();
    match write!(s, "{}", at.format(&strf)) {
        Ok(()) => s,
        Err(_) => fmt.to_owned(),
    }
}

/// A moment token is either a strftime code chrono understands or, where
/// chrono has none, the value written out for this instant.
enum Tok {
    Strf(&'static str),
    Calc(fn(NaiveDateTime) -> String),
}

// Longest token first, as moment scans. `[text]` is literal, as in moment.
const TOKENS: &[(&str, Tok)] = &[
    ("YYYY", Tok::Strf("%Y")),
    ("YY", Tok::Strf("%y")),
    ("gggg", Tok::Strf("%G")),
    ("gg", Tok::Strf("%g")),
    ("GGGG", Tok::Strf("%G")),
    ("GG", Tok::Strf("%g")),
    ("Qo", Tok::Calc(|t| ordinal(quarter(t)))),
    ("Q", Tok::Calc(|t| quarter(t).to_string())),
    ("MMMM", Tok::Strf("%B")),
    ("MMM", Tok::Strf("%b")),
    ("MM", Tok::Strf("%m")),
    ("Mo", Tok::Calc(|t| ordinal(t.month()))),
    ("M", Tok::Strf("%-m")),
    ("DDDo", Tok::Calc(|t| ordinal(t.ordinal()))),
    ("DDDD", Tok::Strf("%j")),
    ("DDD", Tok::Calc(|t| t.ordinal().to_string())),
    ("DD", Tok::Strf("%d")),
    ("Do", Tok::Calc(|t| ordinal(t.day()))),
    ("D", Tok::Strf("%-d")),
    ("dddd", Tok::Strf("%A")),
    ("ddd", Tok::Strf("%a")),
    ("dd", Tok::Calc(|t| short_weekday(t).to_owned())),
    (
        "do",
        Tok::Calc(|t| ordinal(t.weekday().num_days_from_sunday())),
    ),
    ("d", Tok::Strf("%w")),
    ("E", Tok::Strf("%u")),
    ("e", Tok::Strf("%w")),
    ("WW", Tok::Strf("%V")),
    ("Wo", Tok::Calc(|t| ordinal(t.iso_week().week()))),
    ("W", Tok::Strf("%-V")),
    ("ww", Tok::Strf("%U")),
    ("wo", Tok::Calc(|t| ordinal(sunday_week(t)))),
    ("w", Tok::Strf("%-U")),
    ("HH", Tok::Strf("%H")),
    ("H", Tok::Strf("%-H")),
    ("hh", Tok::Strf("%I")),
    ("h", Tok::Strf("%-I")),
    ("kk", Tok::Calc(|t| format!("{:02}", hour_from_one(t)))),
    ("k", Tok::Calc(|t| hour_from_one(t).to_string())),
    ("mm", Tok::Strf("%M")),
    ("m", Tok::Strf("%-M")),
    ("ss", Tok::Strf("%S")),
    ("s", Tok::Strf("%-S")),
    ("SSS", Tok::Strf("%3f")),
    (
        "SS",
        Tok::Calc(|t| format!("{:02}", t.nanosecond() / 10_000_000)),
    ),
    (
        "S",
        Tok::Calc(|t| (t.nanosecond() / 100_000_000).to_string()),
    ),
    ("A", Tok::Strf("%p")),
    ("a", Tok::Strf("%P")),
    ("X", Tok::Strf("%s")),
    (
        "x",
        Tok::Calc(|t| t.and_utc().timestamp_millis().to_string()),
    ),
];

fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn quarter(t: NaiveDateTime) -> u32 {
    (t.month() - 1) / 3 + 1
}

fn short_weekday(t: NaiveDateTime) -> &'static str {
    ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"][t.weekday().num_days_from_sunday() as usize]
}

// strftime's `%U`, which chrono can print but not hand back as a number.
fn sunday_week(t: NaiveDateTime) -> u32 {
    (t.ordinal() + 6 - t.weekday().num_days_from_sunday()) / 7
}

// Moment's `k` counts 1..=24, so midnight is the 24th hour of the day before.
fn hour_from_one(t: NaiveDateTime) -> u32 {
    if t.hour() == 0 { 24 } else { t.hour() }
}

fn moment_to_strftime(fmt: &str, at: NaiveDateTime) -> String {
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
        for (tok, repl) in TOKENS {
            if let Some(after) = rest.strip_prefix(tok) {
                match repl {
                    Tok::Strf(s) => out.push_str(s),
                    Tok::Calc(f) => out.push_str(&f(at).replace('%', "%%")),
                }
                rest = after;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        if c == '%' {
            out.push_str("%%");
            rest = &rest[1..];
            continue;
        }
        // A token we do not know stays as written. Letting the one-character
        // entries above take it apart would put a wrong date in the note with
        // nothing to show it went wrong.
        let run = if c.is_ascii_alphabetic() {
            rest.len() - rest.trim_start_matches(c).len()
        } else {
            c.len_utf8()
        };
        out.push_str(&rest[..run]);
        rest = &rest[run..];
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
    fn the_tokens_chrono_cannot_write_are_filled_in_and_unknown_ones_survive() {
        assert_eq!(
            format_date("dddd, MMMM Do YYYY", at()),
            "Monday, September 14th 2026"
        );
        assert_eq!(format_date("Do Mo Qo DDDo", at()), "14th 9th 3rd 257th");
        assert_eq!(format_date("[Q]Q DDD DDDD E e", at()), "Q3 257 257 1 1");
        assert_eq!(format_date("gggg-[W]WW-E", at()), "2026-W38-1");
        assert_eq!(format_date("dd do Wo wo", at()), "Mo 1st 38th 37th");
        assert_eq!(format_date("k kk S SS SSS", at()), "9 09 0 00 000");
        // Unknown tokens come back whole instead of being taken apart.
        assert_eq!(format_date("YYYY-MM-DDTHH:mmZ", at()), "2026-09-14T09:05Z");
        assert_eq!(format_date("ZZ NNN [Do]", at()), "ZZ NNN Do");
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
