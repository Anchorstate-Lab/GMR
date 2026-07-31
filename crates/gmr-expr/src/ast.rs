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
