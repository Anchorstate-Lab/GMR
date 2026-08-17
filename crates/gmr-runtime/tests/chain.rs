use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched};
use gmr_core::{
    AnchorKey, Expr, ExternalId, ProviderId, Ref, Retain, Rule, RunSettings, Transitions, Version,
};
use gmr_runtime::{OpenRequest, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport::shell::Shell;

fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    gmr_transport::shell::testkit::install_script(root.join(".probes"), "cat", "cat world.json")
}

struct Files {
    root: PathBuf,
    id: ProviderId,
}

#[async_trait]
impl ContentProvider for Files {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(&self, id: &ExternalId) -> Result<Option<Fetched>, ContentError> {
        let path = self.root.join(id.as_str());
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| ContentError::new(e.to_string()))?;
        Ok(Some(Fetched {
            version: Version::new(gmr_core::content_hash_of_bytes(&bytes).into_inner()),
            bytes,
        }))
    }
}

const MEMORY: &str = "\
# Core Modules

There is no `fact` module; facts do not exist independently of the probe that produced them.
";

#[tokio::test]
async fn one_read_hands_back_both_the_change_and_the_memory_it_may_have_invalidated() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("memories")).unwrap();
    std::fs::write(dir.path().join("memories/core-modules.md"), MEMORY).unwrap();
    std::fs::write(
        dir.path().join("world.json"),
        r#"{"modules":["addr","probe"]}"#,
    )
    .unwrap();

    let bindings = Arc::new(MemoryBindings::default());
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
        .provider(Arc::new(Files {
            root: dir.path().to_path_buf(),
            id: ProviderId::new("git"),
        }))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(bindings.clone())
        .sealer(bindings.clone())
        .links(bindings)
        .settings(Arc::new(MemoryQueue::default()))
        .build();

    let key = AnchorKey::new("core::modules");

    rt.open(OpenRequest {
        key: key.clone(),
        probe: cat_probe(dir.path()),
        transitions: Transitions(vec![Rule {
            when: Expr::text("changed(\"modules\")"),
            to: Expr::text("{ modules: obs.modules }"),
        }]),
        terminal: Default::default(),
        initial: None,
        settings: RunSettings {
            budget_ms: None,
            retain: Retain::Tick,
            cadence_secs: None,
        },
        supersedes: None,
    })
    .await
    .unwrap();

    rt.bind(
        Ref::new("git", "memories/core-modules.md"),
        vec![key.clone()],
        Version::new("blob-at-bind-time"),
    )
    .await
    .unwrap();

    std::fs::write(
        dir.path().join("world.json"),
        r#"{"modules":["addr","probe","fact"]}"#,
    )
    .unwrap();
    rt.observe(&key).await.unwrap();

    let view = rt.read(&key).await.unwrap();
    let handed_back = serde_json::to_string(&view).unwrap();

    assert_eq!(
        view.state.as_value()["modules"],
        serde_json::json!(["addr", "probe", "fact"]),
        "the anchor should hand back what it sees now, not just that it changed"
    );
    assert!(
        handed_back.contains("fact"),
        "the handed-back view should contain the new value"
    );

    assert_eq!(view.memories.len(), 1);

    assert!(
        handed_back.contains("facts do not exist independently of the probe that produced them"),
        "read handed back memory content, not just its address; otherwise distinguishing memory from fact would happen outside the tool"
    );

    assert!(
        handed_back.contains("blob-at-bind-time"),
        "the bound version must come back, otherwise callers cannot tell whether they are reading the bound content"
    );
}
