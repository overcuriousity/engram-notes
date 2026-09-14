//! A Logseq graph (`pages/`, `journals/`, `assets/`) into a folder of the
//! vault in Obsidian's shape. `map_graph` is pure; `run` reads and writes.

pub mod inline;
pub mod page;

use crate::config::{self, AppConfig};
use crate::import::Report;
use crate::vault::Vault;
use crate::{Error, Result};
use inline::Refs;
use page::{Line, Page};
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
    /// The graph file it came from, relative to the graph root.
    pub source: String,
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

/// Characters Obsidian will not take in a file name; `/` is the folder
/// separator and is judged per segment instead.
const FORBIDDEN: [char; 12] = ['\\', ':', '*', '?', '"', '<', '>', '|', '#', '^', '[', ']'];

/// A title Obsidian would accept as a file name, with `/` for folders.
fn usable_name(title: &str) -> bool {
    !title.is_empty()
        && !title.contains(FORBIDDEN)
        && !title.starts_with('/')
        && !title.ends_with('/')
        && title.split('/').all(|seg| {
            !seg.is_empty() && !seg.starts_with('.') && !seg.ends_with('.') && seg.trim() == seg
        })
}

/// The nearest usable name to one the graph's own file name produced. A
/// segment that is left with nothing is dropped, which is what turns `..`
/// into no segment at all and keeps the page inside the chosen folder.
fn sanitize(name: &str) -> String {
    let segments: Vec<String> = name
        .split('/')
        .map(|seg| {
            let seg: String = seg
                .chars()
                .map(|c| if FORBIDDEN.contains(&c) { '-' } else { c })
                .collect();
            seg.trim().trim_matches('.').trim().to_owned()
        })
        .filter(|seg| !seg.is_empty())
        .collect();
    if segments.is_empty() {
        "untitled".to_owned()
    } else {
        segments.join("/")
    }
}

/// Logseq writes `.md`, but a graph carried through other tools can hold `.MD`.
fn strip_md(name: &str) -> Option<&str> {
    let cut = name.len().checked_sub(3)?;
    if name.get(cut..)?.eq_ignore_ascii_case(".md") {
        Some(&name[..cut])
    } else {
        None
    }
}

/// Logseq's file name for a page: `a___b.md` (or the older `a%2Fb.md`) is `a/b`.
fn page_name(rel: &str) -> String {
    let stem = strip_md(rel).unwrap_or(rel);
    stem.replace("___", "/").replace("%2F", "/")
}

fn plan<'a>(f: &'a SourceFile, opts: &Options, report: &mut Report) -> Planned<'a> {
    let mut page = page::parse(&f.text, opts.config.editor.indent);
    let mut properties = std::mem::take(&mut page.properties);
    if let Some(name) = f.path.strip_prefix("journals/") {
        let stem = strip_md(name).unwrap_or(name);
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
    // A title is checked above, so this is a name the graph's file name made:
    // `..___etc___x.md` would leave the chosen folder, `a___.md` would be a
    // dotfile the vault never shows.
    if !usable_name(&name) {
        let fixed = sanitize(&name);
        report.note(
            &f.path,
            0,
            format!("`{name}` cannot be a file name, imported as `{fixed}`"),
        );
        name = fixed;
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
        // An `id::` value is not always a uuid, so count characters.
        let a: String = flat.chars().take(len).collect();
        if used.insert(a.clone()) {
            return a;
        }
    }
    flat
}

/// The lines a block will render, `id::` and `collapsed::` already dropped.
fn block_lines(b: &page::Block) -> Vec<String> {
    let mut lines = vec![b.head.clone()];
    lines.extend(
        b.body
            .iter()
            .filter(
                |l| !matches!(l, Line::Property { key, .. } if key == "id" || key == "collapsed"),
            )
            .map(page::block_line),
    );
    lines
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    strip_md(name).unwrap_or(name)
}

/// Collects `id::` blocks into the table and stamps their anchors onto the
/// blocks, so the second pass can render `^anchor` and rewrite `((uuid))`.
fn collect_ids(planned: &mut [Planned], refs: &mut Refs, report: &mut Report) {
    let mut stems: HashMap<String, usize> = HashMap::new();
    // A uuid copied onto a second block would otherwise point every reference
    // at whichever page was planned last.
    let mut owner: HashMap<String, String> = HashMap::new();
    for p in planned.iter() {
        *stems.entry(stem(&p.path).to_lowercase()).or_default() += 1;
    }
    for p in planned.iter_mut() {
        let stem = stem(&p.path);
        // Obsidian resolves a bare name to the shortest path; a shared name
        // needs the full one.
        let link = if stems[&stem.to_lowercase()] > 1 {
            strip_md(&p.path).unwrap_or(&p.path).to_owned()
        } else {
            stem.to_owned()
        };
        // Logseq keys every page by an `id::` of its own; in the vault the
        // page's name is that key, so a reference to it is a page link.
        if let Some(i) = p.properties.iter().position(|(k, _)| k == "id") {
            let (_, uuid) = p.properties.remove(i);
            let key = uuid.to_lowercase();
            match owner.get(&key) {
                Some(first) => report.note(
                    &p.src.path,
                    0,
                    format!(
                        "page `id:: {uuid}` is already used by `{first}`, references to it point there"
                    ),
                ),
                None => {
                    owner.insert(key.clone(), p.src.path.clone());
                    refs.insert(key, (link.clone(), String::new()));
                }
            }
        }
        let mut used = HashSet::new();
        for b in &mut p.page.blocks {
            let id = b.body.iter().find_map(|l| match l {
                Line::Property { key, value, .. } if key == "id" => Some(value.clone()),
                _ => None,
            });
            let Some(uuid) = id else { continue };
            if page::anchor_line(&block_lines(b)).is_none() {
                // The anchor belongs on the block's last line, and inside a
                // code block it is code, not something to point at.
                report.note(
                    &p.src.path,
                    b.line,
                    "`id::` on a block that ends in a code block cannot become an anchor, references to it are kept as text",
                );
                continue;
            }
            let key = uuid.to_lowercase();
            if let Some(first) = owner.get(&key) {
                let what = format!(
                    "`id:: {uuid}` is already used by `{first}`, references to it point there and this block has no anchor"
                );
                report.note(&p.src.path, b.line, what);
                continue;
            }
            let anchor = anchor_for(&uuid, &mut used);
            owner.insert(key.clone(), p.src.path.clone());
            refs.insert(key, (link.clone(), anchor.clone()));
            b.anchor = Some(anchor);
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
    // Only when the number writes itself back the same way: the zeroes in
    // `0012345` and `1.10` are part of an id or a version, not arithmetic.
    if let Ok(n) = value.parse::<i64>()
        && n.to_string() == value
    {
        return Value::Number(n.into());
    }
    if let Ok(f) = value.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
        && n.to_string() == value
    {
        return Value::Number(n);
    }
    Value::String(value.to_owned())
}

fn frontmatter(properties: &[(String, String)], file: &str, report: &mut Report) -> String {
    if properties.is_empty() {
        return String::new();
    }
    let mut map = serde_json::Map::new();
    for (k, v) in properties {
        let key = if k == "alias" { "aliases" } else { k.as_str() };
        match (map.get_mut(key), yaml_value(k, v)) {
            // `alias::` and `aliases::` are one key, and two lists are one list.
            (Some(serde_json::Value::Array(have)), serde_json::Value::Array(more)) => {
                for item in more {
                    if !have.contains(&item) {
                        have.push(item);
                    }
                }
            }
            (Some(_), _) => report.note(
                file,
                0,
                format!("property `{k}:: {v}` repeats `{key}`, the first value is kept"),
            ),
            (None, value) => {
                map.insert(key.to_owned(), value);
            }
        }
    }
    match serde_yaml_ng::to_string(&map) {
        Ok(yaml) => format!("---\n{yaml}---\n"),
        Err(e) => {
            report.note(
                file,
                0,
                format!("page properties are not valid YAML ({e}), left out"),
            );
            String::new()
        }
    }
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
    page::render(&page, &frontmatter(&p.properties, file, report), indent)
}

pub fn map_graph(files: &[SourceFile], opts: &Options) -> Output {
    let mut out = Output::default();
    let mut planned = Vec::new();
    for f in files {
        let top = f.path.split('/').next().unwrap_or("");
        if IGNORED.contains(&top) || top == "assets" {
            continue;
        }
        if strip_md(&f.path).is_none() {
            out.report.note(&f.path, 0, "not imported");
            continue;
        }
        planned.push(plan(f, opts, &mut out.report));
    }
    // Two pages can want one file: a `title::` onto another page's name, or
    // journals whose daily format has no day in it. The first one keeps it.
    let mut taken: HashMap<String, String> = HashMap::new();
    // Two names that differ only in case are two files on Linux and one on
    // macOS or Windows, where the second is left alone and reported as
    // already in the vault. Keeping both is the answer the filesystem can
    // still refuse; dropping one is an answer it cannot undo.
    let mut folded: HashMap<String, String> = HashMap::new();
    planned.retain(|p| match taken.get(&p.path) {
        Some(first) => {
            out.report.note(
                &p.src.path,
                0,
                format!(
                    "maps to `{}`, which `{first}` already uses, not imported",
                    p.path
                ),
            );
            false
        }
        None => {
            if let Some(first) = folded.get(&p.path.to_lowercase()) {
                out.report.note(
                    &p.src.path,
                    0,
                    format!(
                        "maps to `{}`, which differs only in case from the target of `{first}`; a filesystem that ignores case keeps only one of them",
                        p.path
                    ),
                );
            }
            folded.insert(p.path.to_lowercase(), p.src.path.clone());
            taken.insert(p.path.clone(), p.src.path.clone());
            true
        }
    });
    let mut refs = Refs::new();
    collect_ids(&mut planned, &mut refs, &mut out.report);
    let indent = opts.config.editor.indent;
    for p in &planned {
        let text = render(p, &refs, indent, &mut out.report);
        out.pages.push(Mapped {
            path: p.path.clone(),
            source: p.src.path.clone(),
            text,
            journal: p.journal,
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct Summary {
    pub pages: usize,
    pub journals: usize,
    pub assets: usize,
    /// Files left alone because the vault already had them.
    pub skipped: usize,
    pub unmapped: usize,
    /// Vault path of the report.
    pub report: String,
}

/// A report name no imported page wants. The report is the one file the
/// importer overwrites, and that licence covers only reports it wrote.
fn report_path(dest: &str, pages: &[Mapped]) -> String {
    let taken: HashSet<String> = pages.iter().map(|m| m.path.to_lowercase()).collect();
    let mut path = join(dest, "import-report.md");
    let mut n = 1;
    while taken.contains(&path.to_lowercase()) {
        path = join(dest, &format!("import-report-{n}.md"));
        n += 1;
    }
    path
}

/// Reads the graph, writes what maps into the vault without overwriting
/// anything, copies `assets/`, and writes the report. The graph is only read.
pub fn run(vault: &Vault, cfg: &AppConfig, graph: &Path, dest: &str) -> Result<Summary> {
    if !graph.join("pages").is_dir() && !graph.join("journals").is_dir() {
        return Err(Error::NotFound(format!(
            "{}: not a Logseq graph (no pages/ or journals/)",
            graph.display()
        )));
    }
    let mut files = Vec::new();
    let mut assets = Vec::new();
    let mut unreadable = Vec::new();
    let walker = walkdir::WalkDir::new(graph).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        e.depth() == 0
            || (!name.starts_with('.') && !(e.depth() == 1 && IGNORED.contains(&name.as_ref())))
    });
    for entry in walker {
        let entry = entry.map_err(|e| Error::io(graph, e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(graph)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("assets/") {
            assets.push((entry.path().to_path_buf(), rel));
        } else {
            // A file that is not text is not a note.
            match std::fs::read_to_string(entry.path()) {
                Ok(text) => files.push(SourceFile { path: rel, text }),
                Err(e) => unreadable.push((rel, e.to_string())),
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = map_graph(&files, &Options { dest, config: cfg });
    for (rel, why) in &unreadable {
        out.report.note(
            rel,
            0,
            format!("could not be read as text ({why}), not imported"),
        );
    }
    let mut s = Summary {
        report: report_path(dest, &out.pages),
        // What the mapping could not map; writing counts its own skips.
        unmapped: out.report.entries.len(),
        ..Default::default()
    };
    // A write that fails still owes the user a report of what landed.
    let mut failed = None;
    for m in &out.pages {
        match vault.create(&m.path, &m.text) {
            Ok(()) if m.journal => s.journals += 1,
            Ok(()) => s.pages += 1,
            Err(Error::Exists(_)) => {
                s.skipped += 1;
                out.report
                    .note(&m.source, 0, "already in the vault, not written");
            }
            Err(e) => {
                out.report.note(
                    &m.source,
                    0,
                    format!("could not be written ({e}), the import stopped here"),
                );
                failed = Some(e);
                break;
            }
        }
    }
    for (src, rel) in assets {
        if failed.is_some() {
            break;
        }
        let dst = vault.abs(&join(dest, &rel));
        if dst.exists() {
            s.skipped += 1;
            out.report
                .note(&rel, 0, "already in the vault, not written");
            continue;
        }
        let copy = || -> Result<()> {
            if let Some(dir) = dst.parent() {
                std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
            }
            std::fs::copy(&src, &dst).map_err(|e| Error::io(&dst, e))?;
            Ok(())
        };
        match copy() {
            Ok(()) => s.assets += 1,
            Err(e) => {
                out.report.note(
                    &rel,
                    0,
                    format!("could not be copied ({e}), the import stopped here"),
                );
                failed = Some(e);
            }
        }
    }
    let summary = format!(
        "Imported {} pages, {} journals and {} assets from `{}` on {}. {} files were already in the vault and were left alone.{}",
        s.pages,
        s.journals,
        s.assets,
        graph.display(),
        chrono::Local::now().date_naive(),
        s.skipped,
        if failed.is_some() {
            " The import stopped on an error and is incomplete."
        } else {
            ""
        }
    );
    let written = vault.write(&s.report, &out.report.render(&summary));
    match failed {
        Some(e) => Err(e),
        None => {
            written?;
            Ok(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn run_writes_pages_assets_and_report_and_never_overwrites() {
        let graph = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/logseq");
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        vault.create("notes.md", "mine").unwrap();
        let cfg = AppConfig::default();
        let s = run(&vault, &cfg, &graph, "In/").unwrap();
        assert_eq!((s.pages, s.journals, s.assets, s.skipped), (6, 2, 1, 0));
        assert_eq!(s.report, "In/import-report.md");
        assert!(d.path().join("In/Project/Alpha.md").is_file());
        assert!(d.path().join("Daily/2026-09-14.md").is_file());
        assert!(d.path().join("In/journals/notadate.md").is_file());
        assert!(d.path().join("In/assets/diagram.png").is_file());
        assert_eq!(vault.read("notes.md").unwrap(), "mine");
        let report = vault.read("In/import-report.md").unwrap();
        assert!(report.contains("### `whiteboards/board.edn`"), "{report}");
        assert!(!report.contains("logseq/config.edn"), "{report}");
        // Running again writes nothing and says why.
        let s = run(&vault, &cfg, &graph, "In/").unwrap();
        assert_eq!((s.pages, s.journals, s.assets), (0, 0, 0));
        assert_eq!(s.skipped, 9);
        // The files left alone are not constructs the mapping could not map.
        assert_eq!(s.unmapped, 9);
        let report = vault.read("In/import-report.md").unwrap();
        assert!(
            report.contains("already in the vault, not written"),
            "{report}"
        );
    }

    #[test]
    fn a_non_ascii_id_is_not_sliced_mid_character() {
        let cfg = AppConfig::default();
        let files = vec![SourceFile {
            path: "pages/a.md".into(),
            text: "- one\n  id:: aaaaaaa\u{e9}b\n".into(),
        }];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        assert_eq!(out.pages[0].text, "- one ^aaaaaaa\u{e9}\n");
    }

    #[test]
    fn an_id_on_a_code_block_is_reported_and_not_linked() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text:
                    "- ```rust\n  fn x() {}\n  ```\n  id:: 64f1a2b3-0000-4000-8000-000000000001\n"
                        .into(),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: "- ((64f1a2b3-0000-4000-8000-000000000001))\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        assert!(!out.pages[0].text.contains('^'), "{}", out.pages[0].text);
        assert_eq!(
            out.pages[1].text,
            "- ((64f1a2b3-0000-4000-8000-000000000001))\n"
        );
        let whats: Vec<_> = out.report.entries.iter().map(|e| e.what.as_str()).collect();
        assert!(
            whats
                .iter()
                .any(|w| w.starts_with("`id::` on a block that ends in a code block")),
            "{whats:?}"
        );
    }

    #[test]
    fn an_id_on_a_block_that_ends_in_a_code_block_is_reported_and_not_linked() {
        let id = "64f1a2b3-0000-4000-8000-000000000001";
        let out = map_one(
            "pages/a.md",
            &format!("- notes\n  id:: {id}\n  ```\n  x\n  ```\n"),
        );
        assert!(!out.pages[0].text.contains('^'), "{}", out.pages[0].text);
        let whats: Vec<_> = out.report.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(
            whats,
            vec![
                "`id::` on a block that ends in a code block cannot become an anchor, references to it are kept as text"
            ]
        );
    }

    #[test]
    fn an_anchor_goes_on_the_last_line_of_a_block_that_has_more_than_one() {
        let id = "64f1a2b3-0000-4000-8000-000000000001";
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text: format!("- notes\n  id:: {id}\n  kept:: yes\n"),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: format!("- (({id}))\n"),
            },
        ];
        let out = map(&files);
        assert_eq!(out.pages[0].text, "- notes\n  kept:: yes ^64f1a2b3\n");
        assert_eq!(out.pages[1].text, "- [[a#^64f1a2b3]]\n");
    }

    #[test]
    fn a_page_id_is_a_link_to_the_page_and_leaves_the_frontmatter() {
        let id = "64f1a2b3-0000-4000-8000-000000000001";
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text: format!("- id:: {id}\n  public:: true\n- body\n"),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: format!("- (({id})) and {{{{embed (({id}))}}}}\n"),
            },
        ];
        let out = map(&files);
        assert_eq!(out.pages[0].text, "---\npublic: true\n---\n- body\n");
        assert_eq!(out.pages[1].text, "- [[a]] and ![[a]]\n");
        assert!(out.report.is_empty(), "{:?}", out.report.entries);
    }

    #[test]
    fn the_report_gives_way_to_a_page_that_wants_its_name() {
        let graph = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(graph.path().join("pages")).unwrap();
        std::fs::write(graph.path().join("pages/import-report.md"), "- mine\n").unwrap();
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        let s = run(&vault, &AppConfig::default(), graph.path(), "").unwrap();
        assert_eq!(s.report, "import-report-1.md");
        assert_eq!(vault.read("import-report.md").unwrap(), "- mine\n");
        assert!(
            vault
                .read("import-report-1.md")
                .unwrap()
                .contains("Imported 1 pages")
        );
    }

    #[test]
    fn an_uppercase_extension_is_still_markdown() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/Note.MD".into(),
                text: "- x\n".into(),
            },
            SourceFile {
                path: "journals/2026_01_02.MD".into(),
                text: "- j\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["Note.md", "Daily/2026-01-02.md"]);
        assert!(out.report.is_empty(), "{:?}", out.report.entries);
    }

    #[test]
    fn a_title_that_ends_in_md_keeps_its_whole_name_in_links() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text: "title:: My Notes.md\n\n- one\n  id:: 64f1a2b3-0000-4000-8000-000000000001\n"
                    .into(),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: "- ((64f1a2b3-0000-4000-8000-000000000001))\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        assert_eq!(out.pages[0].path, "My Notes.md.md");
        assert_eq!(out.pages[1].text, "- [[My Notes.md#^64f1a2b3]]\n");
    }

    #[test]
    fn two_pages_that_want_one_file_keep_the_first_and_report_the_second() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text: "title:: b\n\n- from a\n".into(),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: "- from b\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["b.md"]);
        assert_eq!(out.pages[0].text, "\n- from a\n");
        let e = out.report.entries.last().unwrap();
        assert_eq!(e.file, "pages/b.md");
        assert_eq!(
            e.what,
            "maps to `b.md`, which `pages/a.md` already uses, not imported"
        );
    }

    #[test]
    fn two_targets_that_differ_only_in_case_are_both_imported_and_reported() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/Notes.md".into(),
                text: "- upper\n".into(),
            },
            SourceFile {
                path: "pages/notes.md".into(),
                text: "- lower\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "",
                config: &cfg,
            },
        );
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["Notes.md", "notes.md"]);
        let e = out.report.entries.last().unwrap();
        assert_eq!(e.file, "pages/notes.md");
        assert_eq!(
            e.what,
            "maps to `notes.md`, which differs only in case from the target of `pages/Notes.md`; a filesystem that ignores case keeps only one of them"
        );
    }

    fn map_one(path: &str, text: &str) -> Output {
        map(&[SourceFile {
            path: path.into(),
            text: text.into(),
        }])
    }

    #[test]
    fn repeated_properties_merge_their_lists_and_report_the_rest() {
        let out = map_one(
            "pages/a.md",
            "alias:: A\naliases:: B, A\ntags:: x\ntags:: y\nkind:: one\nkind:: two\n\n- x\n",
        );
        assert!(
            out.pages[0]
                .text
                .starts_with("---\naliases:\n- A\n- B\ntags:\n- x\n- y\nkind: one\n---\n"),
            "{}",
            out.pages[0].text
        );
        let whats: Vec<_> = out.report.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(
            whats,
            vec!["property `kind:: two` repeats `kind`, the first value is kept"]
        );
    }

    #[test]
    fn a_property_that_looks_like_a_number_keeps_how_it_was_written() {
        let out = map_one(
            "pages/a.md",
            "isbn:: 0012345\nversion:: 1.10\nbuild:: 1.5\ncount:: 12\n\n- x\n",
        );
        assert!(
            out.pages[0]
                .text
                .contains("isbn: '0012345'\nversion: '1.10'\nbuild: 1.5\ncount: 12\n"),
            "{}",
            out.pages[0].text
        );
    }

    #[test]
    fn one_uuid_on_two_pages_keeps_the_first_and_reports_the_second() {
        let id = "64f1a2b3-0000-4000-8000-000000000001";
        let files = vec![
            SourceFile {
                path: "pages/a.md".into(),
                text: format!("- from a\n  id:: {id}\n"),
            },
            SourceFile {
                path: "pages/b.md".into(),
                text: format!("- from b\n  id:: {id}\n"),
            },
            SourceFile {
                path: "pages/c.md".into(),
                text: format!("- (({id}))\n"),
            },
        ];
        let out = map(&files);
        assert_eq!(out.pages[0].text, "- from a ^64f1a2b3\n");
        assert_eq!(out.pages[1].text, "- from b\n");
        assert_eq!(out.pages[2].text, "- [[a#^64f1a2b3]]\n");
        let e = out.report.entries.last().unwrap();
        assert_eq!((e.file.as_str(), e.line), ("pages/b.md", 1));
        assert_eq!(
            e.what,
            format!(
                "`id:: {id}` is already used by `pages/a.md`, references to it point there and this block has no anchor"
            )
        );
    }

    #[test]
    fn a_file_name_that_leaves_the_folder_or_hides_the_page_is_made_usable() {
        let cfg = AppConfig::default();
        let files = vec![
            SourceFile {
                path: "pages/..___etc___x.md".into(),
                text: "- x\n".into(),
            },
            SourceFile {
                path: "pages/a___.md".into(),
                text: "- y\n".into(),
            },
        ];
        let out = map_graph(
            &files,
            &Options {
                dest: "Logseq",
                config: &cfg,
            },
        );
        let paths: Vec<_> = out.pages.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["Logseq/etc/x.md", "Logseq/a.md"]);
        let whats: Vec<_> = out.report.entries.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(
            whats,
            vec![
                "`../etc/x` cannot be a file name, imported as `etc/x`",
                "`a/` cannot be a file name, imported as `a`"
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_text_is_reported_not_imported_as_empty() {
        let graph = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(graph.path().join("pages")).unwrap();
        std::fs::write(graph.path().join("pages/binary.md"), [0xff, 0xfe, b'a']).unwrap();
        std::fs::write(graph.path().join("pages/ok.md"), "- x\n").unwrap();
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        let s = run(&vault, &AppConfig::default(), graph.path(), "").unwrap();
        assert_eq!(s.pages, 1);
        assert!(!d.path().join("binary.md").exists());
        let report = vault.read("import-report.md").unwrap();
        assert!(report.contains("### `pages/binary.md`"), "{report}");
        assert!(report.contains("could not be read as text"), "{report}");
    }

    #[test]
    fn a_page_that_cannot_be_written_still_leaves_a_report() {
        let graph = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(graph.path().join("pages")).unwrap();
        std::fs::write(graph.path().join("pages/a___b.md"), "- x\n").unwrap();
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        // `a` is a file, so the folder `a/b.md` needs cannot be made.
        vault.create("a", "in the way").unwrap();
        let err = run(&vault, &AppConfig::default(), graph.path(), "").unwrap_err();
        assert!(matches!(err, Error::Io { .. }), "{err}");
        let report = vault.read("import-report.md").unwrap();
        assert!(report.contains("### `pages/a___b.md`"), "{report}");
        assert!(report.contains("could not be written"), "{report}");
        assert!(report.contains("stopped on an error"), "{report}");
    }

    #[test]
    fn run_refuses_a_folder_that_is_not_a_graph() {
        let d = tempfile::tempdir().unwrap();
        let vault = Vault::open(d.path()).unwrap();
        let err = run(&vault, &AppConfig::default(), d.path(), "").unwrap_err();
        assert!(err.to_string().contains("not a Logseq graph"), "{err}");
    }
}
