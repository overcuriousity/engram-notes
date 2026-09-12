//! Obsidian's `.base` files: filters and formulas over notes, shown as views.

pub mod eval;
pub mod expr;

use crate::index::Index;
use crate::{Error, Result};
use chrono::NaiveDateTime;
use eval::{Ctx, NoteData, Value};
use expr::{Expr, Scope};
use std::collections::{BTreeMap, HashMap};

/// What Obsidian writes for a new base.
pub const NEW_BASE: &str = "views:\n  - type: table\n    name: Table\n";

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct BaseFile {
    filters: Option<Filter>,
    formulas: BTreeMap<String, String>,
    properties: BTreeMap<String, PropertyConfig>,
    summaries: Option<serde_yaml_ng::Value>,
    views: Vec<View>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum Filter {
    Expr(String),
    And { and: Vec<Filter> },
    Or { or: Vec<Filter> },
    Not { not: Vec<Filter> },
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct PropertyConfig {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
struct View {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    limit: Option<usize>,
    filters: Option<Filter>,
    order: Vec<String>,
    sort: Vec<SortKey>,
    #[serde(rename = "groupBy")]
    group_by: Option<serde_yaml_ng::Value>,
    summaries: Option<serde_yaml_ng::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SortKey {
    pub property: String,
    #[serde(default = "ascending")]
    pub direction: String,
}

fn ascending() -> String {
    "ASC".into()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Column {
    pub id: String,
    pub label: String,
    /// Note properties edit the note's frontmatter; file fields and formulas do not.
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Row {
    pub path: String,
    pub cells: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Table {
    pub views: Vec<String>,
    pub view: usize,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub sort: Vec<SortKey>,
    /// What the base asks for outside the supported subset.
    pub errors: Vec<String>,
}

enum Cond {
    Expr(Expr),
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Vec<Cond>),
}

fn compile(f: &Filter) -> std::result::Result<Cond, String> {
    let all = |fs: &[Filter]| {
        fs.iter()
            .map(compile)
            .collect::<std::result::Result<Vec<_>, _>>()
    };
    Ok(match f {
        Filter::Expr(s) => Cond::Expr(expr::parse(s)?),
        Filter::And { and } => Cond::And(all(and)?),
        Filter::Or { or } => Cond::Or(all(or)?),
        Filter::Not { not } => Cond::Not(all(not)?),
    })
}

// `not` holds when none of its filters do.
fn holds(c: &Cond, cx: &Ctx) -> std::result::Result<bool, String> {
    Ok(match c {
        Cond::Expr(e) => eval::truthy(&cx.eval(e)?),
        Cond::And(cs) => cs
            .iter()
            .try_fold(true, |ok, c| Ok::<_, String>(ok && holds(c, cx)?))?,
        Cond::Or(cs) => cs
            .iter()
            .try_fold(false, |ok, c| Ok::<_, String>(ok || holds(c, cx)?))?,
        Cond::Not(cs) => !cs
            .iter()
            .try_fold(false, |ok, c| Ok::<_, String>(ok || holds(c, cx)?))?,
    })
}

/// A column id as an expression; a bare name is a note property.
fn column(id: &str) -> std::result::Result<(String, Expr), String> {
    match id.split_once('.') {
        Some(("file", _)) => Ok((id.to_owned(), expr::parse(id)?)),
        Some(("note", k)) => Ok((id.to_owned(), Expr::Field(Scope::Note, k.into()))),
        Some(("formula", k)) => Ok((id.to_owned(), Expr::Field(Scope::Formula, k.into()))),
        _ => Ok((format!("note.{id}"), Expr::Field(Scope::Note, id.into()))),
    }
}

fn yaml_err(e: serde_yaml_ng::Error) -> Error {
    Error::Base(e.to_string())
}

fn parse_base(text: &str) -> Result<BaseFile> {
    if text.trim().is_empty() {
        return Ok(BaseFile::default());
    }
    serde_yaml_ng::from_str(text).map_err(yaml_err)
}

fn push_once(errors: &mut Vec<String>, e: String) {
    if !errors.contains(&e) {
        errors.push(e);
    }
}

pub fn run(text: &str, notes: &[NoteData], view: usize, now: NaiveDateTime) -> Result<Table> {
    let base = parse_base(text)?;
    let mut views = base.views.clone();
    if views.is_empty() {
        views.push(View {
            kind: "table".into(),
            name: "Table".into(),
            ..Default::default()
        });
    }
    let view = if view < views.len() { view } else { 0 };
    let v = &views[view];
    let mut errors = Vec::new();
    if !v.kind.is_empty() && v.kind != "table" {
        errors.push(format!(
            "unsupported view type `{}`, shown as a table",
            v.kind
        ));
    }
    if v.group_by.is_some() {
        errors.push("unsupported `groupBy`, rows are not grouped".into());
    }
    if v.summaries.is_some() || base.summaries.is_some() {
        errors.push("unsupported `summaries`".into());
    }

    let mut formulas = HashMap::new();
    for (name, src) in &base.formulas {
        match expr::parse(src) {
            Ok(e) => {
                formulas.insert(name.clone(), e);
            }
            Err(e) => errors.push(format!("formula `{name}`: {e}")),
        }
    }
    let mut conds = Vec::new();
    let mut filter_failed = false;
    for f in [&base.filters, &v.filters].into_iter().flatten() {
        match compile(f) {
            Ok(c) => conds.push(c),
            Err(e) => {
                errors.push(format!("filter: {e}"));
                filter_failed = true;
            }
        }
    }

    let order = if v.order.is_empty() {
        vec!["file.name".to_owned()]
    } else {
        v.order.clone()
    };
    let mut columns = Vec::new();
    let mut exprs = Vec::new();
    for id in &order {
        let (id, e) = match column(id) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("column `{id}`: {e}"));
                (id.clone(), Expr::Null)
            }
        };
        let (scope, key) = id.split_once('.').unwrap_or(("note", &id));
        let label = base
            .properties
            .get(&id)
            .or_else(|| base.properties.get(key).filter(|_| scope == "note"))
            .and_then(|p| p.display_name.clone())
            .unwrap_or_else(|| {
                if scope == "file" {
                    format!("file {key}")
                } else {
                    key.to_owned()
                }
            });
        columns.push(Column {
            editable: scope == "note",
            label,
            id: id.clone(),
        });
        exprs.push(e);
    }
    let sort_exprs: Vec<Option<Expr>> = v
        .sort
        .iter()
        .map(|k| column(&k.property).ok().map(|c| c.1))
        .collect();

    let mut rows: Vec<(Vec<Value>, Row)> = Vec::new();
    if !filter_failed {
        for note in notes {
            let cx = Ctx {
                note,
                formulas: &formulas,
                now,
            };
            let keep = conds
                .iter()
                .try_fold(true, |ok, c| Ok::<_, String>(ok && holds(c, &cx)?));
            match keep {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    push_once(&mut errors, e);
                    continue;
                }
            }
            let mut cells = Vec::new();
            for (c, e) in columns.iter().zip(&exprs) {
                // Note properties pass through as written, so editors see the file's value.
                let cell = match (c.editable, c.id.split_once('.')) {
                    (true, Some((_, key))) => note.properties.get(key).cloned().unwrap_or_default(),
                    _ => cx.eval(e).map(|v| eval::to_json(&v)).unwrap_or_else(|err| {
                        push_once(&mut errors, err);
                        serde_json::Value::Null
                    }),
                };
                cells.push(cell);
            }
            let keys = sort_exprs
                .iter()
                .map(|e| {
                    e.as_ref()
                        .and_then(|e| cx.eval(e).ok())
                        .unwrap_or(Value::Null)
                })
                .collect();
            rows.push((
                keys,
                Row {
                    path: note.path.clone(),
                    cells,
                },
            ));
        }
    }
    rows.sort_by(|(a, _), (b, _)| {
        for (i, k) in v.sort.iter().enumerate() {
            let (x, y) = (&a[i], &b[i]);
            // Empty values sort last in either direction.
            let ord = match (x == &Value::Null, y == &Value::Null) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => {
                    let o = eval::compare(x, y)
                        .unwrap_or_else(|| eval::display(x).cmp(&eval::display(y)));
                    if k.direction.eq_ignore_ascii_case("desc") {
                        o.reverse()
                    } else {
                        o
                    }
                }
            };
            if ord.is_ne() {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    let limit = v.limit.unwrap_or(usize::MAX);
    Ok(Table {
        views: views
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if v.name.is_empty() {
                    format!("View {}", i + 1)
                } else {
                    v.name.clone()
                }
            })
            .collect(),
        view,
        columns,
        rows: rows.into_iter().take(limit).map(|(_, r)| r).collect(),
        sort: v.sort.clone(),
        errors,
    })
}

/// The base text with one view's sort replaced; Obsidian rewrites the file the same way.
pub fn set_sort(text: &str, view: usize, sort: &[SortKey]) -> Result<String> {
    use serde_yaml_ng::{Mapping, Value as Yaml};
    let mut doc = if text.trim().is_empty() {
        Yaml::Mapping(Mapping::new())
    } else {
        serde_yaml_ng::from_str(text).map_err(yaml_err)?
    };
    let not_a_base = || Error::Base("a base file must be a mapping with a list of views".into());
    let map = doc.as_mapping_mut().ok_or_else(not_a_base)?;
    if !map.contains_key("views") {
        map.insert("views".into(), Yaml::Sequence(vec![]));
    }
    let views = map
        .get_mut("views")
        .and_then(Yaml::as_sequence_mut)
        .ok_or_else(not_a_base)?;
    if views.is_empty() {
        let mut m = Mapping::new();
        m.insert("type".into(), "table".into());
        m.insert("name".into(), "Table".into());
        views.push(Yaml::Mapping(m));
    }
    let v = views
        .get_mut(view)
        .and_then(Yaml::as_mapping_mut)
        .ok_or_else(|| Error::Base(format!("no view {view}")))?;
    if sort.is_empty() {
        v.remove("sort");
    } else {
        v.insert(
            "sort".into(),
            serde_yaml_ng::to_value(sort).map_err(yaml_err)?,
        );
    }
    serde_yaml_ng::to_string(&doc).map_err(yaml_err)
}

pub fn notes(index: &Index) -> Result<Vec<NoteData>> {
    let conn = index.conn();
    let pairs = |sql: &str| -> Result<HashMap<String, Vec<String>>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        let mut stmt = conn.prepare(sql)?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = row?;
            out.entry(k).or_default().push(v);
        }
        Ok(out)
    };
    let mut tags = pairs("SELECT path, tag FROM tags ORDER BY tag")?;
    let mut links = pairs(
        "SELECT src_path, target_path FROM links WHERE target_path IS NOT NULL ORDER BY src_path, start",
    )?;
    let mut stmt =
        conn.prepare("SELECT path, mtime_ms, size, frontmatter FROM notes ORDER BY path")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (path, mtime_ms, size, fm) = row?;
        out.push(NoteData {
            properties: serde_json::from_str(&fm).unwrap_or_default(),
            tags: tags.remove(&path).unwrap_or_default(),
            links: links.remove(&path).unwrap_or_default(),
            path,
            mtime_ms,
            size,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data() -> Vec<NoteData> {
        let props = |v: serde_json::Value| v.as_object().unwrap().clone();
        vec![
            NoteData {
                path: "Books/Dune.md".into(),
                properties: props(json!({"status": "reading", "rating": 5})),
                tags: vec!["book".into()],
                ..Default::default()
            },
            NoteData {
                path: "Books/Emma.md".into(),
                properties: props(json!({"status": "done", "rating": 3})),
                tags: vec!["book".into(), "classic".into()],
                ..Default::default()
            },
            NoteData {
                path: "Inbox.md".into(),
                ..Default::default()
            },
        ]
    }

    fn now() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 9, 13)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    const BOOKS: &str = r#"
filters:
  and:
    - file.hasTag("book")
formulas:
  double: "rating * 2"
properties:
  note.status:
    displayName: State
views:
  - type: table
    name: Reading
    filters:
      not:
        - 'status == "done"'
    order: [file.name, note.status, formula.double]
  - type: table
    name: All books
    order: [file.basename, rating]
    sort:
      - property: rating
        direction: ASC
    limit: 1
"#;

    #[test]
    fn filters_columns_formulas() {
        let t = run(BOOKS, &data(), 0, now()).unwrap();
        assert_eq!(t.views, vec!["Reading", "All books"]);
        let cols: Vec<_> = t
            .columns
            .iter()
            .map(|c| (c.id.as_str(), c.label.as_str(), c.editable))
            .collect();
        assert_eq!(
            cols,
            vec![
                ("file.name", "file name", false),
                ("note.status", "State", true),
                ("formula.double", "double", false)
            ]
        );
        assert_eq!(
            t.rows,
            vec![Row {
                path: "Books/Dune.md".into(),
                cells: vec![json!("Dune.md"), json!("reading"), json!(10)]
            }]
        );
        assert!(t.errors.is_empty(), "{:?}", t.errors);
    }

    #[test]
    fn sort_and_limit_per_view() {
        let t = run(BOOKS, &data(), 1, now()).unwrap();
        assert_eq!(t.view, 1);
        assert_eq!(
            t.rows,
            vec![Row {
                path: "Books/Emma.md".into(),
                cells: vec![json!("Emma"), json!(3)]
            }]
        );
        assert_eq!(t.columns[1].id, "note.rating");
        assert_eq!(t.sort[0].direction, "ASC");
    }

    #[test]
    fn empty_base_lists_every_note_by_name() {
        let t = run("", &data(), 0, now()).unwrap();
        assert_eq!(t.views, vec!["Table"]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.columns[0].id, "file.name");
    }

    #[test]
    fn unsupported_is_reported_not_guessed() {
        let text = "filters: 'file.ctime > now()'\nformulas:\n  x: 'link(\"a\")'\nviews:\n  - type: cards\n    groupBy:\n      property: note.status\n      direction: ASC\n";
        let t = run(text, &data(), 0, now()).unwrap();
        assert!(t.rows.is_empty());
        let all = t.errors.join("\n");
        for what in ["file.ctime", "link()", "cards", "groupBy"] {
            assert!(all.contains(what), "{what} missing from {all}");
        }
        assert!(run("views: [", &data(), 0, now()).is_err());
    }

    #[test]
    fn set_sort_keeps_the_rest() {
        let key = SortKey {
            property: "file.name".into(),
            direction: "DESC".into(),
        };
        let out = set_sort(BOOKS, 0, std::slice::from_ref(&key)).unwrap();
        let t = run(&out, &data(), 0, now()).unwrap();
        assert_eq!(t.sort, vec![key]);
        assert_eq!(t.views, vec!["Reading", "All books"]);
        assert_eq!(t.rows.len(), 1);
        let out = set_sort(&out, 0, &[]).unwrap();
        assert!(run(&out, &data(), 0, now()).unwrap().sort.is_empty());
        assert!(set_sort("", 0, &[]).is_ok());
    }

    #[test]
    fn notes_come_from_the_index() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "---\nstatus: open\n---\n[[B]] #x").unwrap();
        std::fs::write(d.path().join("B.md"), "").unwrap();
        let v = crate::vault::Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let n = notes(&ix).unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].properties["status"], json!("open"));
        assert_eq!(n[0].tags, vec!["x"]);
        assert_eq!(n[0].links, vec!["B.md"]);
    }
}
