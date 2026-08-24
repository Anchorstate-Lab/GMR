use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gmr::{AnchorView, Before, Grounding, MemoryView, Runtime, Sighting};
use gmr_atlas::{Edge, EdgeKind, Graph, Kind, Node, Tone};

use crate::delivery::Subscriptions;
use crate::error::CliError;
use crate::probes::Catalog;

pub const DEFAULT_OUT: &str = ".anchor/output/atlas.html";

const LOGO: &[u8] = include_bytes!("../../assets/logo.png");

fn logo() -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(LOGO)
    )
}

fn anchor_id(key: &str) -> String {
    format!("anchor:{key}")
}

fn memory_id(reference: &gmr::Ref) -> String {
    format!("memory:{}:{}", reference.provider, reference.external_id)
}

fn label_of(key: &str) -> String {
    if let Some((_, name)) = key.split_once('#') {
        return name.to_owned();
    }
    if let Some((_, rest)) = key.split_once("::") {
        return rest.to_owned();
    }
    key.rsplit_once('/')
        .map_or_else(|| key.to_owned(), |(_, base)| base.to_owned())
}

fn trail_of(key: &str) -> Vec<String> {
    let (path, names_a_definition) = key
        .split_once('#')
        .map_or((key, false), |(path, _)| (path, true));
    if let Some((head, _)) = path.split_once("::") {
        return vec![head.to_owned()];
    }
    let mut trail: Vec<String> = path.split('/').map(str::to_owned).collect();
    if !names_a_definition {
        trail.pop();
    }
    trail
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
    match &m.grounding {
        Grounding::Gone => (Tone::Alarm, Some("gone")),
        Grounding::NoProvider { .. } => (Tone::Alarm, Some("no provider")),
        Grounding::Unreachable { .. } => (Tone::Alarm, Some("unreachable")),
        _ if !m.grounded => (Tone::Alarm, Some("ungrounded")),
        Grounding::Rewritten { before, .. } => match before {
            Before::Retrieved { .. } => (Tone::Notice, Some("rewritten since binding")),
            _ => (Tone::Alarm, Some("bound version lost")),
        },
        Grounding::Unverified { .. } => (Tone::Notice, Some("never verified")),
        Grounding::Current { .. } => (Tone::Calm, None),
    }
}

fn anchor_node(view: &AnchorView, tone: Tone) -> Node {
    let key = view.key.to_string();
    let mut node = Node::new(anchor_id(&key), label_of(&key), Kind::Anchor, tone)
        .under(trail_of(&key))
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

fn memory_node(m: &MemoryView, names: &crate::memories::Names, detail: Option<String>) -> Node {
    let label = names.of(&m.reference);
    let (tone, badge) = memory_tone(m);
    let mut node = Node::new(memory_id(&m.reference), label, Kind::Memory, tone)
        .fact("provider", m.reference.provider.to_string())
        .fact("address", m.reference.external_id.to_string());
    if let Some(b) = badge {
        node = node.badge(b);
    }
    if let Some(html) = detail {
        node = node.detail(html);
    }
    if m.stale == Some(true) {
        node = node.fact("bound at", "before this anchor's latest entry");
    }
    match &m.grounding {
        Grounding::Gone => node = node.fact("gone", "the provider says this record is gone"),
        Grounding::NoProvider { provider } => {
            node = node.fact(
                "no provider",
                format!("`{provider}` is not registered here"),
            );
        }
        Grounding::Unreachable { why, .. } => node = node.fact("unreachable", why.clone()),
        _ => {}
    }
    node
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &crate::memories::Names,
    out: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let catalog = Catalog::load(root)?;
    let (subs, _) = Subscriptions::load(root, &catalog, names)?;
    let views = rt.grounded_all().await?;

    let mut nodes_by_name = crate::prose::Nodes::new();
    for m in views.iter().flat_map(|g| &g.memories) {
        if let Some(name) = names.named(&m.reference) {
            nodes_by_name.insert(name, memory_id(&m.reference));
        }
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut memories: BTreeMap<gmr::Ref, Node> = BTreeMap::new();
    let mut barren = 0usize;

    for grounded in &views {
        let view = &grounded.view;
        let shape = crate::shapes::of(&view.anchor.transitions);
        let bound: Vec<gmr::Ref> = grounded
            .memories
            .iter()
            .map(|m| m.reference.clone())
            .collect();
        let delivering = bound.iter().any(|note| {
            subs.delivers(view.key.as_str(), shape, note, &view.state)
                .unwrap_or(true)
        });
        let moved = crate::delivery::axes_set(&view.state).is_some_and(|set| !set.is_empty());
        let unclaimed = bound.is_empty() && moved;
        if bound.is_empty() {
            barren += 1;
        }

        let key = view.key.to_string();
        nodes.push(anchor_node(view, anchor_tone(view, delivering, unclaimed)));

        for m in &grounded.memories {
            edges.push(Edge::new(
                memory_id(&m.reference),
                anchor_id(&key),
                EdgeKind::Binding,
            ));
            memories.entry(m.reference.clone()).or_insert_with(|| {
                let detail = m
                    .content()
                    .map(|b| crate::prose::to_html(&String::from_utf8_lossy(b), &nodes_by_name));
                memory_node(m, names, detail)
            });
        }
    }

    let present: Vec<gmr::Ref> = memories.keys().cloned().collect();
    for reference in &present {
        let Some(body) = views
            .iter()
            .flat_map(|g| &g.memories)
            .find(|m| &m.reference == reference)
            .and_then(gmr::MemoryView::content)
        else {
            continue;
        };
        let from = memory_id(reference);
        for name in crate::prose::wikilinks(&String::from_utf8_lossy(body)) {
            let Some(to) = nodes_by_name.get(&name) else {
                continue;
            };
            if *to == from {
                continue;
            }
            edges.push(Edge::new(from.clone(), to.clone(), EdgeKind::Reference));
        }
    }

    let memory_count = memories.len();
    nodes.extend(memories.into_values());

    let bindings = edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Binding))
        .count();
    let references = edges.len() - bindings;

    let repo: String = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into(),
    );
    let graph = Graph {
        title: "GMR Atlas".to_owned(),
        subtitle: format!("{repo} · {} anchors · {memory_count} memories", views.len()),
        logo: Some(logo()),
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
    fn a_definition_hangs_under_every_directory_and_the_file_it_lives_in() {
        assert_eq!(
            trail_of("crates/gmr-core/src/addr.rs#write_array"),
            ["crates", "gmr-core", "src", "addr.rs"]
        );
    }

    #[test]
    fn a_whole_file_hangs_under_its_directories_and_is_itself_the_leaf() {
        assert_eq!(
            trail_of("crates/gmr-core/src/addr.rs"),
            ["crates", "gmr-core", "src"]
        );
        assert_eq!(label_of("crates/gmr-core/src/addr.rs"), "addr.rs");
    }

    #[test]
    fn a_key_that_is_not_a_path_keeps_its_namespace_as_the_only_level() {
        assert_eq!(trail_of("layer::gmr-core"), ["layer"]);
        assert_eq!(label_of("layer::gmr-core"), "gmr-core");
        assert_eq!(trail_of("doctrine::decisions"), ["doctrine"]);
        assert_eq!(label_of("doctrine::decisions"), "decisions");
    }

    #[test]
    fn the_same_id_in_two_stores_is_two_nodes_not_one() {
        assert_ne!(
            memory_id(&gmr::Ref::new("git", "a.md")),
            memory_id(&gmr::Ref::new("mem0", "a.md")),
            "a node's identity has to be the whole reference. Keyed by the id alone, two \
             records that merely share a name collapse into one node: one label, one tone, \
             and both anchors' binding edges pointing at it — the page then says two \
             coordinates are watched by the same memory, which is a claim nobody made. \
             `external_id` was globally unique for exactly as long as git was the only store"
        );
    }

    fn view_memory(grounded: bool, rewritten: bool, stale: Option<bool>) -> MemoryView {
        MemoryView {
            reference: gmr::Ref::new("git", "memories/a.md"),
            bound_version: Some(gmr::Version::new("v1")),
            sources: std::collections::BTreeSet::from([gmr::Source::Adjudicated]),
            baseline_at: None,
            asserted_at: None,
            grounding: if rewritten {
                Grounding::Rewritten {
                    version: gmr::Version::new("v2"),
                    content: b"body".to_vec(),
                    before: Before::Retrieved {
                        content: b"was".to_vec(),
                    },
                }
            } else {
                Grounding::Current {
                    version: gmr::Version::new("v1"),
                    content: b"body".to_vec(),
                }
            },
            grounded,
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
    }

    #[test]
    fn nothing_a_row_shows_repeats_what_its_ancestors_already_say() {
        let key = "crates/gmr-core/src/addr.rs#write_array";
        let trail = trail_of(key);
        assert!(
            !trail.contains(&label_of(key)),
            "the leaf would say again what the branch above it already says"
        );
    }
}
