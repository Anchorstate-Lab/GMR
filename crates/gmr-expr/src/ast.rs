use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Root {
    Obs,
    State,
    TakenAt,
    EnteredAt,
}

impl Root {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Obs => "obs",
            Self::State => "state",
            Self::TakenAt => "taken_at",
            Self::EnteredAt => "entered_at",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Field(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub root: Root,
    pub steps: Vec<Step>,
}

impl Path {
    pub fn render(&self) -> String {
        let mut out = self.root.as_str().to_owned();
        for step in &self.steps {
            match step {
                Step::Field(name) => {
                    out.push('.');
                    out.push_str(name);
                }
                Step::Index(i) => {
                    out.push('[');
                    out.push_str(&i.to_string());
                    out.push(']');
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "and",
            Self::Or => "or",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Node {
    Path(Path),
    Lit(Value),
    Changed(String),
    Exists(Box<Node>),
    Not(Box<Node>),
    Neg(Box<Node>),
    Binary {
        op: BinOp,
        lhs: Box<Node>,
        rhs: Box<Node>,
    },
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
}

impl Node {
    /// Whether this expression reads the previous state.
    ///
    /// Walks the AST rather than matching rendered text: `{ note: "state.x" }`
    /// is a literal, not a read, and text cannot tell the two apart.
    pub fn reads_state(&self) -> bool {
        match self {
            Self::Path(p) => p.root == Root::State,
            Self::Lit(_) | Self::Changed(_) => false,
            Self::Exists(x) | Self::Not(x) | Self::Neg(x) => x.reads_state(),
            Self::Binary { lhs, rhs, .. } => lhs.reads_state() || rhs.reads_state(),
            Self::Object(fields) => fields.iter().any(|(_, v)| v.reads_state()),
            Self::Array(items) => items.iter().any(Node::reads_state),
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Path(p) => p.render(),
            Self::Lit(v) => v.to_string(),
            Self::Changed(n) => format!("changed(\"{n}\")"),
            Self::Exists(x) => format!("exists({})", x.render()),
            Self::Not(x) => format!("not ({})", x.render()),
            Self::Neg(x) => format!("-({})", x.render()),
            Self::Binary { op, lhs, rhs } => {
                format!("({} {} {})", lhs.render(), op.as_str(), rhs.render())
            }
            Self::Object(fields) => {
                let body: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.render()))
                    .collect();
                format!("{{ {} }}", body.join(", "))
            }
            Self::Array(items) => {
                let body: Vec<String> = items.iter().map(Node::render).collect();
                format!("[{}]", body.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parse::parse;

    fn reads(src: &str) -> bool {
        parse(src).unwrap().reads_state()
    }

    #[test]
    fn reading_the_previous_state_is_visible_at_any_depth() {
        assert!(reads("state.n"));
        assert!(reads("{ n: state.n + 1 }"));
        assert!(reads("{ xs: [1, state.n] }"));
        assert!(reads("not exists(state.n)"));
    }

    #[test]
    fn a_literal_that_merely_looks_like_a_path_is_not_a_read() {
        assert!(!reads(r#"{ note: "state.n" }"#));
        assert!(!reads("{ n: obs.n }"));
        assert!(!reads(r#"changed("state")"#));
    }

    #[test]
    fn the_bare_state_root_counts_too() {
        assert!(reads("state"), "matching the text `state.` would miss this one");
    }
}
