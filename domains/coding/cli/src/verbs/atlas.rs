//! Every anchor and every memory this repository holds, as one page on disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gmr::{AnchorView, MemoryView, Runtime, Sighting};
use gmr_atlas::{Edge, EdgeKind, Graph, Kind, Node, Tone};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;

pub const DEFAULT_OUT: &str = ".anchor/output/atlas.html";

fn anchor_id(key: &str) -> String {
    format!("anchor:{key}")
}

fn memory_id(external_id: &str) -> String {
    format!("memory:{external_id}")
}

fn label_of(key: &str) -> String {
    match key.split_once('#') {
        Some((_, name)) => name.to_owned(),
        None => key
            .rsplit_once('/')
            .map_or_else(|| key.to_owned(), |(_, base)| base.to_owned()),
    }
}

fn group_of(key: &str) -> String {
    let path = key.split_once('#').map_or(key, |(p, _)| p);
    if let Some((head, _)) = path.split_once("::") {
        return head.to_owned();
    }
    let mut parts = path.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("crates" | "batteries"), Some(name), _) => name.to_owned(),
        (Some("domains"), Some(domain), Some(part)) => format!("{domain}-{part}"),
        (Some(head), _, _) if !head.is_empty() => head.to_owned(),
        _ => "other".to_owned(),
    }
}

fn anchor_tone(view: &AnchorView, delivering: bool, unclaimed: bool) -> Tone {
    if view.closed {
        Tone::Muted
    } else if view.attempts > 0 || matches!(view.sighting, Sighting::Absent) {
        Tone::Alarm
    } else if delivering || unclaimed {
        Tone::Notice
    } else {
        Tone::Calm
    }
}

fn memory_tone(m: &MemoryView) -> (Tone, Option<&'static str>) {
    if m.unavailable.is_some() || m.content.is_none() {
        (Tone::Alarm, Some("unreadable"))
    } else if !m.grounded {
        (Tone::Alarm, Some("ungrounded"))
    } else if m.rewritten && m.retrievable == Some(false) {
        (Tone::Alarm, Some("bound version lost"))
    } else if m.rewritten {
        (Tone::Notice, Some("rewritten since binding"))
    } else {
        (Tone::Calm, None)
    }
}

fn anchor_node(view: &AnchorView, tone: Tone) -> Node {
    let key = view.key.to_string();
    let mut node = Node::new(anchor_id(&key), label_of(&key), Kind::Anchor, tone)
        .group(group_of(&key))
        .fact("probe", view.anchor.probe.name.to_string())
        .fact("sightings", view.sightings.to_string());
    if let Some(status) = view.status.as_ref() {
        node = node.fact("status", status.to_string());
        if tone != Tone::Calm {
            node = node.badge(status.to_string());
        }
    }
    if view.attempts > 0 {
        node = node.fact("failed attempts", view.attempts.to_string());
    }
    if let Some(at) = view.last_sighting {
        node = node.fact("last seen", at.format("%Y-%m-%d %H:%M").to_string());
    }
    if view.closed {
        node = node.fact("closed", "yes");
    }
    node
}

fn memory_node(m: &MemoryView, detail: Option<String>) -> Node {
    let external = m.reference.external_id.to_string();
    let (tone, badge) = memory_tone(m);
    let mut node = Node::new(memory_id(&external), external, Kind::Memory, tone)
        .fact("provider", m.reference.provider.to_string());
    if let Some(b) = badge {
        node = node.badge(b);
    }
    if let Some(html) = detail {
        node = node.detail(html);
    }
    if m.stale == Some(true) {
        node = node.fact("bound at", "before this anchor's latest entry");
    }
    if let Some(why) = &m.unavailable {
        node = node.fact("unavailable", why.clone());
    }
    node
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    out: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let catalog = Catalog::load(root)?;
    let (subs, _) = Subscriptions::load(root, &catalog)?;
    let views = rt.read_all().await?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut memories: BTreeMap<String, Node> = BTreeMap::new();
    let mut barren = 0usize;

    for view in &views {
        let shape = crate::shapes::of(&view.anchor.transitions);
        let bound: Vec<String> = view
            .memories
            .iter()
            .map(|m| m.reference.external_id.to_string())
            .collect();
        let delivering = bound
            .iter()
            .any(|note| subs.delivers(shape, note, &view.state, false));
        let moved = crate::delivery::axes_set(&view.state).is_some_and(|set| !set.is_empty());
        let unclaimed = bound.is_empty() && moved;
        if bound.is_empty() {
            barren += 1;
        }

        let key = view.key.to_string();
        nodes.push(anchor_node(view, anchor_tone(view, delivering, unclaimed)));

        for m in &view.memories {
            let external = m.reference.external_id.to_string();
            edges.push(Edge::new(
                memory_id(&external),
                anchor_id(&key),
                EdgeKind::Binding,
            ));
            memories.entry(external).or_insert_with(|| {
                let detail = m.content.as_deref().map(crate::prose::to_html);
                memory_node(m, detail)
            });
        }
    }

    let present: Vec<String> = memories.keys().cloned().collect();
    for external in &present {
        let Some(body) = views
            .iter()
            .flat_map(|v| &v.memories)
            .find(|m| m.reference.external_id.as_str() == external)
            .and_then(|m| m.content.as_deref())
        else {
            continue;
        };
        for target in crate::prose::wikilinks(body) {
            if target == *external || !memories.contains_key(&target) {
                continue;
            }
            edges.push(Edge::new(
                memory_id(external),
                memory_id(&target),
                EdgeKind::Reference,
            ));
        }
    }

    let memory_count = memories.len();
    nodes.extend(memories.into_values());

    let bindings = edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Binding))
        .count();
    let references = edges.len() - bindings;

    let repo = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    let graph = Graph {
        title: format!("{repo} atlas"),
        subtitle: format!("{} anchors · {memory_count} memories", views.len()),
        nodes,
        edges,
    };

    let html = gmr_atlas::render(&graph).map_err(|e| CliError(e.to_string()))?;
    let path = out.map_or_else(|| root.join(DEFAULT_OUT), PathBuf::from);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| CliError(format!("cannot create {}: {e}", dir.display())))?;
    }
    std::fs::write(&path, &html)
        .map_err(|e| CliError(format!("cannot write {}: {e}", path.display())))?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "written": path.display().to_string(),
                "anchors": views.len(),
                "memories": memory_count,
                "bindings": bindings,
                "references": references,
                "barren": barren,
            })
        );
    } else {
        println!("{}", path.display());
        println!(
            "{} anchors · {memory_count} memories · {bindings} bindings · {references} references",
            views.len()
        );
        if barren > 0 {
            println!("{barren} anchors have no memory bound to them");
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_is_the_package_the_coordinate_lives_in() {
        assert_eq!(
            group_of("crates/gmr-core/src/addr.rs#write_array"),
            "gmr-core"
        );
        assert_eq!(group_of("batteries/survey/src/cache.rs#scan"), "survey");
        assert_eq!(
            group_of("domains/coding/cli/src/verbs/sync.rs#run"),
            "coding-cli"
        );
        assert_eq!(group_of("tools/gate.py#check_acceptance_intact"), "tools");
    }

    #[test]
    fn a_key_that_is_not_a_path_groups_by_what_it_names_instead() {
        assert_eq!(group_of("layer::gmr-core"), "layer");
        assert_eq!(group_of("doctrine::decisions"), "doctrine");
    }

    fn view_memory(grounded: bool, rewritten: bool, stale: Option<bool>) -> MemoryView {
        MemoryView {
            reference: gmr::Ref::new("git", "memories/a.md"),
            bound_version: gmr::Version::new("v1"),
            current_version: None,
            rewritten,
            content: Some("body".to_owned()),
            content_at_bind: None,
            retrievable: None,
            grounded,
            unavailable: None,
            links: Vec::new(),
            bound_at_seq: None,
            stale,
        }
    }

    fn view_anchor(status: &str, closed: bool) -> AnchorView {
        AnchorView {
            key: gmr::AnchorKey::new("crates/x/src/a.rs#b"),
            anchor: gmr::Anchor {
                key: gmr::AnchorKey::new("crates/x/src/a.rs#b"),
                probe: gmr::ProbeRef::new(
                    gmr::Kind::new("inproc"),
                    gmr::ProbeName::new("ast-map"),
                    serde_json::json!({}),
                ),
                transitions: gmr::Transitions::default(),
                terminal: Default::default(),
                supersedes: None,
            },
            state: gmr::State::new(serde_json::json!({ "status": status })),
            status: Some(gmr::StatusId::new(status)),
            sighting: Sighting::Found,
            closed,
            attempts: 0,
            entered_at: None,
            last_sighting: None,
            sightings: 1,
            derivation: None,
            facts: None,
            memories: Vec::new(),
        }
    }

    #[test]
    fn a_status_that_asks_for_nothing_does_not_take_up_a_badge() {
        let calm = anchor_node(&view_anchor("settled", false), Tone::Calm);
        assert_eq!(calm.badge, None);
        assert!(
            calm.facts
                .iter()
                .any(|f| f.label == "status" && f.value == "settled"),
            "the word is still readable, it just does not shout"
        );
    }

    #[test]
    fn a_status_behind_any_other_tone_keeps_its_own_word_on_the_badge() {
        for tone in [Tone::Notice, Tone::Alarm, Tone::Muted] {
            let node = anchor_node(&view_anchor("расчёт", false), tone);
            assert_eq!(
                node.badge.as_deref(),
                Some("расчёт"),
                "the badge must echo whatever the domain called it"
            );
        }
    }

    #[test]
    fn a_memory_bound_before_the_latest_entry_is_not_an_alert_by_itself() {
        assert_eq!(
            memory_tone(&view_memory(true, false, Some(true))).0,
            Tone::Calm
        );
        assert_eq!(memory_tone(&view_memory(true, false, None)).0, Tone::Calm);
    }

    #[test]
    fn the_states_a_person_is_asked_to_act_on_are_the_ones_that_carry_a_tone() {
        assert_eq!(memory_tone(&view_memory(false, false, None)).0, Tone::Alarm);
        assert_eq!(memory_tone(&view_memory(true, true, None)).0, Tone::Notice);
    }

    #[test]
    fn the_label_is_the_definition_when_the_key_points_at_one() {
        assert_eq!(
            label_of("crates/gmr-core/src/addr.rs#write_array"),
            "write_array"
        );
        assert_eq!(label_of("crates/gmr-core/src/addr.rs"), "addr.rs");
        assert_eq!(label_of("doctrine::decisions"), "doctrine::decisions");
    }
}
