use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_core::{
    AnchorKey, Expr, ExternalId, Kind, Probe, ProviderId, Ref, Retain, Rule, Transitions, Version,
};
use gmr_runtime::{ContentError, ContentProvider, Fetched, OpenRequest, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_transport_shell::Shell;

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

    async fn fetch_at(
        &self,
        _id: &ExternalId,
        _version: &Version,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        Ok(None)
    }
}

const MEMORY: &str = "\
# core 的模块

没有 `fact` 模块 —— 事实没有独立于产出它的探针的存在。
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

    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path())))
        .provider(Arc::new(Files {
            root: dir.path().to_path_buf(),
            id: ProviderId::new("git"),
        }))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(Arc::new(MemoryBindings::default()))
        .build();

    let key = AnchorKey::new("core::modules");

    rt.open(OpenRequest {
        key: key.clone(),
        probe: Probe::new(
            Kind::new("shell"),
            serde_json::json!({ "run": "cat world.json" }),
        ),
        transitions: Transitions(vec![Rule {
            when: Expr::text("changed(\"modules\")"),
            to: Expr::text("{ modules: obs.modules }"),
        }]),
        terminal: Default::default(),
        initial: None,
        retain: Retain::Tick,
        cadence_secs: None,
    })
    .await
    .unwrap();

    rt.bind(
        Ref::new("git", "memories/core-modules.md"),
        vec![key.clone()],
        Version::new("blob-at-bind-time"),
        vec![],
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
        "锚该交出它现在看到的东西，不只是『变了』"
    );
    assert!(handed_back.contains("fact"), "交回去的那份里要读得出新值");

    assert_eq!(view.memories.len(), 1);

    assert!(
        handed_back.contains("事实没有独立于产出它的探针的存在"),
        "read 只交回了记忆的地址，没交回内容 —— \
         『自动识别记忆和事实的差别』这一步因此发生在工具外面"
    );

    assert!(
        handed_back.contains("blob-at-bind-time"),
        "绑定时那一版必须交回来，否则无从判断读到的是不是当初那份"
    );
}
