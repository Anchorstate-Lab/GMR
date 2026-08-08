use std::collections::BTreeSet;

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

    pub fn reads_obs(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_obs(&mut out);
        out
    }

    fn collect_obs(&self, out: &mut BTreeSet<String>) {
        match self {
            Self::Path(p) if p.root == Root::Obs => {
                let fields: Vec<&str> = p
                    .steps
                    .iter()
                    .map_while(|s| match s {
                        Step::Field(name) => Some(name.as_str()),
                        Step::Index(_) => None,
                    })
                    .collect();
                if !fields.is_empty() {
                    out.insert(fields.join("."));
                }
            }
            Self::Changed(name) => {
                out.insert(name.clone());
            }
            Self::Path(_) | Self::Lit(_) => {}
            Self::Exists(x) | Self::Not(x) | Self::Neg(x) => x.collect_obs(out),
            Self::Binary { lhs, rhs, .. } => {
                lhs.collect_obs(out);
                rhs.collect_obs(out);
            }
            Self::Object(fields) => fields.iter().for_each(|(_, v)| v.collect_obs(out)),
            Self::Array(items) => items.iter().for_each(|v| v.collect_obs(out)),
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

    fn obs_of(src: &str) -> Vec<String> {
        parse(src).unwrap().reads_obs().into_iter().collect()
    }

    #[test]
    fn every_obs_read_is_visible_at_any_depth() {
        assert_eq!(obs_of("obs.candidates"), ["candidates"]);
        assert_eq!(obs_of("obs.at.scope"), ["at.scope"]);
        assert_eq!(
            obs_of("{ n: obs.facts.occurrences }"),
            ["facts.occurrences"]
        );
        assert_eq!(obs_of("{ xs: [1, obs.a] }"), ["a"]);
        assert_eq!(obs_of("not exists(obs.b)"), ["b"]);
        assert_eq!(obs_of("obs.x == obs.y"), ["x", "y"]);
    }

    #[test]
    fn changed_is_a_read_of_obs() {
        assert_eq!(obs_of(r#"changed("matches")"#), ["matches"]);
    }

    #[test]
    fn a_string_that_looks_like_a_path_is_not_a_read() {
        assert!(obs_of(r#"{ note: "obs.x" }"#).is_empty());
    }

    #[test]
    fn an_index_ends_the_path() {
        assert_eq!(obs_of("obs.matches[0]"), ["matches"]);
        assert_eq!(obs_of("obs.facts.files[0].name"), ["facts.files"]);
    }

    #[test]
    fn state_and_time_are_not_obs_reads() {
        assert!(obs_of("state.n + taken_at - entered_at").is_empty());
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
        assert!(
            reads("state"),
            "matching the text `state.` would miss this one"
        );
    }
}
