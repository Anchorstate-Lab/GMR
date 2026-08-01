use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_core::{
    AnchorKey, Expr, ExternalId, Kind, Probe, ProviderId, Ref, Retain, Rule, Transitions, Version,
};
use gmr_runtime::{ContentError, ContentProvider, Edge, Fetched, OpenRequest, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_transport_shell::Shell;

struct Versioned {
    root: PathBuf,
    history: std::sync::Mutex<BTreeMap<String, Vec<u8>>>,
    id: ProviderId,
    keeps_history: bool,
}

impl Versioned {
    fn new(root: PathBuf, keeps_history: bool) -> Self {
        Self {
            root,
            history: Default::default(),
            id: ProviderId::new("git"),
            keeps_history,
        }
    }
}

#[async_trait]
impl ContentProvider for Versioned {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(&self, id: &ExternalId) -> Result<Option<Fetched>, ContentError> {
        let path = self.root.join(id.as_str());
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| ContentError::new(e.to_string()))?;
        let version = gmr_core::content_hash_of_bytes(&bytes).into_inner();
        self.history
            .lock()
            .unwrap()
            .insert(version.clone(), bytes.clone());
        Ok(Some(Fetched {
            version: Version::new(version),
            bytes,
        }))
    }

    async fn fetch_at(
        &self,
        _id: &ExternalId,
        version: &Version,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        if !self.keeps_history {
            return Ok(None);
        }
        Ok(self.history.lock().unwrap().get(version.as_str()).cloned())
    }
}

struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
}

impl World {
    fn new(keeps_history: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("memories")).unwrap();
        std::fs::write(dir.path().join("world.json"), r#"{"x":1}"#).unwrap();
        let runtime = Runtime::builder()
            .transport(Arc::new(Shell::new(dir.path())))
            .provider(Arc::new(Versioned::new(
                dir.path().to_path_buf(),
                keeps_history,
            )))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(Arc::new(MemoryBindings::default()))
            .build();
        Self { dir, runtime }
    }

    fn memory(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join("memories").join(name), text).unwrap();
    }

    async fn open(&self, key: &str) {
        self.runtime
            .open(OpenRequest {
                key: AnchorKey::new(key),
                probe: Probe::new(
                    Kind::new("shell"),
                    serde_json::json!({ "run": "cat world.json" }),
                ),
                transitions: Transitions(vec![Rule {
                    when: Expr::text("changed(\"x\")"),
                    to: Expr::text("{ x: obs.x }"),
                }]),
                terminal: Default::default(),
                initial: None,
                retain: Retain::Tick,
                cadence_secs: None,
                supersedes: None,
            })
            .await
            .unwrap();
    }

    async fn bind(&self, name: &str, anchors: &[&str]) {
        let reference = Ref::new("git", format!("memories/{name}"));
        let bytes = std::fs::read(self.dir.path().join("memories").join(name)).unwrap();
        let version = gmr_core::content_hash_of_bytes(&bytes).into_inner();
        self.runtime
            .bind(
                reference,
                anchors.iter().map(|a| AnchorKey::new(*a)).collect(),
                Version::new(version),
                vec![],
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_rewritten_record_emits_an_edge_with_both_versions() {
    let w = World::new(true);
    w.memory("a.md", "锚的模块名单就是契约本体。");
    w.open("a").await;
    w.runtime.read(&AnchorKey::new("a")).await.unwrap();
    w.bind("a.md", &["a"]).await;

    let before = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        !before
            .edges
            .iter()
            .any(|e| matches!(e, Edge::Rewritten { .. })),
        "还没改写"
    );

    w.memory("a.md", "改口了：名单只是影子。");

    let view = w.runtime.read(&AnchorKey::new("a")).await.unwrap();
    let m = &view.memories[0];
    assert!(m.rewritten, "绑定以来被改写过");
    assert_eq!(m.retrievable, Some(true));
    assert_eq!(
        m.content_at_bind.as_deref(),
        Some("锚的模块名单就是契约本体。"),
        "判断「它还在说同一件事吗」要的是从什么改成了什么"
    );
    assert_eq!(m.content.as_deref(), Some("改口了：名单只是影子。"));

    let after = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        after.edges.iter().any(|e| matches!(
            e,
            Edge::Rewritten {
                retrievable: Some(true),
                ..
            }
        )),
        "记录改写必须发边沿：{:?}",
        after.edges
    );
}

#[tokio::test]
async fn an_unreachable_bound_version_is_flagged_not_silently_dropped() {
    let w = World::new(false);
    w.memory("a.md", "原话。");
    w.open("a").await;
    w.runtime.read(&AnchorKey::new("a")).await.unwrap();
    w.bind("a.md", &["a"]).await;
    w.memory("a.md", "改过的话。");

    let view = w.runtime.read(&AnchorKey::new("a")).await.unwrap();
    let m = &view.memories[0];
    assert!(m.rewritten);
    assert_eq!(m.retrievable, Some(false), "history rewrite 之后不可达");
    assert_eq!(m.content_at_bind, None);

    assert!(
        w.runtime
            .changed_since(0, None)
            .await
            .unwrap()
            .edges
            .iter()
            .any(|e| matches!(
                e,
                Edge::Rewritten {
                    retrievable: Some(false),
                    ..
                }
            )),
        "取不回也要出声 —— 那是「从什么改成什么」永远问不出来了"
    );
}

#[tokio::test]
async fn cobound_is_derived_from_binds_not_stored() {
    let w = World::new(true);
    w.memory("a.md", "一。");
    w.memory("b.md", "二。");
    w.memory("c.md", "三。");
    w.open("a").await;
    w.open("b").await;

    w.bind("a.md", &["a"]).await;
    w.bind("b.md", &["a"]).await;
    w.bind("c.md", &["b"]).await;

    let same = w
        .runtime
        .cobound(&Ref::new("git", "memories/a.md"))
        .await
        .unwrap();
    assert_eq!(same, vec![Ref::new("git", "memories/b.md")]);

    assert!(
        w.runtime
            .cobound(&Ref::new("git", "memories/c.md"))
            .await
            .unwrap()
            .is_empty()
    );

    w.runtime
        .bind(
            Ref::new("git", "memories/b.md"),
            vec![],
            Version::new("v"),
            vec![],
        )
        .await
        .unwrap();
    assert!(
        w.runtime
            .cobound(&Ref::new("git", "memories/a.md"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_detached_record_is_marked_ungrounded() {
    let w = World::new(true);
    w.memory("a.md", "一。");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    let reference = Ref::new("git", "memories/a.md");
    let bound = w
        .runtime
        .bindings()
        .binding_of(&reference)
        .await
        .unwrap()
        .unwrap();
    assert!(!bound.anchors.is_empty());

    w.runtime
        .bind(reference.clone(), vec![], Version::new("v"), vec![])
        .await
        .unwrap();

    let detached = w
        .runtime
        .bindings()
        .binding_of(&reference)
        .await
        .unwrap()
        .unwrap();
    assert!(detached.anchors.is_empty(), "摘走了，但历史留在表里");
    assert!(
        w.runtime
            .read(&AnchorKey::new("a"))
            .await
            .unwrap()
            .memories
            .is_empty(),
        "不再挂在这个锚上"
    );
}
