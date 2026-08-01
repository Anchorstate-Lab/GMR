use std::sync::Arc;

use gmr_core::{AnchorKey, Change, Entry, Expr, Kind, Probe, Retain, Rule, Transitions, fold};
use gmr_runtime::{OpenRequest, Runtime};
use gmr_store::Journal;
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_transport_shell::Shell;

struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
    journal: Arc<MemoryJournal>,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let journal = Arc::new(MemoryJournal::default());
        let runtime = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path())))
            .journal(journal.clone())
            .bindings(Arc::new(MemoryBindings::default()))
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

fn probe(script: &str) -> Probe {
    Probe::new(Kind::new("shell"), serde_json::json!({ "run": script }))
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
            probe: probe("cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Full,
            cadence_secs: None,
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
                observation.fact_address.clone()
            }
            _ => None,
        })
        .collect();
    assert_eq!(addrs.len(), 2);
    assert_eq!(addrs[0], addrs[1], "世界没动，重跑必得同一个事实地址");
}

#[tokio::test]
async fn the_two_hops_version_independently() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.write("other.json", r#"{"shape":"v1"}"#);

    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe("cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Full,
            cadence_secs: None,
            supersedes: None,
        })
        .await
        .unwrap();

    w.runtime
        .revise(
            &key(),
            Change::Reprobe {
                probe: probe("cat other.json"),
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

    assert_ne!(versions[0].probe, versions[1].probe, "换探针 = 换派生规则");
    assert_eq!(
        versions[0].evaluator, versions[1].evaluator,
        "求值器是基底自己的，探针作者改不动它"
    );

    let gmr_runtime::Observed::Transitioned { from, to } = observed else {
        panic!("Reprobe 之后的第一次观测要留完整一条")
    };
    assert_eq!(from, to, "内容没变，状态就不该动");
}

#[tokio::test]
async fn the_fact_address_moves_when_the_rule_moves() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.write("other.json", r#"{"shape":"v1"}"#);

    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe("cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Full,
            cadence_secs: None,
            supersedes: None,
        })
        .await
        .unwrap();
    w.runtime
        .revise(
            &key(),
            Change::Reprobe {
                probe: probe("cat other.json"),
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
                observation.fact_address.clone()
            }
            _ => None,
        })
        .collect();
    assert_ne!(addrs[0], addrs[1], "地址定身份：派生规则是事实身份的一部分");
}

#[tokio::test]
async fn folding_the_same_log_twice_yields_the_same_state() {
    let w = World::new();
    w.write("world.json", r#"{"shape":"v1"}"#);
    w.runtime
        .open(OpenRequest {
            key: key(),
            probe: probe("cat world.json"),
            transitions: rules(&[("changed(\"shape\")", "{ shape: obs.shape }")]),
            terminal: Default::default(),
            initial: None,
            retain: Retain::Tick,
            cadence_secs: None,
            supersedes: None,
        })
        .await
        .unwrap();
    w.write("world.json", r#"{"shape":"v2"}"#);
    w.runtime.observe(&key()).await.unwrap();

    let entries = w.journal.as_ref().entries(&key(), 0).await.unwrap();
    assert_eq!(fold(&entries), fold(&entries));
}
