//! The vocabulary a caller hands over, and what this layer refuses to accept.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Muted,
    Calm,
    Notice,
    Alarm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Anchor,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Binding,
    Reference,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

impl Fact {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub tone: Tone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub under: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Fact>,
}

impl Node {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: Kind, tone: Tone) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            tone,
            badge: None,
            under: Vec::new(),
            detail: None,
            facts: Vec::new(),
        }
    }

    #[must_use]
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    #[must_use]
    pub fn under<I, S>(mut self, trail: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.under = trail.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn detail(mut self, html: impl Into<String>) -> Self {
        self.detail = Some(html.into());
        self
    }

    #[must_use]
    pub fn fact(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.facts.push(Fact::new(label, value));
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

impl Edge {
    pub fn new(source: impl Into<String>, target: impl Into<String>, kind: EdgeKind) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Graph {
    pub title: String,
    pub subtitle: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AtlasError {
    #[error("two nodes share the id `{0}`")]
    DuplicateNode(String),
    #[error("edge `{from}` -> `{to}` names `{missing}`, which is not a node in this graph")]
    DanglingEdge {
        from: String,
        to: String,
        missing: String,
    },
}

impl Graph {
    pub fn check(&self) -> Result<(), AtlasError> {
        let mut ids = std::collections::BTreeSet::new();
        for node in &self.nodes {
            if !ids.insert(node.id.as_str()) {
                return Err(AtlasError::DuplicateNode(node.id.clone()));
            }
        }
        for edge in &self.edges {
            for end in [&edge.source, &edge.target] {
                if !ids.contains(end.as_str()) {
                    return Err(AtlasError::DanglingEdge {
                        from: edge.source.clone(),
                        to: edge.target.clone(),
                        missing: end.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> Node {
        Node::new(id, id, Kind::Anchor, Tone::Calm)
    }

    #[test]
    fn a_graph_whose_every_edge_lands_on_a_node_is_accepted() {
        let g = Graph {
            nodes: vec![node("a"), node("b")],
            edges: vec![Edge::new("a", "b", EdgeKind::Binding)],
            ..Graph::default()
        };
        assert_eq!(g.check(), Ok(()));
    }

    #[test]
    fn an_edge_reaching_a_node_that_is_not_here_is_refused_before_the_page_is_written() {
        let g = Graph {
            nodes: vec![node("a")],
            edges: vec![Edge::new("a", "gone", EdgeKind::Binding)],
            ..Graph::default()
        };
        assert_eq!(
            g.check(),
            Err(AtlasError::DanglingEdge {
                from: "a".to_owned(),
                to: "gone".to_owned(),
                missing: "gone".to_owned(),
            })
        );
    }

    #[test]
    fn two_nodes_sharing_an_id_are_refused_rather_than_silently_collapsed() {
        let g = Graph {
            nodes: vec![node("a"), node("a")],
            ..Graph::default()
        };
        assert_eq!(g.check(), Err(AtlasError::DuplicateNode("a".to_owned())));
    }

    #[test]
    fn tone_orders_by_how_loudly_it_asks_to_be_looked_at() {
        let mut tones = vec![Tone::Alarm, Tone::Muted, Tone::Notice, Tone::Calm];
        tones.sort();
        assert_eq!(
            tones,
            vec![Tone::Muted, Tone::Calm, Tone::Notice, Tone::Alarm]
        );
    }
}
