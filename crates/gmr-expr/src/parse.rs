use serde_json::Value;

use crate::ast::{BinOp, Node, Path, Root, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
}

impl SyntaxError {
    pub fn class(&self) -> &'static str {
        "bad_expression"
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SyntaxError {}

fn err<T>(message: impl Into<String>) -> Result<T, SyntaxError> {
    Err(SyntaxError {
        message: message.into(),
    })
}

pub fn parse(source: &str) -> Result<Node, SyntaxError> {
    let mut p = Parser {
        chars: source.chars().collect(),
        at: 0,
    };
    p.skip_space();
    if p.done() {
        return err("表达式为空");
    }
    let node = p.expr(0)?;
    p.skip_space();
    if !p.done() {
        return err(format!("多余的 `{}`", p.rest()));
    }
    Ok(node)
}

pub fn parse_path(source: &str) -> Result<Path, SyntaxError> {
    match parse(source)? {
        Node::Path(p) => Ok(p),
        _ => err("这里只能是一条字段路径"),
    }
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

fn level(n: u8) -> &'static [(&'static str, BinOp)] {
    match n {
        0 => &[("or", BinOp::Or)],
        1 => &[("and", BinOp::And)],
        2 => &[
            ("==", BinOp::Eq),
            ("!=", BinOp::Ne),
            ("<=", BinOp::Le),
            (">=", BinOp::Ge),
            ("<", BinOp::Lt),
            (">", BinOp::Gt),
        ],
        3 => &[("+", BinOp::Add), ("-", BinOp::Sub)],
        _ => &[("*", BinOp::Mul), ("/", BinOp::Div)],
    }
}

const LEVELS: u8 = 5;

impl Parser {
    fn done(&self) -> bool {
        self.at >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn rest(&self) -> String {
        self.chars[self.at..].iter().collect()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn eat(&mut self, token: &str) -> bool {
        let n = token.chars().count();
        if self.chars.len() < self.at + n {
            return false;
        }
        if !self.chars[self.at..self.at + n]
            .iter()
            .eq(token.chars().collect::<Vec<_>>().iter())
        {
            return false;
        }
        if token.chars().all(|c| c.is_alphabetic())
            && matches!(self.chars.get(self.at + n), Some(c) if is_ident(*c))
        {
            return false;
        }
        self.at += n;
        true
    }

    fn expr(&mut self, lv: u8) -> Result<Node, SyntaxError> {
        if lv >= LEVELS {
            return self.unary();
        }
        let mut lhs = self.expr(lv + 1)?;
        loop {
            self.skip_space();
            let Some((_, op)) = level(lv).iter().find(|(t, _)| self.eat(t)).copied() else {
                return Ok(lhs);
            };
            self.skip_space();
            let rhs = self.expr(lv + 1)?;
            lhs = Node::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
    }

    fn unary(&mut self) -> Result<Node, SyntaxError> {
        self.skip_space();
        if self.eat("not") {
            self.skip_space();
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        if self.eat("-") {
            self.skip_space();
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Node, SyntaxError> {
        self.skip_space();
        match self.peek() {
            None => err("表达式在这里断了"),
            Some('(') => {
                self.at += 1;
                let inner = self.expr(0)?;
                self.skip_space();
                if !self.eat(")") {
                    return err("括号没闭合");
                }
                Ok(inner)
            }
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Node::Lit(Value::String(self.string()?))),
            Some(c) if c.is_ascii_digit() => self.number(),
            Some(c) if is_ident(c) => self.word(),
            Some(c) => err(format!("这里不该出现 `{c}`")),
        }
    }

    fn object(&mut self) -> Result<Node, SyntaxError> {
        self.at += 1;
        let mut fields: Vec<(String, Node)> = Vec::new();
        loop {
            self.skip_space();
            if self.eat("}") {
                return Ok(Node::Object(fields));
            }
            if !fields.is_empty() {
                if !self.eat(",") {
                    return err("对象的字段之间要用 `,` 隔开");
                }
                self.skip_space();
                if self.eat("}") {
                    return Ok(Node::Object(fields));
                }
            }

            let name = match self.peek() {
                Some('"') => self.string()?,
                Some(c) if is_ident(c) => {
                    let start = self.at;
                    while matches!(self.peek(), Some(c) if is_ident(c)) {
                        self.at += 1;
                    }
                    self.chars[start..self.at].iter().collect()
                }
                _ => return err("对象的字段名要么是标识符，要么是带引号的字符串"),
            };
            if fields.iter().any(|(k, _)| *k == name) {
                return err(format!("字段 `{name}` 写了两遍"));
            }

            self.skip_space();
            if !self.eat(":") {
                return err(format!("字段 `{name}` 后面要跟 `:`"));
            }
            fields.push((name, self.expr(0)?));
        }
    }

    fn array(&mut self) -> Result<Node, SyntaxError> {
        self.at += 1;
        let mut items: Vec<Node> = Vec::new();
        loop {
            self.skip_space();
            if self.eat("]") {
                return Ok(Node::Array(items));
            }
            if !items.is_empty() {
                if !self.eat(",") {
                    return err("数组的元素之间要用 `,` 隔开");
                }
                self.skip_space();
                if self.eat("]") {
                    return Ok(Node::Array(items));
                }
            }
            if self.peek().is_none() {
                return err("数组没有闭合的 `]`");
            }
            items.push(self.expr(0)?);
        }
    }

    fn string(&mut self) -> Result<String, SyntaxError> {
        self.at += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return err("字符串没有闭合的引号"),
                Some('"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(c) => {
                    out.push(c);
                    self.at += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Result<Node, SyntaxError> {
        let start = self.at;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.') {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();

        let unit = match self.peek() {
            Some(u @ ('d' | 'h' | 'm' | 's')) if !matches!(self.chars.get(self.at + 1), Some(c) if is_ident(*c)) =>
            {
                self.at += 1;
                Some(u)
            }
            _ => None,
        };

        let n: f64 = text.parse().map_err(|_| SyntaxError {
            message: format!("`{text}` 不是一个数"),
        })?;

        let value = match unit {
            None => n,
            Some('d') => n * 86_400.0,
            Some('h') => n * 3_600.0,
            Some('m') => n * 60.0,
            _ => n,
        };
        Ok(Node::Lit(json_number(value)))
    }

    fn word(&mut self) -> Result<Node, SyntaxError> {
        let start = self.at;
        while matches!(self.peek(), Some(c) if is_ident(c)) {
            self.at += 1;
        }
        let word: String = self.chars[start..self.at].iter().collect();

        match word.as_str() {
            "true" => return Ok(Node::Lit(Value::Bool(true))),
            "false" => return Ok(Node::Lit(Value::Bool(false))),
            "null" => return Ok(Node::Lit(Value::Null)),
            "changed" => {
                let name = self.call_arg_string("changed")?;
                return Ok(Node::Changed(name));
            }
            "exists" => {
                self.skip_space();
                if !self.eat("(") {
                    return err("exists 后面要跟 `(`");
                }
                let inner = self.expr(0)?;
                self.skip_space();
                if !self.eat(")") {
                    return err("exists 的括号没闭合");
                }
                return Ok(Node::Exists(Box::new(inner)));
            }
            _ => {}
        }

        let root = match word.as_str() {
            "obs" => Root::Obs,
            "state" => Root::State,
            "taken_at" => Root::TakenAt,
            "entered_at" => Root::EnteredAt,
            other => {
                return err(format!(
                    "未知的命名槽 `{other}`；只有 obs · state · taken_at · entered_at"
                ));
            }
        };
        Ok(Node::Path(Path {
            root,
            steps: self.steps()?,
        }))
    }

    fn call_arg_string(&mut self, name: &str) -> Result<String, SyntaxError> {
        self.skip_space();
        if !self.eat("(") {
            return err(format!("{name} 后面要跟 `(`"));
        }
        self.skip_space();
        if self.peek() != Some('"') {
            return err(format!("{name} 的参数是一个带引号的切面名"));
        }
        let arg = self.string()?;
        self.skip_space();
        if !self.eat(")") {
            return err(format!("{name} 的括号没闭合"));
        }
        Ok(arg)
    }

    fn steps(&mut self) -> Result<Vec<Step>, SyntaxError> {
        let mut steps = Vec::new();
        loop {
            match self.peek() {
                Some('.') => {
                    self.at += 1;
                    let start = self.at;
                    while matches!(self.peek(), Some(c) if is_ident(c)) {
                        self.at += 1;
                    }
                    if start == self.at {
                        return err("`.` 后面缺字段名");
                    }
                    steps.push(Step::Field(self.chars[start..self.at].iter().collect()));
                }
                Some('[') => {
                    self.at += 1;
                    let start = self.at;
                    while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                        self.at += 1;
                    }
                    let digits: String = self.chars[start..self.at].iter().collect();
                    if !self.eat("]") {
                        return err("下标没有闭合的 `]`");
                    }
                    let i = digits.parse::<usize>().map_err(|_| SyntaxError {
                        message: "下标不是一个非负整数".into(),
                    })?;
                    steps.push(Step::Index(i));
                }
                _ => return Ok(steps),
            }
        }
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

pub(crate) fn json_number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9e15 {
        Value::from(v as i64)
    } else {
        serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(s: &str) -> String {
        parse(s).unwrap().render()
    }

    #[test]
    fn bare_root_is_the_whole_slot() {
        assert_eq!(
            parse("obs").unwrap(),
            Node::Path(Path {
                root: Root::Obs,
                steps: vec![]
            })
        );
    }

    #[test]
    fn fields_and_indices() {
        let Node::Path(p) = parse("obs.params[0].type").unwrap() else {
            panic!()
        };
        assert_eq!(
            p.steps,
            vec![
                Step::Field("params".into()),
                Step::Index(0),
                Step::Field("type".into())
            ]
        );
    }

    #[test]
    fn there_are_exactly_four_slots() {
        for s in ["obs", "state.position", "taken_at", "entered_at"] {
            assert!(parse(s).is_ok(), "{s}");
        }
        for s in ["located_at", "prev.close", "streak", "delta"] {
            assert!(parse(s).is_err(), "{s} 不该还是一个槽");
        }
    }

    #[test]
    fn arrays_parse_and_render() {
        assert_eq!(parse("[]").unwrap().render(), "[]");
        assert_eq!(parse("[1, 2]").unwrap().render(), "[1, 2]");
        assert_eq!(
            parse("{ names: [], n: 0 }").unwrap().render(),
            "{ names: [], n: 0 }"
        );
        assert_eq!(
            parse("[obs.a, state.b + 1]").unwrap().render(),
            "[obs.a, (state.b + 1)]"
        );
        assert_eq!(parse("[[1], [2]]").unwrap().render(), "[[1], [2]]");
    }

    #[test]
    fn a_subscript_is_not_an_array_literal() {
        assert_eq!(parse("obs.a[0]").unwrap().render(), "obs.a[0]");
        assert_eq!(parse("[obs.a[0]]").unwrap().render(), "[obs.a[0]]");
    }

    #[test]
    fn a_malformed_array_is_rejected() {
        for s in ["[", "[1", "[1 2]", "[1,,2]"] {
            assert!(parse(s).is_err(), "`{s}` 该被拒绝");
        }
    }

    #[test]
    fn object_construction_parses_and_renders() {
        assert_eq!(
            render("{ position: obs.at, status: \"ok\" }"),
            "{ position: obs.at, status: \"ok\" }"
        );
        assert_eq!(render("{}"), "{  }");
        assert_eq!(render("{ a: 1, }"), "{ a: 1 }", "允许尾逗号");
    }

    #[test]
    fn object_fields_may_be_quoted_and_must_not_repeat() {
        assert!(parse("{ \"my-field\": 1 }").is_ok());
        assert!(
            parse("{ a: 1, a: 2 }").is_err(),
            "同一个字段写两遍是笔误，不是覆盖"
        );
    }

    #[test]
    fn objects_nest_and_carry_expressions() {
        let src = "{ at: state.at, n: state.n + 1, deep: { x: obs.x } }";
        assert!(parse(src).is_ok(), "{src}");
    }

    #[test]
    fn precedence_binds_tighter_downward() {
        assert_eq!(render("1 + 2 * 3"), "(1 + (2 * 3))");
        assert_eq!(render("1 * 2 + 3"), "((1 * 2) + 3)");
    }

    #[test]
    fn comparison_binds_looser_than_arithmetic() {
        assert_eq!(
            render("taken_at - entered_at > 30d"),
            "((taken_at - entered_at) > 2592000)"
        );
    }

    #[test]
    fn logic_binds_loosest() {
        assert_eq!(
            render("obs.a > 1 and obs.b < 2 or obs.c == 3"),
            "(((obs.a > 1) and (obs.b < 2)) or (obs.c == 3))"
        );
    }

    #[test]
    fn parens_override() {
        assert_eq!(render("(1 + 2) * 3"), "((1 + 2) * 3)");
    }

    #[test]
    fn durations_are_seconds() {
        assert_eq!(render("30d"), "2592000");
        assert_eq!(render("2h"), "7200");
        assert_eq!(render("15m"), "900");
        assert_eq!(render("10s"), "10");
    }

    #[test]
    fn changed_and_exists() {
        assert_eq!(render("changed(\"shape\")"), "changed(\"shape\")");
        assert_eq!(render("exists(obs.a)"), "exists(obs.a)");
    }

    #[test]
    fn a_word_starting_with_and_is_not_the_operator() {
        assert!(parse("obs.android").is_ok());
    }

    #[test]
    fn malformed_expressions_are_rejected() {
        for s in [
            "",
            "obs.",
            "obs[",
            "obs[a]",
            "obs.a b",
            "1 +",
            "(1",
            "changed(x)",
        ] {
            assert!(parse(s).is_err(), "{s:?} 该被拒绝");
        }
    }

    #[test]
    fn parse_path_refuses_a_predicate() {
        assert!(parse_path("obs.a").is_ok());
        assert!(parse_path("obs.a > 1").is_err());
    }
}
