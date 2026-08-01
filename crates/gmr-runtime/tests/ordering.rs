use std::sync::Arc;

use gmr_core::{AnchorKey, Change, Entry, Expr, Kind, Probe, Retain, Rule, State, Transitions};
use gmr_runtime::{OpenRequest, Runtime};
use gmr_store::testkit::{MemoryBindings, MemoryJournal};
use gmr_store::{BindingStore, Journal};
use gmr_transport_shell::Shell;

#[tokio::test]
async fn every_sealed_address_a_revise_cites_is_retrievable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("w.json"), r#"{"x":1}"#).unwrap();

    let journal = Arc::new(MemoryJournal::default());
    let bindings = Arc::new(MemoryBindings::default());
    let rt = Runtime::builder()
        .transport(Arc::new(Shell::new(dir.path())))
        .journal(journal.clone())
        .bindings(bindings.clone())
        .build();

    let key = AnchorKey::new("a");
    rt.open(OpenRequest {
        key: key.clone(),
        probe: Probe::new(
            Kind::new("shell"),
            serde_json::json!({ "run": "cat w.json" }),
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

    std::fs::write(dir.path().join("w.json"), r#"{"x":2}"#).unwrap();
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
