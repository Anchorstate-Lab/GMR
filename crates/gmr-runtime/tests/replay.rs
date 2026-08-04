use std::sync::Arc;

use gmr_core::{
    AnchorKey, Change, Entry, Expr, Kind, ProbeRef, Retain, Rule, RunSettings, Transitions, fold,
};
use gmr_runtime::{OpenRequest, Runtime};
use gmr_store::Journal;
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport::shell::Shell;

/// Every test publishes a real artifact. Otherwise "earned versions" would
/// hold on the production path while tests bypass it.
struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
    journal: Arc<MemoryJournal>,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let journal = Arc::new(MemoryJournal::default());
        let bindings = Arc::new(MemoryBindings::default());
        let runtime = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .journal(journal.clone())
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .build();
        Self {
            dir,
            runtime,
            journal,
        }
    }

    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.dir.path().join(name), contents).unwrap();
    }
}

fn key() -> AnchorKey {
    AnchorKey::new("a")
}

fn probe(root: &std::path::Path, script: &str) -> ProbeRef {
    let version = gmr_transport::shell::testkit::publish_script(root.join(".probes"), script);
    ProbeRef::new(Kind::new("shell"), version, serde_json::json!({}))
}

fn rules(pairs: &[(&str, &str)]) -> Transitions {
    Transitions(
        pairs
            .iter()
            .map(|(w, t)| Rule {
                when: Expr::text(*w),
                to: Expr::text(*t),
            })
            .collect(),
    )
}

#[tokio::test]
async fn the_same_probe_on_the_same_target_yields_the_same_facts() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe(w.dir.path(), "cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                retain: Retain::Full,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();

    let entries = w.journal.as_ref().entries(&key(), 0).await.unwrap();
    let addrs: Vec<_> = entries
        .iter()
        .filter_map(|(_, e)| match e {
            Entry::Open { observation, .. } | Entry::Transition { observation, .. } => {
                Some(observation.fact_address.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(addrs.len(), 2);
    assert_eq!(
        addrs[0], addrs[1],
        "a still world should rerun to the same fact address"
    );
}

#[tokio::test]
async fn the_two_hops_version_independently() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.write("other.json", r#"{"shape":"v1"}"#);

    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe(w.dir.path(), "cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                retain: Retain::Full,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();

    w.runtime
        .revise(
            &key(),
            Change::Reprobe {
                probe: probe(w.dir.path(), "cat other.json"),
            },
            b"same content, different rule",
        )
        .await
        .unwrap();
    let observed = w.runtime.observe(&key()).await.unwrap();

    let entries = w.journal.as_ref().entries(&key(), 0).await.unwrap();
    let versions: Vec<_> = entries
        .iter()
        .filter_map(|(_, e)| match e {
            Entry::Open { observation, .. } | Entry::Transition { observation, .. } => {
                Some(observation.versions.clone())
            }
            _ => None,
        })
        .collect();

    assert_ne!(
        versions[0].derivation.version, versions[1].derivation.version,
        "changing the probe changes the derivation rule"
    );
    assert_eq!(
        versions[0].evaluator, versions[1].evaluator,
        "the evaluator belongs to the substrate; probe authors cannot change it"
    );

    let gmr_runtime::Observed::Unchanged { .. } = observed else {
        panic!(
            "the first observation after Reprobe must retain a full entry, \
             but unchanged content must not be reported as a move: {observed:?}"
        )
    };
}

#[tokio::test]
async fn the_fact_address_moves_when_the_rule_moves() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.write("other.json", r#"{"shape":"v1"}"#);

    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe(w.dir.path(), "cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                retain: Retain::Full,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    w.runtime
        .revise(
            &key(),
            Change::Reprobe {
                probe: probe(w.dir.path(), "cat other.json"),
            },
            b"rule change",
        )
        .await
        .unwrap();
    w.runtime.observe(&key()).await.unwrap();

    let entries = w.journal.as_ref().entries(&key(), 0).await.unwrap();
    let addrs: Vec<_> = entries
        .iter()
        .filter_map(|(_, e)| match e {
            Entry::Open { observation, .. } | Entry::Transition { observation, .. } => {
                Some(observation.fact_address.clone())
            }
            _ => None,
        })
        .collect();
    assert_ne!(
        addrs[0], addrs[1],
        "the address defines identity; the derivation rule is part of fact identity"
    );
}

#[tokio::test]
async fn folding_the_same_log_twice_yields_the_same_state() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe(w.dir.path(), "cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            settings: RunSettings {
                retain: Retain::Tick,
                cadence_secs: None,
            },
            supersedes: None,
        })
        .await
        .unwrap();
    w.write("world.json", r#"{"shape":"v2"}"#);
    w.runtime.observe(&key()).await.unwrap();

    let entries = w.journal.as_ref().entries(&key(), 0).await.unwrap();
    assert_eq!(fold(&entries), fold(&entries));
}
