//! Obsidian's bases expression syntax, the subset `docs/bases.md` lists.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    /// `file.name`, `note.status` or a bare `status`, `formula.x`.
    Field(Scope, String),
    /// A global like `date`, or a file method as `file.hasTag`.
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    Unary(char, Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    File,
    Note,
    Formula,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

const GLOBALS: &[&str] = &["date", "now", "today", "if"];
const FILE_FIELDS: &[&str] = &[
    "name", "basename", "path", "folder", "ext", "size", "mtime", "tags", "links",
];
const FILE_METHODS: &[&str] = &["hasTag", "inFolder", "hasLink", "hasProperty"];
const METHODS: &[&str] = &["contains", "isEmpty"];
// Two-character operators first, so `<=` is not read as `<`.
const PUNCT: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "<", ">", "!", "+", "-", "*", "/", "%", "(", ")", ",", ".",
    "[", "]",
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    P(&'static str),
}

fn syntax(src: &str, what: &str) -> String {
    format!("{what} in `{src}`")
}

fn unsupported(src: &str, what: &str) -> String {
    format!("unsupported {what} in `{src}`")
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Tok::Num(
                text.parse().map_err(|_| syntax(src, "bad number"))?,
            ));
        } else if c == '"' || c == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(syntax(src, "unclosed string")),
                    Some(&q) if q == c => break,
                    Some('\\') => {
                        s.extend(chars.get(i + 1));
                        i += 2;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            i += 1;
            out.push(Tok::Str(s));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let p = PUNCT
                .iter()
                .find(|p| rest.starts_with(**p))
                .ok_or_else(|| syntax(src, &format!("unexpected `{c}`")))?;
            out.push(Tok::P(p));
            i += p.len();
        }
    }
    Ok(out)
}

pub fn parse(src: &str) -> Result<Expr, String> {
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
        src,
    };
    let e = p.or()?;
    if p.pos < p.toks.len() {
        return Err(syntax(src, "unexpected input after the expression"));
    }
    Ok(e)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    src: &'a str,
}

type Level<'a> = fn(&mut Parser<'a>) -> Result<Expr, String>;

impl<'a> Parser<'a> {
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn eat(&mut self, p: &str) -> bool {
        let hit = matches!(self.toks.get(self.pos), Some(Tok::P(q)) if *q == p);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn expect(&mut self, p: &str) -> Result<(), String> {
        if self.eat(p) {
            Ok(())
        } else {
            Err(syntax(self.src, &format!("expected `{p}`")))
        }
    }

    fn binary(&mut self, ops: &[(&str, Op)], next: Level<'a>) -> Result<Expr, String> {
        let mut left = next(self)?;
        'more: loop {
            for (s, op) in ops {
                if self.eat(s) {
                    left = Expr::Binary(Box::new(left), *op, Box::new(next(self)?));
                    continue 'more;
                }
            }
            return Ok(left);
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        self.binary(&[("||", Op::Or)], Self::and)
    }

    fn and(&mut self) -> Result<Expr, String> {
        self.binary(&[("&&", Op::And)], Self::equality)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        self.binary(&[("==", Op::Eq), ("!=", Op::Ne)], Self::relational)
    }

    fn relational(&mut self) -> Result<Expr, String> {
        let ops = [("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)];
        self.binary(&ops, Self::additive)
    }

    fn additive(&mut self) -> Result<Expr, String> {
        self.binary(&[("+", Op::Add), ("-", Op::Sub)], Self::multiplicative)
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        let ops = [("*", Op::Mul), ("/", Op::Div), ("%", Op::Mod)];
        self.binary(&ops, Self::unary)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        for op in ['!', '-'] {
            if self.eat(&op.to_string()) {
                return Ok(Expr::Unary(op, Box::new(self.unary()?)));
            }
        }
        self.postfix()
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        if self.eat(")") {
            return Ok(out);
        }
        loop {
            out.push(self.or()?);
            if self.eat(")") {
                return Ok(out);
            }
            self.expect(",")?;
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Tok::Ident(s)) => Ok(s),
            _ => Err(syntax(self.src, "expected a name")),
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat("[") {
                return Err(unsupported(self.src, "index `[...]`"));
            }
            if !self.eat(".") {
                return Ok(e);
            }
            let m = self.name()?;
            if !self.eat("(") {
                return Err(unsupported(self.src, &format!("field `.{m}`")));
            }
            if !METHODS.contains(&m.as_str()) {
                return Err(unsupported(self.src, &format!("method `.{m}()`")));
            }
            e = Expr::Method(Box::new(e), m, self.args()?);
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Num(n)),
            Some(Tok::Str(s)) => Ok(Expr::Str(s)),
            Some(Tok::P("(")) => {
                let e = self.or()?;
                self.expect(")")?;
                Ok(e)
            }
            Some(Tok::Ident(id)) => self.ident(id),
            Some(Tok::P(p)) => Err(syntax(self.src, &format!("unexpected `{p}`"))),
            None => Err(syntax(self.src, "unexpected end")),
        }
    }

    fn ident(&mut self, id: String) -> Result<Expr, String> {
        match id.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "null" => return Ok(Expr::Null),
            "this" => return Err(unsupported(self.src, "`this`")),
            _ => {}
        }
        if self.eat("(") {
            if !GLOBALS.contains(&id.as_str()) {
                return Err(unsupported(self.src, &format!("function `{id}()`")));
            }
            return Ok(Expr::Call(id, self.args()?));
        }
        let scope = match id.as_str() {
            "file" => Scope::File,
            "note" => Scope::Note,
            "formula" => Scope::Formula,
            _ => return Ok(Expr::Field(Scope::Note, id)),
        };
        let key = if self.eat("[") {
            let Some(Tok::Str(k)) = self.next() else {
                return Err(syntax(self.src, "expected a quoted name"));
            };
            self.expect("]")?;
            k
        } else {
            self.expect(".")?;
            self.name()?
        };
        if scope != Scope::File {
            return Ok(Expr::Field(scope, key));
        }
        if self.eat("(") {
            if !FILE_METHODS.contains(&key.as_str()) {
                return Err(unsupported(self.src, &format!("method `file.{key}()`")));
            }
            return Ok(Expr::Call(format!("file.{key}"), self.args()?));
        }
        if !FILE_FIELDS.contains(&key.as_str()) {
            return Err(unsupported(self.src, &format!("field `file.{key}`")));
        }
        Ok(Expr::Field(Scope::File, key))
    }
}

#[cfg(test)]
mod tests {
    use super::Expr::*;
    use super::*;

    fn b(l: Expr, op: Op, r: Expr) -> Expr {
        Binary(Box::new(l), op, Box::new(r))
    }

    #[test]
    fn precedence_and_fields() {
        assert_eq!(
            parse(r#"status != "done" && file.size > 1 + 2 * 3"#).unwrap(),
            b(
                b(
                    Field(Scope::Note, "status".into()),
                    Op::Ne,
                    Str("done".into())
                ),
                Op::And,
                b(
                    Field(Scope::File, "size".into()),
                    Op::Gt,
                    b(Num(1.0), Op::Add, b(Num(2.0), Op::Mul, Num(3.0)))
                )
            )
        );
        assert_eq!(
            parse("a || b && !c").unwrap(),
            b(
                Field(Scope::Note, "a".into()),
                Op::Or,
                b(
                    Field(Scope::Note, "b".into()),
                    Op::And,
                    Unary('!', Box::new(Field(Scope::Note, "c".into())))
                )
            )
        );
    }

    #[test]
    fn calls_methods_brackets_and_literals() {
        assert_eq!(
            parse(r#"file.hasTag("a", 'b')"#).unwrap(),
            Call("file.hasTag".into(), vec![Str("a".into()), Str("b".into())])
        );
        assert_eq!(
            parse(r#"note["due date"].isEmpty()"#).unwrap(),
            Method(
                Box::new(Field(Scope::Note, "due date".into())),
                "isEmpty".into(),
                vec![]
            )
        );
        assert_eq!(
            parse("formula.x == null").unwrap(),
            b(Field(Scope::Formula, "x".into()), Op::Eq, Null)
        );
        assert_eq!(parse("-2.5").unwrap(), Unary('-', Box::new(Num(2.5))));
        assert_eq!(
            parse("if(true, now(), 1)").unwrap(),
            Call(
                "if".into(),
                vec![Bool(true), Call("now".into(), vec![]), Num(1.0)]
            )
        );
        assert_eq!(parse(r#""a\"b""#).unwrap(), Str("a\"b".into()));
    }

    #[test]
    fn unsupported_is_named_and_bad_syntax_fails() {
        for (src, what) in [
            (r#"link("a")"#, "function `link()`"),
            ("name.lower()", "method `.lower()`"),
            ("name.length", "field `.length`"),
            ("file.ctime > now()", "field `file.ctime`"),
            ("file.asLink()", "method `file.asLink()`"),
            ("this.file", "`this`"),
            ("tags[0]", "index"),
        ] {
            let e = parse(src).unwrap_err();
            assert!(e.contains(what), "{src}: {e}");
            assert!(e.starts_with("unsupported"), "{src}: {e}");
        }
        for src in ["a ==", "\"open", "(a", "a b", "#"] {
            assert!(parse(src).is_err(), "{src}");
        }
    }
}
