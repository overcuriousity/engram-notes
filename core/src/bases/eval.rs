//! Evaluating an expression against one note.

use super::expr::{Expr, Op, Scope};
use chrono::{Months, NaiveDate, NaiveDateTime, TimeDelta};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Date(NaiveDateTime),
    List(Vec<Value>),
}

/// What an expression sees of one note.
#[derive(Debug, Clone, Default)]
pub struct NoteData {
    pub path: String,
    pub mtime_ms: i64,
    pub size: i64,
    pub properties: serde_json::Map<String, serde_json::Value>,
    /// Without `#`, as the index stores them.
    pub tags: Vec<String>,
    /// Resolved note paths.
    pub links: Vec<String>,
}

pub struct Ctx<'a> {
    pub note: &'a NoteData,
    pub formulas: &'a HashMap<String, Expr>,
    pub now: NaiveDateTime,
}

// Deeper than this, a formula is taken to refer to itself.
const MAX_DEPTH: usize = 32;

static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:-?\d+\s*[A-Za-z]+\s*)+$").unwrap());
static DURATION_PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(-?\d+)\s*([A-Za-z]+)").unwrap());

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Num(n) => *n != 0.0 && !n.is_nan(),
        Value::Str(s) => !s.is_empty(),
        Value::Date(_) => true,
        Value::List(l) => !l.is_empty(),
    }
}

pub fn from_json(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => n.as_f64().map_or(Value::Null, Value::Num),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(from_json).collect()),
        serde_json::Value::Null | serde_json::Value::Object(_) => Value::Null,
    }
}

fn parse_date(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0);
    }
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ]
    .iter()
    .find_map(|f| NaiveDateTime::parse_from_str(s, f).ok())
}

fn as_date(v: &Value) -> Option<NaiveDateTime> {
    match v {
        Value::Date(d) => Some(*d),
        Value::Str(s) => parse_date(s),
        _ => None,
    }
}

fn is_midnight(d: &NaiveDateTime) -> bool {
    d.time() == chrono::NaiveTime::MIN
}

pub fn display(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
        Value::Num(n) => n.to_string(),
        Value::Str(s) => s.clone(),
        Value::Date(d) if is_midnight(d) => d.format("%Y-%m-%d").to_string(),
        Value::Date(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::List(l) => l.iter().map(display).collect::<Vec<_>>().join(", "),
    }
}

/// A cell value: dates in the form the property editors read.
pub fn to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => (*b).into(),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => (*n as i64).into(),
        Value::Num(n) => {
            serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, Into::into)
        }
        Value::Str(s) => s.clone().into(),
        Value::Date(d) if is_midnight(d) => d.format("%Y-%m-%d").to_string().into(),
        Value::Date(d) => d.format("%Y-%m-%dT%H:%M:%S").to_string().into(),
        Value::List(l) => l.iter().map(to_json).collect::<Vec<_>>().into(),
    }
}

/// Loose equality: a date equals a string naming the same moment.
pub fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Date(_), Value::Str(_)) | (Value::Str(_), Value::Date(_)) => {
            as_date(a).is_some() && as_date(a) == as_date(b)
        }
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equal(p, q))
        }
        _ => a == b,
    }
}

pub fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Date(_), _) | (_, Value::Date(_)) => Some(as_date(a)?.cmp(&as_date(b)?)),
        (Value::Str(x), Value::Str(y)) => Some(x.to_lowercase().cmp(&y.to_lowercase())),
        _ => None,
    }
}

/// `date + "1M"`: Obsidian's duration units, several parts allowed.
fn shift(date: NaiveDateTime, text: &str, sign: i64) -> Result<NaiveDateTime, String> {
    let bad = || format!("`{text}` is not a duration");
    if !DURATION.is_match(text) {
        return Err(bad());
    }
    let mut out = date;
    for c in DURATION_PART.captures_iter(text) {
        let n: i64 = c[1].parse::<i64>().map_err(|_| bad())? * sign;
        let unit = &c[2];
        let months = match unit {
            "M" => Some(1),
            "y" => Some(12),
            _ => match unit.to_lowercase().as_str() {
                "year" | "years" => Some(12),
                "month" | "months" => Some(1),
                _ => None,
            },
        };
        if let Some(m) = months {
            let span = Months::new((n.unsigned_abs() * m) as u32);
            out = if n < 0 {
                out.checked_sub_months(span)
            } else {
                out.checked_add_months(span)
            }
            .ok_or_else(bad)?;
            continue;
        }
        let secs = match unit.to_lowercase().as_str() {
            "w" | "week" | "weeks" => 604_800,
            "d" | "day" | "days" => 86_400,
            "h" | "hour" | "hours" => 3_600,
            "m" | "minute" | "minutes" => 60,
            "s" | "second" | "seconds" => 1,
            _ => return Err(bad()),
        };
        out = out
            .checked_add_signed(TimeDelta::seconds(n * secs))
            .ok_or_else(bad)?;
    }
    Ok(out)
}

fn names_link(link_path: &str, target: &str) -> bool {
    let l = link_path.to_lowercase();
    let l = l.strip_suffix(".md").unwrap_or(&l);
    let t = target.trim().to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t);
    l == t || l.rsplit('/').next() == Some(t)
}

impl Ctx<'_> {
    pub fn eval(&self, e: &Expr) -> Result<Value, String> {
        self.at(e, 0)
    }

    fn at(&self, e: &Expr, depth: usize) -> Result<Value, String> {
        let ev = |x: &Expr| self.at(x, depth);
        Ok(match e {
            Expr::Null => Value::Null,
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Field(Scope::Note, k) => {
                self.note.properties.get(k).map_or(Value::Null, from_json)
            }
            Expr::Field(Scope::Formula, k) => {
                let f = self
                    .formulas
                    .get(k)
                    .ok_or_else(|| format!("unknown formula `{k}`"))?;
                if depth >= MAX_DEPTH {
                    return Err(format!("formula `{k}` refers to itself"));
                }
                self.at(f, depth + 1)?
            }
            Expr::Field(Scope::File, k) => self.file_field(k),
            Expr::Unary('!', x) => Value::Bool(!truthy(&ev(x)?)),
            Expr::Unary(_, x) => match ev(x)? {
                Value::Num(n) => Value::Num(-n),
                _ => Value::Null,
            },
            Expr::Binary(l, op, r) => self.binary(ev(l)?, *op, r, depth)?,
            Expr::Call(name, args) => self.call(name, args, depth)?,
            Expr::Method(recv, name, args) => {
                let v = ev(recv)?;
                let args = args.iter().map(ev).collect::<Result<Vec<_>, _>>()?;
                match (name.as_str(), args.as_slice()) {
                    ("isEmpty", []) => {
                        Value::Bool(!truthy(&v) && !matches!(v, Value::Bool(_) | Value::Num(_)))
                    }
                    ("contains", [x]) => Value::Bool(match (&v, x) {
                        (Value::Str(s), Value::Str(n)) => s.contains(n.as_str()),
                        (Value::List(l), x) => l.iter().any(|i| equal(i, x)),
                        _ => false,
                    }),
                    _ => return Err(format!("`.{name}()` got {} arguments", args.len())),
                }
            }
        })
    }

    fn file_field(&self, k: &str) -> Value {
        let path = &self.note.path;
        let name = path.rsplit('/').next().unwrap_or(path);
        match k {
            "name" => Value::Str(name.into()),
            "basename" => Value::Str(name.rsplit_once('.').map_or(name, |(b, _)| b).into()),
            "path" => Value::Str(path.clone()),
            // Obsidian's root folder is `/`.
            "folder" => Value::Str(path.rsplit_once('/').map_or("/", |(f, _)| f).into()),
            "ext" => Value::Str(name.rsplit_once('.').map_or("", |(_, e)| e).into()),
            "size" => Value::Num(self.note.size as f64),
            "mtime" => chrono::DateTime::from_timestamp_millis(self.note.mtime_ms)
                .map_or(Value::Null, |d| {
                    Value::Date(d.with_timezone(&chrono::Local).naive_local())
                }),
            "tags" => Value::List(
                self.note
                    .tags
                    .iter()
                    .map(|t| Value::Str(format!("#{t}")))
                    .collect(),
            ),
            "links" => Value::List(
                self.note
                    .links
                    .iter()
                    .map(|l| Value::Str(l.clone()))
                    .collect(),
            ),
            _ => Value::Null,
        }
    }

    fn binary(&self, a: Value, op: Op, r: &Expr, depth: usize) -> Result<Value, String> {
        // `&&` and `||` evaluate their right side only when it matters.
        match op {
            Op::And if !truthy(&a) => return Ok(Value::Bool(false)),
            Op::Or if truthy(&a) => return Ok(Value::Bool(true)),
            _ => {}
        }
        let b = self.at(r, depth)?;
        let ord = || compare(&a, &b);
        Ok(match op {
            Op::And | Op::Or => Value::Bool(truthy(&b)),
            Op::Eq => Value::Bool(equal(&a, &b)),
            Op::Ne => Value::Bool(!equal(&a, &b)),
            Op::Lt => Value::Bool(ord() == Some(Ordering::Less)),
            Op::Le => Value::Bool(matches!(ord(), Some(Ordering::Less | Ordering::Equal))),
            Op::Gt => Value::Bool(ord() == Some(Ordering::Greater)),
            Op::Ge => Value::Bool(matches!(ord(), Some(Ordering::Greater | Ordering::Equal))),
            Op::Add | Op::Sub => match (&a, &b, op) {
                (Value::Num(x), Value::Num(y), Op::Add) => Value::Num(x + y),
                (Value::Num(x), Value::Num(y), _) => Value::Num(x - y),
                (Value::Date(d), Value::Str(s), _) => {
                    Value::Date(shift(*d, s, if op == Op::Add { 1 } else { -1 })?)
                }
                (Value::Date(x), Value::Date(y), Op::Sub) => {
                    Value::Num((*x - *y).num_milliseconds() as f64)
                }
                (Value::Str(_), _, Op::Add) | (_, Value::Str(_), Op::Add) => {
                    Value::Str(display(&a) + &display(&b))
                }
                _ => Value::Null,
            },
            Op::Mul | Op::Div | Op::Mod => match (&a, &b) {
                (Value::Num(x), Value::Num(y)) => Value::Num(match op {
                    Op::Mul => x * y,
                    Op::Div => x / y,
                    _ => x % y,
                }),
                _ => Value::Null,
            },
        })
    }

    fn call(&self, name: &str, args: &[Expr], depth: usize) -> Result<Value, String> {
        let ev = |x: &Expr| self.at(x, depth);
        let arity = |n: std::ops::RangeInclusive<usize>| {
            if n.contains(&args.len()) {
                Ok(())
            } else {
                Err(format!("`{name}()` got {} arguments", args.len()))
            }
        };
        let strings = || -> Result<Vec<String>, String> {
            args.iter().map(|a| ev(a).map(|v| display(&v))).collect()
        };
        Ok(match name {
            "now" => {
                arity(0..=0)?;
                Value::Date(self.now)
            }
            "today" => {
                arity(0..=0)?;
                Value::Date(self.now.date().and_time(chrono::NaiveTime::MIN))
            }
            "date" => {
                arity(1..=1)?;
                as_date(&ev(&args[0])?).map_or(Value::Null, Value::Date)
            }
            "if" => {
                arity(2..=3)?;
                if truthy(&ev(&args[0])?) {
                    ev(&args[1])?
                } else if let Some(f) = args.get(2) {
                    ev(f)?
                } else {
                    Value::Null
                }
            }
            "file.hasTag" => {
                arity(1..=usize::MAX)?;
                let tags = &self.note.tags;
                Value::Bool(strings()?.iter().any(|want| {
                    let want = want.trim_start_matches('#').to_lowercase();
                    tags.iter()
                        .any(|t| *t == want || t.starts_with(&format!("{want}/")))
                }))
            }
            "file.inFolder" => {
                arity(1..=1)?;
                let folder = strings()?.remove(0);
                let folder = folder.trim_matches('/');
                Value::Bool(folder.is_empty() || self.note.path.starts_with(&format!("{folder}/")))
            }
            "file.hasLink" => {
                arity(1..=1)?;
                let target = strings()?.remove(0);
                Value::Bool(self.note.links.iter().any(|l| names_link(l, &target)))
            }
            "file.hasProperty" => {
                arity(1..=1)?;
                Value::Bool(self.note.properties.contains_key(&strings()?.remove(0)))
            }
            _ => return Err(format!("unsupported function `{name}()`")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::expr::parse;
    use serde_json::json;

    fn note() -> NoteData {
        let now = chrono::Local::now();
        NoteData {
            path: "Projects/Alpha.md".into(),
            mtime_ms: now.timestamp_millis() - 3 * 86_400_000,
            size: 120,
            properties: json!({
                "status": "open", "rating": 3, "done": false, "due": "2026-09-20",
                "tags": ["a", "b"], "title": "Alpha note"
            })
            .as_object()
            .unwrap()
            .clone(),
            tags: vec!["project".into(), "project/sub".into()],
            links: vec!["Other/Beta.md".into()],
        }
    }

    fn ev(src: &str) -> Result<Value, String> {
        let n = note();
        let formulas: HashMap<String, Expr> = [("double", "rating * 2"), ("loop", "formula.loop")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), parse(v).unwrap()))
            .collect();
        let cx = Ctx {
            note: &n,
            formulas: &formulas,
            now: chrono::Local::now().naive_local(),
        };
        cx.eval(&parse(src).unwrap())
    }

    fn s(v: &str) -> Value {
        Value::Str(v.into())
    }

    #[test]
    fn file_fields_and_methods() {
        assert_eq!(ev("file.name"), Ok(s("Alpha.md")));
        assert_eq!(ev("file.basename"), Ok(s("Alpha")));
        assert_eq!(ev("file.folder"), Ok(s("Projects")));
        assert_eq!(ev("file.ext"), Ok(s("md")));
        assert_eq!(ev("file.size"), Ok(Value::Num(120.0)));
        assert_eq!(
            ev("file.tags"),
            Ok(Value::List(vec![s("#project"), s("#project/sub")]))
        );
        assert_eq!(ev(r#"file.hasTag("project")"#), Ok(Value::Bool(true)));
        assert_eq!(
            ev(r##"file.hasTag("#sub", "proj")"##),
            Ok(Value::Bool(false))
        );
        assert_eq!(ev(r#"file.inFolder("Projects")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.inFolder("Proj")"#), Ok(Value::Bool(false)));
        assert_eq!(ev(r#"file.hasLink("beta")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"file.hasProperty("due")"#), Ok(Value::Bool(true)));
        assert_eq!(
            ev(r#"file.mtime > now() - "1 week""#),
            Ok(Value::Bool(true))
        );
        assert_eq!(ev(r#"file.mtime > now() - "2d""#), Ok(Value::Bool(false)));
    }

    #[test]
    fn properties_comparisons_and_null() {
        assert_eq!(ev(r#"status == "open""#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"missing != "done""#), Ok(Value::Bool(true)));
        assert_eq!(ev("missing == null"), Ok(Value::Bool(true)));
        assert_eq!(ev("missing > 1"), Ok(Value::Bool(false)));
        assert_eq!(ev(r#"due < date("2026-10-01")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"note["rating"] >= 3 && !done"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"tags.contains("b")"#), Ok(Value::Bool(true)));
        assert_eq!(ev(r#"title.contains("lph")"#), Ok(Value::Bool(true)));
        assert_eq!(ev("missing.isEmpty()"), Ok(Value::Bool(true)));
        assert_eq!(ev("tags.isEmpty()"), Ok(Value::Bool(false)));
    }

    #[test]
    fn arithmetic_formulas_if_and_dates() {
        assert_eq!(ev("formula.double + 1"), Ok(Value::Num(7.0)));
        assert_eq!(ev("7 % 4"), Ok(Value::Num(3.0)));
        assert_eq!(ev(r#""a" + 1"#), Ok(s("a1")));
        assert_eq!(ev(r#"if(done, "yes", "no")"#), Ok(s("no")));
        assert_eq!(ev(r#"if(done, "yes")"#), Ok(Value::Null));
        let d = ev(r#"date("2024-12-01") + "1M" + "4h" + "3m""#).unwrap();
        assert_eq!(to_json(&d), json!("2025-01-01T04:03:00"));
        assert_eq!(
            to_json(&ev(r#"date("2024-12-01")"#).unwrap()),
            json!("2024-12-01")
        );
        assert!(ev("formula.loop").unwrap_err().contains("refers to itself"));
        assert!(ev("formula.nope").unwrap_err().contains("unknown formula"));
        assert!(ev(r#"date("2024-01-01") + "3 fortnights""#).is_err());
    }
}
