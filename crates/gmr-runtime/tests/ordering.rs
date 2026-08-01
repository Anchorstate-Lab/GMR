use std::sync::Arc;

use gmr_core::{AnchorKey, Change, Entry, Expr, Retain, Rule, State, Transitions};
use gmr_runtime::{OpenRequest, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_store::{BindingStore, Journal};
use gmr_transport_shell::Shell;

/// 每个测试都发布一个真的 artifact —— 否则「版本是挣来的」这条只在
/// 生产路径上成立，测试反而绕过了它。
fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    let version =
        gmr_transport_shell::testkit::publish_script(root.join(".probes"), "cat world.json");
    gmr_core::ProbeRef::new(gmr_core::Kind::new("shell"), version, serde_json::json!({}))
}

#[tokio::test]
async fn every_sealed_address_a_revise_cites_is_retrievable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("world.json"), r#"{"x":1}"#).unwrap();

    let journal = Arc::new(MemoryJournal::default());
    let bindings = Arc::new(MemoryBindings::default());
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
        .journal(journal.clone())
        .bindings(bindings.clone())
        .build();

    let key = AnchorKey::new("a");
    rt.open(OpenRequest {
        key: key.clone(),
        probe: cat_probe(dir.path()),
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

    std::fs::write(dir.path().join("world.json"), r#"{"x":2}"#).unwrap();
    rt.observe(&key).await.unwrap();
    rt.revise(
        &key,
        Change::Restate {
            state: State::new(serde_json::json!({ "x": 2 })),
        },
        "接受".as_bytes(),
    )
    .await
    .unwrap();
    rt.close(&key, "收尾".as_bytes()).await.unwrap();

    let mut cited = 0;
    for (_, entry) in journal.entries(&key, 0).await.unwrap() {
        let addrs = match &entry {
            Entry::Revise {
                context, rationale, ..
            } => vec![context.clone(), rationale.clone()],
            Entry::Close {
                context, rationale, ..
            } => vec![context.clone(), rationale.clone()],
            _ => vec![],
        };
        for a in addrs {
            assert!(
                bindings.sealed(&a).await.unwrap().is_some(),
                "{} 引用了 {a}，但那个地址在密封存储里不存在 —— 链断了",
                entry.name()
            );
            cited += 1;
        }
    }
    assert_eq!(
        cited, 4,
        "Revise 与作者关锚各引两个：context 基底捕获，rationale 作者写"
    );
}

#[tokio::test]
async fn an_orphan_seal_is_harmless_garbage() {
    let bindings = MemoryBindings::default();

    let orphan = bindings
        .seal("一条没能落地的理由".as_bytes())
        .await
        .unwrap();

    assert!(bindings.sealed(&orphan).await.unwrap().is_some());
    assert_eq!(
        orphan,
        bindings
            .seal("一条没能落地的理由".as_bytes())
            .await
            .unwrap()
    );
}
