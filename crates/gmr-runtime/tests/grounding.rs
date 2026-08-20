use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use gmr_content::{ContentError, ContentProvider, Fetched, History};
use gmr_core::{
    AnchorKey, Expr, ExternalId, ProviderId, Ref, Retain, Rule, RunSettings, Transitions, Version,
};
use gmr_probe::Budget;
use gmr_runtime::{Before, Grounding, OpenRequest, Runtime, Standing};
use gmr_store::testkit::{MemoryBindings, MemoryJournal, MemoryQueue};
use gmr_transport::shell::Shell;

fn cat_probe(root: &std::path::Path) -> gmr_core::ProbeRef {
    gmr_transport::shell::testkit::install_script(root.join(".probes"), "cat", "cat world.json")
}

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

    async fn fetch(
        &self,
        id: &ExternalId,
        _budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
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

    fn history(&self) -> Option<&dyn History> {
        self.keeps_history.then_some(self as &dyn History)
    }
}

#[async_trait]
impl History for Versioned {
    async fn fetch_at(
        &self,
        _id: &ExternalId,
        version: &Version,
        _budget: &Budget,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        Ok(self.history.lock().unwrap().get(version.as_str()).cloned())
    }
}

struct Counted {
    root: PathBuf,
    id: ProviderId,
    fetch_at_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ContentProvider for Counted {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        _budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
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

#[async_trait]
impl History for Counted {
    async fn fetch_at(
        &self,
        _id: &ExternalId,
        _version: &Version,
        _budget: &Budget,
    ) -> Result<Option<Vec<u8>>, ContentError> {
        self.fetch_at_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(
            b"a caller that got here was never supposed to".to_vec(),
        ))
    }
}

struct Broken {
    id: ProviderId,
}

#[async_trait]
impl ContentProvider for Broken {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        _id: &ExternalId,
        _budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        Err(ContentError::new("the store did not answer"))
    }
}

struct Refusing {
    root: PathBuf,
    id: ProviderId,
    refuses: &'static str,
}

#[async_trait]
impl ContentProvider for Refusing {
    fn provider(&self) -> &ProviderId {
        &self.id
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        _budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        if id.as_str().ends_with(self.refuses) {
            return Err(ContentError::spent(
                "this call's slice of the budget was gone",
            ));
        }
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

struct World {
    dir: tempfile::TempDir,
    runtime: Runtime,
}

impl World {
    fn new(keeps_history: bool) -> Self {
        Self::with(|root| Arc::new(Versioned::new(root, keeps_history)))
    }

    fn unreachable() -> Self {
        Self::with(|_| {
            Arc::new(Broken {
                id: ProviderId::new("git"),
            })
        })
    }

    fn with(provider: impl FnOnce(PathBuf) -> Arc<dyn ContentProvider>) -> Self {
        Self::budgeted(provider, gmr_runtime::Policy::default())
    }

    fn budgeted(
        provider: impl FnOnce(PathBuf) -> Arc<dyn ContentProvider>,
        policy: gmr_runtime::Policy,
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("memories")).unwrap();
        std::fs::write(dir.path().join("world.json"), r#"{"x":1}"#).unwrap();
        let bindings = Arc::new(MemoryBindings::default());
        let runtime = Runtime::builder()
            .policy(policy)
            .transport(Arc::new(Shell::new(dir.path(), dir.path().join(".probes"))))
            .provider(provider(dir.path().to_path_buf()))
            .journal(Arc::new(MemoryJournal::default()))
            .bindings(bindings.clone())
            .sealer(bindings.clone())
            .links(bindings)
            .settings(Arc::new(MemoryQueue::default()))
            .sightings(Arc::new(MemoryQueue::default()))
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
                probe: cat_probe(self.dir.path()),
                transitions: Transitions(vec![Rule {
                    when: Expr::text("changed(\"x\")"),
                    to: Expr::text("{ x: obs.x }"),
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
    }

    async fn bind_at(&self, name: &str, anchors: &[&str], version: &str) {
        self.runtime
            .bind(
                Ref::new("git", format!("memories/{name}")),
                anchors.iter().map(|a| AnchorKey::new(*a)).collect(),
                Version::new(version),
            )
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
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_rewritten_record_emits_an_edge_with_both_versions() {
    let w = World::new(true);
    w.memory("a.md", "The anchor module roster is the contract itself.");
    w.open("a").await;
    w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    w.bind("a.md", &["a"]).await;

    let before = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        !before
            .standing
            .iter()
            .flatten()
            .any(|e| matches!(e, Standing::Rewritten { .. })),
        "not rewritten yet"
    );

    w.memory("a.md", "Changed claim: the roster is only a shadow.");

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    let m = &view.memories[0];
    let Grounding::Rewritten {
        content, before, ..
    } = &m.grounding
    else {
        panic!("expected a rewritten grounding, got {:?}", m.grounding);
    };
    assert_eq!(
        before,
        &Before::Retrieved {
            content: b"The anchor module roster is the contract itself.".to_vec()
        },
        "judging whether it still says the same thing requires both before and after"
    );
    assert_eq!(content, b"Changed claim: the roster is only a shadow.");

    let after = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        after.standing.iter().flatten().any(|e| matches!(
            e,
            Standing::Rewritten {
                before: Before::Retrieved { .. },
                ..
            }
        )),
        "record rewrites must be reported: {:?}",
        after.standing
    );
}

#[tokio::test]
async fn reaffirming_clears_rewritten_without_touching_anchors() {
    let w = World::new(true);
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    w.memory("a.md", "Just a typo fix, nothing structural.");
    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert!(view.memories[0].rewritten(), "content moved since bind");

    let reference = Ref::new("git", "memories/a.md");
    let bytes = std::fs::read(w.dir.path().join("memories/a.md")).unwrap();
    let current = gmr_core::content_hash_of_bytes(&bytes).into_inner();
    w.runtime
        .reaffirm(&reference, Version::new(current))
        .await
        .unwrap();

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert!(
        !view.memories[0].rewritten(),
        "reaffirm re-stamped the version, so it's no longer stale"
    );
    assert_eq!(
        view.memories.len(),
        1,
        "reaffirm must not have disturbed which anchors this is bound to"
    );
}

#[tokio::test]
async fn reaffirming_an_unbound_reference_is_refused() {
    let w = World::new(true);
    let err = w
        .runtime
        .reaffirm(
            &Ref::new("git", "memories/never-bound.md"),
            Version::new("v"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_bound");
}

#[tokio::test]
async fn an_unreachable_bound_version_is_flagged_not_silently_dropped() {
    let w = World::new(false);
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    w.bind("a.md", &["a"]).await;
    w.memory("a.md", "Edited wording.");

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    let m = &view.memories[0];
    assert!(
        matches!(
            m.grounding,
            Grounding::Rewritten {
                before: Before::NoHistory,
                ..
            }
        ),
        "this provider does not implement History at all, which is a different fact from a version it failed to keep: {:?}",
        m.grounding
    );

    assert!(
        w.runtime
            .changed_since(0, None)
            .await
            .unwrap()
            .standing
            .iter()
            .flatten()
            .any(|e| matches!(
                e,
                Standing::Rewritten {
                    before: Before::NoHistory,
                    ..
                }
            )),
        "a rewrite must still be reported when the before cannot be shown; the anchor moved either way"
    );
}

#[tokio::test]
async fn cobound_is_derived_from_binds_not_stored() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.memory("b.md", "Two.");
    w.memory("c.md", "Three.");
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
        .bind(Ref::new("git", "memories/b.md"), vec![], Version::new("v"))
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
async fn an_unanchored_record_is_carried_along_but_marked() {
    use gmr_core::LinkKind;

    let w = World::new(true);
    w.memory("bound.md", "This one is anchored.");
    w.memory(
        "loose.md",
        "This one is not anchored, but the one above links to it.",
    );
    w.open("a").await;

    w.runtime
        .bind(
            Ref::new("git", "memories/bound.md"),
            vec![AnchorKey::new("a")],
            Version::new("v1"),
        )
        .await
        .unwrap();
    w.runtime
        .bind(
            Ref::new("git", "memories/loose.md"),
            vec![],
            Version::new("v1"),
        )
        .await
        .unwrap();
    w.runtime
        .link(
            &Ref::new("git", "memories/bound.md"),
            &Ref::new("git", "memories/loose.md"),
            LinkKind("elaborates".into()),
        )
        .await
        .unwrap();

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    let by_id = |id: &str| {
        view.memories
            .iter()
            .find(|m| m.reference.external_id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} should be present here: {:?}", view.memories))
    };

    assert!(by_id("memories/bound.md").grounded);
    assert!(
        !by_id("memories/loose.md").grounded,
        "it was carried along but gets no guarantee, so that must be visible"
    );
    assert_eq!(
        by_id("memories/loose.md").content(),
        Some(b"This one is not anchored, but the one above links to it.".as_slice())
    );
}

#[tokio::test]
async fn a_detached_record_is_no_longer_listed_under_the_anchor() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    let reference = Ref::new("git", "memories/a.md");
    let bound = w
        .runtime
        .memory()
        .binding_of(&reference)
        .await
        .unwrap()
        .unwrap();
    assert!(!bound.binding.anchors.is_empty());

    w.runtime
        .bind(reference.clone(), vec![], Version::new("v"))
        .await
        .unwrap();

    let detached = w
        .runtime
        .memory()
        .binding_of(&reference)
        .await
        .unwrap()
        .unwrap();
    assert!(
        detached.binding.anchors.is_empty(),
        "detached, while history remains in the table"
    );
    assert!(
        w.runtime
            .grounded(&AnchorKey::new("a"))
            .await
            .unwrap()
            .memories
            .is_empty(),
        "no longer attached to this anchor"
    );
}

#[tokio::test]
async fn a_provider_nobody_registered_is_our_fault_not_the_worlds_answer() {
    let w = World::new(true);
    let unregistered = Ref::new("mem0", "some-uuid");

    let err = w.runtime.current_version(&unregistered).await.expect_err(
        "an unregistered provider used to come back as Ok(None), which reads exactly like \
             the provider answering that the record is gone. Callers then told the user their \
             record did not exist, when the truth was that this binary cannot reach that store",
    );

    assert_eq!(err.code(), "no_provider");
    assert!(
        err.to_string().contains("mem0"),
        "the message has to name the provider that is missing: {err}"
    );
}

#[tokio::test]
async fn a_registered_provider_that_has_no_such_record_still_answers_none() {
    let w = World::new(true);
    let absent = Ref::new("git", "memories/never-written.md");

    let version = w
        .runtime
        .current_version(&absent)
        .await
        .expect("reaching the provider worked; it simply has nothing under that id");

    assert!(version.is_none(), "this half is the world's answer");
}

#[tokio::test]
async fn a_version_the_provider_did_not_keep_is_not_the_same_as_a_provider_that_keeps_nothing() {
    let w = World::new(true);
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.bind_at("a.md", &["a"], "a-version-this-provider-never-recorded")
        .await;

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    let m = &view.memories[0];

    assert!(
        matches!(
            m.grounding,
            Grounding::Rewritten {
                before: Before::NotRetained,
                ..
            }
        ),
        "this provider does implement History; it simply has nothing under that version. \
         Reporting it as NoHistory would send the reader to fix the wrong thing — one is \
         a property of the backend, the other of this one binding: {:?}",
        m.grounding
    );
}

#[tokio::test]
async fn a_store_that_will_not_answer_is_reported_not_read_as_nothing_happened() {
    let w = World::unreachable();
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.bind_at("a.md", &["a"], "whatever-was-stamped").await;

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert!(
        matches!(view.memories[0].grounding, Grounding::Unreachable { .. }),
        "{:?}",
        view.memories[0].grounding
    );

    let standing = w.runtime.changed_since(0, None).await.unwrap().standing;
    assert!(
        standing
            .iter()
            .flatten()
            .any(|e| matches!(e, Standing::Unreachable { .. })),
        "a provider that cannot be reached used to leave `rewritten` false, so this walk \
         emitted nothing at all and `gmr edges` told the reader everything was fine. \
         Not knowing is a standing condition of its own: {standing:?}"
    );
}

#[tokio::test]
async fn declining_to_offer_history_means_fetch_at_is_never_reached() {
    let calls = Arc::new(AtomicUsize::new(0));
    let w = {
        let calls = Arc::clone(&calls);
        World::with(move |root| {
            Arc::new(Counted {
                root,
                id: ProviderId::new("git"),
                fetch_at_calls: calls,
            })
        })
    };
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;
    w.memory("a.md", "Edited wording.");

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "this type does implement History, but reports none through history(). A capability \
         expressed by a trait is only honest if declining it actually stops the call — the \
         shape before this one asked every provider and let it answer \"I have none\", which \
         is a different guarantee and a weaker one"
    );
    assert!(
        matches!(
            view.memories[0].grounding,
            Grounding::Rewritten {
                before: Before::NoHistory,
                ..
            }
        ),
        "and the reader learns why there is no before from history() alone: {:?}",
        view.memories[0].grounding
    );
}

#[tokio::test]
async fn a_total_budget_already_spent_asks_the_store_nothing_at_all() {
    let calls = Arc::new(AtomicUsize::new(0));
    let w = {
        let calls = Arc::clone(&calls);
        World::budgeted(
            move |root| {
                Arc::new(Counted {
                    root,
                    id: ProviderId::new("git"),
                    fetch_at_calls: calls,
                })
            },
            gmr_runtime::Policy {
                content_total_ms: 0,
                ..Default::default()
            },
        )
    };
    w.memory("a.md", "Original wording.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();

    assert!(
        matches!(
            view.memories[0].grounding,
            Grounding::Unreachable {
                code: gmr_content::ContentErrorCode::BudgetSpent,
                ..
            }
        ),
        "a spent total must read as not-knowing, never as the record being gone: {:?}",
        view.memories[0].grounding
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "and it must not have been asked; the point of a total is to stop starting work"
    );
}

#[tokio::test]
async fn one_record_running_out_of_budget_does_not_take_the_others_with_it() {
    let w = World::with(|root| {
        Arc::new(Refusing {
            root,
            id: ProviderId::new("git"),
            refuses: "slow.md",
        })
    });
    w.memory("slow.md", "This one costs more than its share.");
    w.memory("quick.md", "This one is right here.");
    w.open("a").await;
    w.bind("slow.md", &["a"]).await;
    w.bind("quick.md", &["a"]).await;

    let view = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    let by_id = |id: &str| {
        view.memories
            .iter()
            .find(|m| m.reference.external_id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} missing: {:?}", view.memories))
    };

    assert!(
        matches!(
            by_id("memories/slow.md").grounding,
            Grounding::Unreachable { .. }
        ),
        "{:?}",
        by_id("memories/slow.md").grounding
    );
    assert!(
        matches!(
            by_id("memories/quick.md").grounding,
            Grounding::Current { .. }
        ),
        "each record gets its own narrowed slice, so one of them giving up says nothing \
         about the rest. The alternative — a breaker that trips after N failures — would \
         make this record's answer depend on how many others happened to be walked first: \
         {:?}",
        by_id("memories/quick.md").grounding
    );
}
