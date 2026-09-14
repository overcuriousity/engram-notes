//! A Logseq graph (`pages/`, `journals/`, `assets/`) into a folder of the
//! vault in Obsidian's shape. `map_graph` is pure; `run` reads and writes.

pub mod inline;
pub mod page;

use crate::config::{self, AppConfig};
use crate::import::Report;
use inline::Refs;
use page::{Line, Page};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Relative to the graph root, `/`-separated.
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// Relative to the vault root.
    pub path: String,
    pub text: String,
    pub journal: bool,
}

pub struct Options<'a> {
    /// Folder inside the vault, `""` for the root.
    pub dest: &'a str,
    pub config: &'a AppConfig,
}

#[derive(Debug, Default)]
pub struct Output {
    pub pages: Vec<Mapped>,
    pub report: Report,
}

struct Planned<'a> {
    src: &'a SourceFile,
    path: String,
    journal: bool,
    page: Page,
    /// Frontmatter keys and values, `title` already decided.
    properties: Vec<(String, String)>,
}

/// Folders of the graph that hold no notes: Logseq's own state and backups.
const IGNORED: [&str; 3] = ["logseq", "bak", "version-files"];

fn join(dest: &str, rest: &str) -> String {
    let dest = dest.trim_matches('/');
    if dest.is_empty() {
        rest.to_owned()
    } else {
        format!("{dest}/{rest}")
    }
}

/// A title Obsidian would accept as a file name, with `/` for folders.
fn usable_name(title: &str) -> bool {
    !title.is_empty()
        && !title.contains(['\\', ':', '*', '?', '"', '<', '>', '|', '#', '^', '['])
        && !title.starts_with('/')
        && !title.ends_with('/')
        && title.split('/').all(|seg| {
            !seg.is_empty() && seg != "." && seg != ".." && !seg.ends_with('.') && seg.trim() == seg
        })
}

/// Logseq's file name for a page: `a___b.md` (or the older `a%2Fb.md`) is `a/b`.
fn page_name(rel: &str) -> String {
    let stem = rel.strip_suffix(".md").unwrap_or(rel);
    stem.replace("___", "/").replace("%2F", "/")
}

fn plan<'a>(f: &'a SourceFile, opts: &Options, report: &mut Report) -> Planned<'a> {
    let mut page = page::parse(&f.text, opts.config.editor.indent);
    let mut properties = std::mem::take(&mut page.properties);
    if let Some(name) = f.path.strip_prefix("journals/") {
        let stem = name.strip_suffix(".md").unwrap_or(name);
        let path = match chrono::NaiveDate::parse_from_str(stem, "%Y_%m_%d") {
            Ok(date) => config::daily_note_path(opts.config, date),
            Err(_) => {
                report.note(
                    &f.path,
                    0,
                    "journal name is not a date, kept under `journals/`",
                );
                join(opts.dest, &format!("journals/{name}"))
            }
        };
        return Planned {
            src: f,
            path,
            journal: true,
            page,
            properties,
        };
    }
    let rel = f.path.strip_prefix("pages/").unwrap_or(&f.path);
    let mut name = page_name(rel);
    if let Some(i) = properties.iter().position(|(k, _)| k == "title") {
        let title = properties[i].1.clone();
        if usable_name(&title) {
            properties.remove(i);
            name = title;
        } else {
            report.note(
                &f.path,
                0,
                format!("title `{title}` cannot be a file name, kept as a property"),
            );
        }
    }
    Planned {
        src: f,
        path: join(opts.dest, &format!("{name}.md")),
        journal: false,
        page,
        properties,
    }
}

/// Eight characters of the uuid, more when the page already uses them.
fn anchor_for(uuid: &str, used: &mut HashSet<String>) -> String {
    let flat: String = uuid
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_lowercase();
    for len in [8, 12, 16, 32] {
        let a = flat[..len.min(flat.len())].to_owned();
        if used.insert(a.clone()) {
            return a;
        }
    }
    flat
}

fn stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
}

/// Collects `id::` blocks into the table and stamps their anchors onto the
/// blocks, so the second pass can render `^anchor` and rewrite `((uuid))`.
fn collect_ids(planned: &mut [Planned], refs: &mut Refs) {
    let mut stems: HashMap<String, usize> = HashMap::new();
    for p in planned.iter() {
        *stems.entry(stem(&p.path).to_lowercase()).or_default() += 1;
    }
    for p in planned.iter_mut() {
        let stem = stem(&p.path);
        // Obsidian resolves a bare name to the shortest path; a shared name
        // needs the full one.
        let link = if stems[&stem.to_lowercase()] > 1 {
            p.path.trim_end_matches(".md").to_owned()
        } else {
            stem.to_owned()
        };
        let mut used = HashSet::new();
        for b in &mut p.page.blocks {
            let id = b.body.iter().find_map(|l| match l {
                Line::Property { key, value, .. } if key == "id" => Some(value.clone()),
                _ => None,
            });
            let Some(uuid) = id else { continue };
            let anchor = anchor_for(&uuid, &mut used);
            refs.insert(uuid.to_lowercase(), (link.clone(), anchor.clone()));
            if b.head.trim_start().starts_with("```") {
                // After a fence opener the anchor would be part of the info string.
                continue;
            }
            b.head = format!("{} ^{anchor}", b.head).trim_start().to_owned();
        }
    }
}

fn yaml_value(key: &str, value: &str) -> serde_json::Value {
    use serde_json::Value;
    if key == "tags" || key == "alias" || key == "aliases" {
        let items = value
            .split(',')
            .map(|s| {
                s.trim()
                    .trim_start_matches('#')
                    .trim_start_matches("[[")
                    .trim_end_matches("]]")
                    .trim()
            })
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_owned()))
            .collect();
        return Value::Array(items);
    }
    if let Ok(b) = value.parse::<bool>() {
        return Value::Bool(b);
    }
    if let Ok(n) = value.parse::<i64>() {
        return Value::Number(n.into());
    }
    if let Ok(f) = value.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return Value::Number(n);
    }
    Value::String(value.to_owned())
}

fn frontmatter(properties: &[(String, String)]) -> String {
    if properties.is_empty() {
        return String::new();
    }
    let mut map = serde_json::Map::new();
    for (k, v) in properties {
        let key = if k == "alias" { "aliases" } else { k.as_str() };
        map.insert(key.to_owned(), yaml_value(k, v));
    }
    let yaml = serde_yaml_ng::to_string(&map).unwrap_or_default();
    format!("---\n{yaml}---\n")
}

fn render(p: &Planned, refs: &Refs, indent: usize, report: &mut Report) -> String {
    let file = &p.src.path;
    let mut page = p.page.clone();
    for b in &mut page.blocks {
        let mut in_fence = b.head.trim_start().starts_with("```");
        if !in_fence {
            b.head = inline::task(&b.head, file, b.line, report);
            b.head = inline::rewrite(&b.head, refs, file, b.line, report);
        }
        // Every body entry is one source line, so numbering follows the index.
        let body = std::mem::take(&mut b.body);
        for (i, l) in body.into_iter().enumerate() {
            let line = b.line + 1 + i;
            match l {
                Line::Text(t) => {
                    if t.trim_start().starts_with("```") {
                        in_fence = !in_fence;
                        b.body.push(Line::Text(t));
                    } else if in_fence {
                        b.body.push(Line::Text(t));
                    } else {
                        inline::note_kept(&t, file, line, report);
                        b.body
                            .push(Line::Text(inline::rewrite(&t, refs, file, line, report)));
                    }
                }
                Line::Property { key, .. } if key == "id" || key == "collapsed" => {}
                Line::Property { line, key, value } => {
                    report.note(
                        file,
                        line,
                        format!("block property `{key}:: {value}` kept as text"),
                    );
                    b.body.push(Line::Property { line, key, value });
                }
            }
        }
    }
    page::render(&page, &frontmatter(&p.properties), indent)
}

pub fn map_graph(files: &[SourceFile], opts: &Options) -> Output {
    let mut out = Output::default();
    let mut planned = Vec::new();
    for f in files {
        let top = f.path.split('/').next().unwrap_or("");
        if IGNORED.contains(&top) || top == "assets" {
            continue;
        }
        if !f.path.to_lowercase().ends_with(".md") {
            out.report.note(&f.path, 0, "not imported");
            continue;
        }
        planned.push(plan(f, opts, &mut out.report));
    }
    let mut refs = Refs::new();
    collect_ids(&mut planned, &mut refs);
    let indent = opts.config.editor.indent;
    for p in &planned {
        let text = render(p, &refs, indent, &mut out.report);
        out.pages.push(Mapped {
            path: p.path.clone(),
            text,
            journal: p.journal,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn read_dir(root: &Path) -> Vec<SourceFile> {
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(root) {
            let e = e.unwrap();
            if !e.file_type().is_file() {
                continue;
            }
            let path = e
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                out.push(SourceFile { path, text });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    fn fixture() -> (Vec<SourceFile>, Vec<SourceFile>) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        (
            read_dir(&root.join("logseq")),
            read_dir(&root.join("logseq-expected")),
        )
    }

    fn map(files: &[SourceFile]) -> Output {
        let cfg = AppConfig::default();
        map_graph(
            files,
            &Options {
                dest: "",
                config: &cfg,
            },
        )
    }

    #[test]
    fn fixture_graph_maps_to_expected_files() {
        let (src, expected) = fixture();
        let out = map(&src);
        let mut got: Vec<_> = out
            .pages
            .iter()
            .map(|m| (m.path.as_str(), m.text.as_str()))
            .collect();
        got.sort();
        let want: Vec<_> = expected
            .iter()
            .map(|m| (m.path.as_str(), m.text.as_str()))
            .collect();
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.0, w.0);
            assert_eq!(g.1, w.1, "in {}", g.0);
        }
        assert_eq!(got.len(), want.len());
        assert_eq!(out.pages.iter().filter(|m| m.journal).count(), 2);
    }

    #[test]
    fn report_names_every_unmapped_construct() {
        let (src, _) = fixture();
        let out = map(&src);
        let mut whats: Vec<_> = out
            .report
            .entries
            .iter()
            .map(|e| format!("{}:{} {}", e.file, e.line, e.what))
            .collect();
        whats.sort();
        let expected = [
            "journals/notadate.md:0 journal name is not a date, kept under `journals/`",
            "pages/Project___Alpha.md:11 `SCHEDULED:` kept as text",
            "pages/Project___Alpha.md:12 priority `[#A]` kept as text",
            "pages/Project___Alpha.md:14 `:LOGBOOK:` kept as text",
            "pages/Project___Alpha.md:19 block property `logseq.order-list-type:: number` kept as text",
            "pages/Project___Alpha.md:24 task marker `CANCELED` has no checkbox form, kept as text",
            "pages/notes.md:1 reference `((64f1a2b3-0000-4000-8000-00000000dead))` has no block in this graph, kept as text",
            "pages/odd title.md:0 title `what: is this?` cannot be a file name, kept as a property",
            "whiteboards/board.edn:0 not imported",
        ];
        assert_eq!(whats, expected);
    }

    #[test]
    fn importing_the_output_changes_nothing() {
        let (src, _) = fixture();
        let first = map(&src);
        let again: Vec<SourceFile> = first
            .pages
            .iter()
            .map(|m| SourceFile {
                path: m.path.clone(),
                text: m.text.clone(),
            })
            .collect();
        let second = map(&again);
        let texts = |o: &Output| -> Vec<(String, String)> {
            let mut v: Vec<_> = o
                .pages
                .iter()
                .map(|m| (m.path.clone(), m.text.clone()))
                .collect();
            v.sort();
            v
        };
        assert_eq!(texts(&second), texts(&first));
    }

    #[test]
    fn destination_and_duplicate_basenames() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/a___x.md".into(),
                text: "- one\n  id:: 64f1a2b3-0000-4000-8000-000000000001\n".into(),
            },
            SourceFile {
                path: "pages/b___x.md".into(),
                text: "- ((64f1a2b3-0000-4000-8000-000000000001))\n".into(),
            },
            SourceFile {
                path: "journals/2026_01_02.md".into(),
                text: "- j\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "Logseq/",
                config: &cfg,
            },
        );
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["Logseq/a/x.md", "Logseq/b/x.md", "Daily/2026-01-02.md"]
        );
        assert_eq!(out.pages[1].text, "- [[Logseq/a/x#^64f1a2b3]]\n");
        assert!(out.pages[2].journal);
    }

    #[test]
    fn title_renames_and_case_is_the_titles() {
        let cfg = AppConfig::default();
        let files = vec![SourceFile {
            path: "pages/my page.md".into(),
            text: "title:: My Page\n\n- x\n".into(),
        }];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        assert_eq!(out.pages[0].path, "My Page.md");
        assert_eq!(out.pages[0].text, "\n- x\n");
    }
}
