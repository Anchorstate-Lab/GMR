use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use gmr_budget::Budget;
use gmr_content::{ContentError, ContentProvider, Fetched, History};
use gmr_core::{
    AnchorKey, Expr, ExternalId, ProviderId, Ref, Retain, Rule, RunSettings, Transitions, Version,
};
use gmr_runtime::{Before, Grounding, OpenRequest, Raised, Runtime};
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
                    facts: gmr_core::Recorded::Plain,
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
                gmr_core::Binding::on(
                    Ref::new("git", format!("memories/{name}")),
                    anchors.iter().map(|a| AnchorKey::new(*a)).collect(),
                ),
                Some(Version::new(version)),
                Default::default(),
                gmr_core::Source::Adjudicated,
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
                gmr_core::Binding::on(
                    reference,
                    anchors.iter().map(|a| AnchorKey::new(*a)).collect(),
                ),
                Some(Version::new(version)),
                Default::default(),
                gmr_core::Source::Adjudicated,
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
            .raised
            .iter()
            .flatten()
            .any(|e| matches!(e, Raised::Rewritten { .. })),
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
        after.raised.iter().flatten().any(|e| matches!(
            e,
            Raised::Rewritten {
                before: Before::Retrieved { .. },
                ..
            }
        )),
        "record rewrites must be reported: {:?}",
        after.raised
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
        .reaffirm(&reference.clone().into(), Some(Version::new(current)))
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
            &Ref::new("git", "memories/never-bound.md").into(),
            Some(Version::new("v")),
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
            .raised
            .iter()
            .flatten()
            .any(|e| matches!(
                e,
                Raised::Rewritten {
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
        .cobound(&Ref::new("git", "memories/a.md").into())
        .await
        .unwrap();
    assert_eq!(
        same,
        vec![gmr_core::Claim::from(Ref::new("git", "memories/b.md"))]
    );

    assert!(
        w.runtime
            .cobound(&Ref::new("git", "memories/c.md").into())
            .await
            .unwrap()
            .is_empty()
    );

    w.runtime
        .revoke(
            &Ref::new("git", "memories/b.md").into(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();
    assert!(
        w.runtime
            .cobound(&Ref::new("git", "memories/a.md").into())
            .await
            .unwrap()
            .is_empty(),
        "cobound reads the same delivered set `read` does, so revoking one of the two \
         records takes it out of the other's neighbours too"
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
            gmr_core::Binding::on(
                Ref::new("git", "memories/bound.md"),
                vec![AnchorKey::new("a")],
            ),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();
    w.runtime
        .bind(
            gmr_core::Binding::on(Ref::new("git", "memories/loose.md"), vec![]),
            Some(Version::new("v1")),
            Default::default(),
            gmr_core::Source::Adjudicated,
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
async fn an_assertion_made_when_the_store_could_not_answer_is_unverified_not_refused() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;

    let reference = Ref::new("git", "memories/a.md");
    w.runtime
        .bind(
            gmr_core::Binding::on(reference.clone(), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let held = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert_eq!(
        held.memories[0].footing(),
        gmr_runtime::Footing::Unverified,
        "an agent binds the moment it writes a memory, which is when the link is most \
         accurate and when the store is least likely to answer yet. Refusing there throws \
         that link away; reporting the assertion as though a baseline stood behind it \
         would claim a comparison nobody made"
    );
    assert!(
        held.memories[0].bound_version.is_none(),
        "there is no baseline to name, and a placeholder would be indistinguishable from \
         one the store really issued"
    );

    w.runtime
        .reaffirm(
            &reference.clone().into(),
            w.runtime.current_version(&reference).await.unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        w.runtime
            .grounded(&AnchorKey::new("a"))
            .await
            .unwrap()
            .memories[0]
            .footing(),
        gmr_runtime::Footing::Current,
        "a later act that does reach the store establishes the baseline. Nothing on the \
         read path writes one — a read says what it found, it does not settle anything"
    );
}

#[tokio::test]
async fn a_later_assertion_that_verified_nothing_does_not_unverify_what_was_verified() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    let reference = Ref::new("git", "memories/a.md");
    let pinned = w.runtime.current_version(&reference).await.unwrap();

    w.runtime
        .bind(
            gmr_core::Binding::on(reference.clone(), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let held = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert_eq!(
        held.memories[0].bound_version, pinned,
        "an agent re-attesting before the store can answer says the record is still \
             about this anchor. It compared nothing, so it has nothing to overwrite \
             the standing baseline with, and taking its silence as the new baseline \
             would throw away a reading somebody really took"
    );
    assert_eq!(held.memories[0].footing(), gmr_runtime::Footing::Current);
}

#[tokio::test]
async fn a_revoked_record_is_no_longer_listed_under_the_anchor() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;

    let reference = Ref::new("git", "memories/a.md");
    let bound = w
        .runtime
        .memory()
        .binding_of(&reference.clone().into())
        .await
        .unwrap();
    assert!(!bound.anchors().is_empty());

    let cleared = w
        .runtime
        .revoke(&reference.clone().into(), gmr_core::Source::Adjudicated)
        .await
        .unwrap();
    assert_eq!(cleared, vec![AnchorKey::new("a")]);

    assert!(
        w.runtime
            .memory()
            .binding_of(&reference.clone().into())
            .await
            .unwrap()
            .anchors()
            .is_empty(),
        "revoked, while every assertion and the revocation itself remain in the table"
    );
    assert!(
        w.runtime
            .grounded(&AnchorKey::new("a"))
            .await
            .unwrap()
            .memories
            .is_empty(),
        "no longer delivered under this anchor"
    );
}

#[tokio::test]
async fn asserting_an_empty_anchor_set_takes_nothing_away() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    w.bind("a.md", &["a"]).await;
    let reference = Ref::new("git", "memories/a.md");

    w.runtime
        .bind(
            gmr_core::Binding::on(reference.clone(), vec![]),
            Some(Version::new("v")),
            Default::default(),
            gmr_core::Source::Adjudicated,
        )
        .await
        .unwrap();

    assert_eq!(
        w.runtime
            .memory()
            .binding_of(&reference.clone().into())
            .await
            .unwrap()
            .anchors(),
        vec![AnchorKey::new("a")],
        "an assertion naming no anchor adds no tag, so it can take none away either. \
         Writing one used to be how a record was detached — under latest-wins it replaced \
         the whole set. A caller that means to remove something now has to say which tags \
         it observed, which is the only form a reader can audit"
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

    let standing = w.runtime.changed_since(0, None).await.unwrap().raised;
    assert!(
        standing
            .iter()
            .flatten()
            .any(|e| matches!(e, Raised::Unreachable { .. })),
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

#[tokio::test]
async fn an_assertion_that_says_what_already_stands_writes_nothing() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    let reference = Ref::new("git", "memories/a.md");

    w.bind("a.md", &["a"]).await;
    w.bind("a.md", &["a"]).await;
    w.bind("a.md", &["a"]).await;

    assert_eq!(
        w.runtime
            .memory()
            .binding_of(&reference.clone().into())
            .await
            .unwrap()
            .assertions()
            .len(),
        1,
        "the table is append-only, so a repeated assertion is a row nobody can take back \
         that no reader can act on: the anchor union, the baseline, the source set and the \
         first-asserted time all come out identical. Whether a write says anything new is a \
         question about the projection, and it has to be asked before the write — a writer \
         deciding it for itself is how every re-run of every writer grows the table forever"
    );
}

#[tokio::test]
async fn a_second_kind_of_assertion_on_the_same_link_is_not_a_repeat() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    let reference = Ref::new("git", "memories/a.md");

    w.bind("a.md", &["a"]).await;
    let version = w
        .runtime
        .memory()
        .binding_of(&reference.clone().into())
        .await
        .unwrap()
        .bound_version()
        .cloned();
    w.runtime
        .bind(
            gmr_core::Binding::on(reference.clone(), vec![AnchorKey::new("a")]),
            version,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let bound = w
        .runtime
        .memory()
        .binding_of(&reference.clone().into())
        .await
        .unwrap();
    assert_eq!(
        bound.assertions().len(),
        2,
        "who says a link holds is part of what an assertion says, so a second party \
         asserting it is new information even at the same version. This is also what lets a \
         binding recorded before its origin was known be re-derived exactly once: the \
         re-derivation says something the projection did not, and the run after it does not"
    );
    assert_eq!(
        bound.sources(),
        std::collections::BTreeSet::from([
            gmr_core::Source::Adjudicated,
            gmr_core::Source::SelfAttested
        ]),
    );
}

#[tokio::test]
async fn reaffirm_records_a_reading_and_a_reading_is_never_a_repeat() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    let reference = Ref::new("git", "memories/a.md");

    w.bind("a.md", &["a"]).await;
    let version = w
        .runtime
        .memory()
        .binding_of(&reference.clone().into())
        .await
        .unwrap()
        .bound_version()
        .cloned();
    w.runtime
        .reaffirm(&reference.clone().into(), version)
        .await
        .unwrap();

    assert_eq!(
        w.runtime
            .memory()
            .binding_of(&reference.clone().into())
            .await
            .unwrap()
            .assertions()
            .len(),
        2,
        "`reaffirm` takes no anchors because it states no aboutness — it stamps a reading \
         taken at a moment. Two readings of the same bytes at different moments are two \
         readings, so the guard that suppresses a repeated assertion must not reach this \
         path, or the one way to say `I have looked at this again` disappears"
    );
}

#[tokio::test]
async fn an_anchor_names_each_memory_once_however_many_assertions_stand_on_it() {
    let w = World::new(true);
    w.memory("a.md", "One.");
    w.open("a").await;
    let reference = Ref::new("git", "memories/a.md");

    w.bind("a.md", &["a"]).await;
    let version = w
        .runtime
        .memory()
        .binding_of(&reference.clone().into())
        .await
        .unwrap()
        .bound_version()
        .cloned();
    for source in [gmr_core::Source::SelfAttested, gmr_core::Source::Configured] {
        w.runtime
            .bind(
                gmr_core::Binding::on(reference.clone(), vec![AnchorKey::new("a")]),
                version.clone(),
                Default::default(),
                source,
            )
            .await
            .unwrap();
    }

    let on = w.runtime.bindings_on(&AnchorKey::new("a")).await.unwrap();
    assert_eq!(
        on.len(),
        1,
        "three parties asserting one link is three assertions and one memory. Both \
         directions of the projection answer per reference, so a roster cannot repeat a \
         name by forgetting to collapse one: `check` printed a memory once per assertion \
         while `doctor` printed it once, and neither verb's type said they disagreed"
    );
    assert_eq!(on[0].assertions().len(), 3);
    assert_eq!(
        w.runtime
            .grounded(&AnchorKey::new("a"))
            .await
            .unwrap()
            .memories
            .len(),
        1,
    );
}

type Deadline = Arc<std::sync::Mutex<Vec<std::time::Instant>>>;

struct Deadlines {
    inner: Arc<Versioned>,
    seen: Deadline,
}

#[async_trait]
impl ContentProvider for Deadlines {
    fn provider(&self) -> &ProviderId {
        self.inner.provider()
    }

    async fn fetch(
        &self,
        id: &ExternalId,
        budget: &Budget,
    ) -> Result<Option<Fetched>, ContentError> {
        self.seen.lock().unwrap().push(budget.deadline());
        self.inner.fetch(id, budget).await
    }
}

struct Probing {
    kind: gmr_core::Kind,
    seen: Deadline,
}

#[async_trait]
impl gmr_probe::Transport for Probing {
    fn kind(&self) -> &gmr_core::Kind {
        &self.kind
    }

    fn resolve(&self, _name: &gmr_core::ProbeName) -> Option<gmr_core::Derivation> {
        Some(gmr_core::Derivation {
            observes: Default::default(),
            version: gmr_core::ProbeVersion::of(gmr_core::content_hash_of_bytes(b"probing")),
            verifiability: gmr_core::Verifiability::Closed,
        })
    }

    async fn invoke(
        &self,
        call: &gmr_probe::ProbeCall<'_>,
    ) -> Result<gmr_core::Outcome, gmr_probe::ProbeError> {
        self.seen.lock().unwrap().push(call.budget.deadline());
        Ok(gmr_core::Outcome::Found {
            facts: gmr_core::Facts::new(serde_json::json!({ "x": 1 })),
        })
    }
}

fn watched() -> (World, Deadline, Deadline) {
    let probed: Deadline = Default::default();
    let fetched: Deadline = Default::default();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("memories")).unwrap();
    std::fs::write(dir.path().join("world.json"), r#"{"x":1}"#).unwrap();
    let bindings = Arc::new(MemoryBindings::default());
    let runtime = Runtime::builder()
        .transport(Arc::new(Probing {
            kind: gmr_core::Kind::new("shell"),
            seen: probed.clone(),
        }))
        .provider(Arc::new(Deadlines {
            inner: Arc::new(Versioned::new(dir.path().to_path_buf(), true)),
            seen: fetched.clone(),
        }))
        .journal(Arc::new(MemoryJournal::default()))
        .bindings(bindings.clone())
        .sealer(bindings.clone())
        .links(bindings)
        .settings(Arc::new(MemoryQueue::default()))
        .sightings(Arc::new(MemoryQueue::default()))
        .build();
    (World { dir, runtime }, probed, fetched)
}

#[tokio::test]
async fn one_sentence_on_four_anchors_comes_back_with_four_warrants() {
    let w = World::new(true);
    for key in ["a", "b", "c", "d"] {
        w.open(key).await;
    }
    w.memory("m.md", "one sentence about four things");
    w.bind("m.md", &["a", "b", "c", "d"]).await;

    let refs = [gmr_core::Claim::from(Ref::new("git", "memories/m.md"))];
    let out = w
        .runtime
        .ground(&refs, &gmr_runtime::Instructions::default())
        .await
        .unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].on.len(),
        4,
        "each anchor carries its own observation state. A single warrant would have to be \
         relative to whichever anchor the caller happened to ask through, and a reference \
         keyed call has no such anchor"
    );
    assert!(
        out[0]
            .on
            .iter()
            .all(|a| matches!(a, gmr_runtime::Anchored::On { .. })),
        "{:?}",
        out[0].on
    );
    assert!(matches!(out[0].record, Some(Grounding::Current { .. })));
}

#[tokio::test]
async fn the_answers_come_back_in_the_order_they_were_asked_for() {
    let w = World::new(true);
    w.open("a").await;
    for name in ["z.md", "m.md", "a.md"] {
        w.memory(name, name);
        w.bind(name, &["a"]).await;
    }

    let refs: Vec<gmr_core::Claim> = ["z.md", "m.md", "a.md"]
        .iter()
        .map(|n| Ref::new("git", format!("memories/{n}")).into())
        .collect();
    let out = w
        .runtime
        .ground(&refs, &gmr_runtime::Instructions::default())
        .await
        .unwrap();

    assert_eq!(
        out.iter().map(|s| &s.claim).collect::<Vec<_>>(),
        refs.iter().collect::<Vec<_>>(),
        "a caller zips these against what it asked for. Reordering does not lose an answer, \
         it attributes one sentence's drift to another, silently, and worse the more \
         sentences there are"
    );
}

#[tokio::test]
async fn one_reference_nobody_can_answer_for_does_not_take_the_batch_with_it() {
    let w = World::new(true);
    w.open("a").await;
    w.memory("bound.md", "bound to a live anchor");
    w.bind("bound.md", &["a"]).await;
    w.memory("dangling.md", "bound to a key nothing opened");
    w.bind("dangling.md", &["never-opened"]).await;
    w.memory("loose.md", "bound to nothing at all");

    let refs: Vec<gmr_core::Claim> = ["bound.md", "dangling.md", "loose.md"]
        .iter()
        .map(|n| Ref::new("git", format!("memories/{n}")).into())
        .collect();
    let out = w
        .runtime
        .ground(&refs, &gmr_runtime::Instructions::default())
        .await
        .expect("a reference GMR cannot justify is an answer about that reference, not a fault");

    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].on[0], gmr_runtime::Anchored::On { .. }));
    assert!(
        matches!(out[1].on[0], gmr_runtime::Anchored::Unopened { .. }),
        "bound to a key nothing ever opened: go fix the binding, which is not what an empty \
         `on` tells you to do"
    );
    assert!(
        out[2].on.is_empty(),
        "nothing anchors this sentence, so nothing warrants it -- and that is an answer"
    );
    assert!(
        matches!(out[0].record, Some(Grounding::Current { .. }))
            && matches!(out[1].record, Some(Grounding::Current { .. })),
        "the record side is answered whatever the anchor side says -- the dangling one's text \
         is still exactly what was bound, and a broken binding does not change that"
    );
    assert!(
        matches!(out[2].record, Some(Grounding::Unverified { .. })),
        "never bound means no baseline to compare against, which is a different answer from \
         `unchanged` and must not be dressed up as one: {:?}",
        out[2].record
    );
}

#[tokio::test]
async fn evidence_carries_addresses_and_versions_and_no_values() {
    let w = World::new(true);
    w.open("a").await;
    w.memory("m.md", "a sentence");
    w.bind("m.md", &["a"]).await;

    let refs = [gmr_core::Claim::from(Ref::new("git", "memories/m.md"))];
    let out = w
        .runtime
        .ground(&refs, &gmr_runtime::Instructions::default())
        .await
        .unwrap();

    let gmr_runtime::Anchored::On { evidence, .. } = &out[0].on[0] else {
        panic!("expected an opened anchor")
    };
    assert!(evidence.reading.is_some(), "the address of what was read");
    assert!(evidence.instrument.is_some(), "which instrument read it");
    assert!(evidence.bound_at.is_some(), "when the sentence was bound");

    let json = serde_json::to_value(evidence).unwrap();
    let text = json.to_string();
    assert!(
        !text.contains("\"x\""),
        "evidence names what to go and check, never the value it would check. GMR neither \
         caches business data nor promises its freshness, so handing back the reading would \
         make it a bad database: {text}"
    );
}

#[tokio::test]
async fn both_phases_of_one_call_run_against_one_deadline() {
    let (w, probed, fetched) = watched();
    w.open("a").await;
    w.memory("m.md", "a sentence");
    w.bind("m.md", &["a"]).await;
    probed.lock().unwrap().clear();
    fetched.lock().unwrap().clear();

    let refs = [gmr_core::Claim::from(Ref::new("git", "memories/m.md"))];
    w.runtime
        .ground(
            &refs,
            &gmr_runtime::Instructions {
                max_staleness: Some(std::time::Duration::ZERO),
                budget: Some(std::time::Duration::from_millis(40)),
                reach: None,
            },
        )
        .await
        .unwrap();

    let probed = probed.lock().unwrap().clone();
    let fetched = fetched.lock().unwrap().clone();
    assert_eq!(probed.len(), 1, "the anchor was observed once");
    assert_eq!(fetched.len(), 1, "the record was fetched once");
    assert_eq!(
        probed[0], fetched[0],
        "a caller asking for 40ms means this call takes 40ms, not 40 for looking at the \
         world and 40 more for reading the sentence. Both phases descend from one budget \
         and are minted together, so a span shorter than either phase's own limit clamps \
         both to the same instant -- which is only observable if they were not started one \
         after the other"
    );
}

#[tokio::test]
async fn a_sentence_bound_to_the_reading_it_was_shown_says_which_one() {
    let w = World::new(true);
    w.open("a").await;

    let seen = w
        .runtime
        .sample(&AnchorKey::new("a"), &gmr_runtime::Instructions::default())
        .await
        .unwrap();
    let saw = seen
        .fact_address
        .clone()
        .expect("a sighting that found something has an address");
    assert_eq!(
        seen.facts.as_ref().map(gmr_core::Facts::as_value),
        Some(&serde_json::json!({"x": 1})),
        "`sample` is the delivery path: whatever it hands back is what the answer is built \
         from, and the address beside it is what that answer must cite"
    );

    let claim = gmr_core::Claim::Said {
        id: gmr_core::SaidId::new("turn-1"),
        asserts: Some(serde_json::json!({ "x": 1 })),
    };
    w.runtime
        .bind(
            gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]),
            None,
            std::collections::BTreeSet::from([saw.clone()]),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&claim),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].record, None,
        "an utterance is not stored anywhere, so there is no record to fetch and no \
         version to compare. Reporting a grounding here would be answering about a \
         document nobody wrote"
    );
    let gmr_runtime::Anchored::On { evidence, .. } = &out[0].on[0] else {
        panic!("{:?}", out[0].on)
    };
    assert_eq!(evidence.saw.iter().collect::<Vec<_>>(), vec![&saw]);
    assert!(
        evidence.shown.is_seen(),
        "the anchor's own journal holds an observation at that address -- the answer and \
         the anchor read the same thing, once: {:?}",
        evidence.shown
    );
}

#[tokio::test]
async fn a_sentence_citing_a_reading_this_anchor_never_took_is_not_grounded_by_it() {
    let w = World::new(true);
    w.open("a").await;
    w.runtime.observe(&AnchorKey::new("a")).await.unwrap();

    let elsewhere = gmr_core::FactAddress::try_new("b".repeat(64)).unwrap();
    let claim = gmr_core::Claim::said("turn-2");
    w.runtime
        .bind(
            gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]),
            None,
            std::collections::BTreeSet::from([elsewhere]),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&claim),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    let gmr_runtime::Anchored::On {
        warrant, evidence, ..
    } = &out[0].on[0]
    else {
        panic!("{:?}", out[0].on)
    };
    assert_eq!(
        evidence.shown,
        gmr_runtime::Shown::Unseen,
        "this is the whole defect the column exists to catch: a second computation of the \
         same fact, running beside the anchor instead of through it. The two agree until \
         they do not, and until now the answer still came back holding"
    );
    assert_eq!(
        warrant.holding,
        gmr_runtime::Holding::Holds,
        "and `Holding` still says holds, because it answers a different question -- has \
         what the anchor established moved. Folding `Unseen` into it would leave a reader \
         unable to tell a moved fact from an answer built somewhere else"
    );
}

#[tokio::test]
async fn a_sentence_that_cited_no_reading_is_not_reported_as_having_missed_one() {
    let w = World::new(true);
    w.open("a").await;
    w.memory("m.md", "written by hand, about the anchor");
    w.bind("m.md", &["a"]).await;

    let claim: gmr_core::Claim = Ref::new("git", "memories/m.md").into();
    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&claim),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    let gmr_runtime::Anchored::On { evidence, .. } = &out[0].on[0] else {
        panic!("{:?}", out[0].on)
    };
    assert_eq!(
        evidence.shown,
        gmr_runtime::Shown::NotSaid,
        "a note a person wrote never claimed to be built from a reading. Reporting it as \
         `Unseen` would make the corpus loud about the one thing it has nothing to say about"
    );
    assert!(matches!(out[0].record, Some(Grounding::Current { .. })));
}

#[tokio::test]
async fn a_list_that_moved_says_which_element_and_which_field() {
    let w = World::new(true);
    std::fs::write(
        w.dir.path().join("world.json"),
        r#"{"x":[{"name":"a","price":100},{"name":"b","price":200}]}"#,
    )
    .unwrap();
    w.open("a").await;

    let claim = gmr_core::Claim::said("turn-3");
    let saw = w
        .runtime
        .sample(&AnchorKey::new("a"), &gmr_runtime::Instructions::default())
        .await
        .unwrap()
        .fact_address
        .unwrap();
    w.runtime
        .bind(
            gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]),
            None,
            std::collections::BTreeSet::from([saw]),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    std::fs::write(
        w.dir.path().join("world.json"),
        r#"{"x":[{"name":"a","price":100},{"name":"b","price":690}]}"#,
    )
    .unwrap();
    w.runtime.observe(&AnchorKey::new("a")).await.unwrap();

    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&claim),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    let gmr_runtime::Anchored::On { warrant, .. } = &out[0].on[0] else {
        panic!("{:?}", out[0].on)
    };
    let gmr_runtime::Holding::Moved { axes, .. } = &warrant.holding else {
        panic!("{:?}", warrant.holding)
    };
    assert_eq!(
        axes,
        &vec!["x.1.price".to_owned()],
        "a reading that is a list of things -- a menu, a roster, a price table -- used to \
         report as one axis called `x`, because the diff walked objects and stopped at \
         arrays. `Moved` then said the whole list moved and could not say which row, which \
         is the difference between a claim about one dish being expired and every claim \
         about that menu being expired at once"
    );
}

async fn depending(w: &World, name: &str, anchors: &[&str], source: &str) -> gmr_core::Claim {
    let claim = gmr_core::Claim::said(name);
    w.runtime
        .bind(
            gmr_core::Binding::on(
                claim.clone(),
                anchors.iter().map(|a| AnchorKey::new(*a)).collect(),
            )
            .depending(source),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();
    claim
}

async fn stands(w: &World, claim: &gmr_core::Claim) -> gmr_runtime::Depends {
    let out = w
        .runtime
        .ground(
            std::slice::from_ref(claim),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    out[0].depends.clone()
}

#[tokio::test]
async fn an_invariant_reads_every_anchor_the_claim_names_as_one_question() {
    let w = World::new(true);
    w.open("a").await;
    w.open("b").await;

    let claim = depending(&w, "turn-4", &["a", "b"], "all(anchors, state.x == 1)").await;
    assert_eq!(
        stands(&w, &claim).await,
        gmr_runtime::Depends::Holds,
        "both anchors read the same world file, so both are at x == 1"
    );

    let broken = depending(&w, "turn-5", &["a", "b"], "any(anchors, state.x == 99)").await;
    assert_eq!(
        stands(&w, &broken).await,
        gmr_runtime::Depends::Broken,
        "`depends` is an invariant, so false means the claim no longer stands -- the \
         opposite polarity from a subscription, which fires when something moved"
    );
}

#[tokio::test]
async fn an_invariant_breaks_when_the_thing_it_named_moves() {
    let w = World::new(true);
    w.open("a").await;
    let claim = depending(&w, "turn-6", &["a"], "all(anchors, state.x == 1)").await;
    assert_eq!(stands(&w, &claim).await, gmr_runtime::Depends::Holds);

    std::fs::write(w.dir.path().join("world.json"), r#"{"x":2}"#).unwrap();
    w.runtime.observe(&AnchorKey::new("a")).await.unwrap();
    assert_eq!(
        stands(&w, &claim).await,
        gmr_runtime::Depends::Broken,
        "the claim said what it rested on, and that stopped being true. `Holding` says the \
         state moved; this says the move was one the claim could not survive"
    );
}

#[tokio::test]
async fn a_claim_that_stated_no_invariant_is_not_reported_as_keeping_one() {
    let w = World::new(true);
    w.open("a").await;
    let claim = gmr_core::Claim::said("turn-7");
    w.runtime
        .bind(
            gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();
    assert_eq!(
        stands(&w, &claim).await,
        gmr_runtime::Depends::Unstated,
        "`Holds` over an invariant nobody wrote is a green light earned by saying nothing, \
         which is the one answer a reader must never get from this field"
    );
}

#[tokio::test]
async fn an_invariant_that_cannot_be_settled_says_so_rather_than_picking_a_side() {
    let w = World::new(true);
    w.open("a").await;

    let nonsense = depending(&w, "turn-8", &["a"], "all(anchors, state.x)").await;
    assert!(
        matches!(
            stands(&w, &nonsense).await,
            gmr_runtime::Depends::Unevaluable { .. }
        ),
        "a body that answers with a number is not a yes or a no, and reading it as either \
         would put a claim in a bucket its author never asked for"
    );

    let claim = depending(&w, "turn-9", &["a"], "all(anchors, state.x == 1)").await;
    assert_eq!(stands(&w, &claim).await, gmr_runtime::Depends::Holds);
}

#[tokio::test]
async fn rebinding_with_a_different_invariant_is_a_new_assertion() {
    let w = World::new(true);
    w.open("a").await;
    let claim = gmr_core::Claim::said("turn-10");

    async fn assert_with(w: &World, claim: &gmr_core::Claim, source: &str) -> bool {
        w.runtime
            .bind(
                gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]).depending(source),
                None,
                Default::default(),
                gmr_core::Source::SelfAttested,
            )
            .await
            .unwrap()
            .recorded
    }

    assert!(assert_with(&w, &claim, "all(anchors, state.x == 1)").await);
    assert!(
        !assert_with(&w, &claim, "all(anchors, state.x == 1)").await,
        "same claim, same invariant"
    );
    assert!(
        assert_with(&w, &claim, "all(anchors, state.x == 2)").await,
        "what a claim rests on is part of what it says. Treating a changed invariant as a \
         repeat would leave the table holding one sentence and the reader reading another"
    );
}

async fn linked(w: &World, from: &str, to: &str, kind: &str) {
    w.runtime
        .link(
            &Ref::new("git", format!("memories/{from}")),
            &Ref::new("git", format!("memories/{to}")),
            gmr_core::LinkKind(kind.to_owned()),
        )
        .await
        .unwrap();
}

fn reaching(depth: usize) -> gmr_runtime::Instructions {
    gmr_runtime::Instructions {
        reach: Some(depth),
        ..Default::default()
    }
}

async fn reached(
    w: &World,
    name: &str,
    how: &gmr_runtime::Instructions,
) -> Vec<gmr_runtime::Reached> {
    let claim: gmr_core::Claim = Ref::new("git", format!("memories/{name}")).into();
    let out = w
        .runtime
        .ground(std::slice::from_ref(&claim), how)
        .await
        .unwrap();
    out[0].reached.clone()
}

#[tokio::test]
async fn what_a_memory_rests_on_is_followed_and_a_broken_link_comes_back_with_its_path() {
    let w = World::new(true);
    w.open("a").await;
    for name in ["a.md", "b.md", "c.md"] {
        w.memory(name, name);
        w.bind(name, &["a"]).await;
    }
    linked(&w, "a.md", "b.md", "cites").await;
    linked(&w, "b.md", "c.md", "cites").await;

    assert!(
        reached(&w, "a.md", &reaching(3)).await.is_empty(),
        "nothing down the chain has moved, so there is nothing to report. A walk that \
         listed every record it touched would bury the one that matters"
    );

    w.memory("c.md", "rewritten two hops away");
    let found = reached(&w, "a.md", &reaching(3)).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].reference, Ref::new("git", "memories/c.md"));
    assert_eq!(found[0].depth, 2);
    assert_eq!(
        found[0].via,
        vec![
            gmr_core::LinkKind("cites".into()),
            gmr_core::LinkKind("cites".into())
        ],
        "the path is what makes the report actionable: a reader has to see which of this \
         memory's own citations led to the thing that moved, not merely that something did"
    );
    assert_eq!(found[0].footing, gmr_runtime::Footing::Rewritten);
}

#[tokio::test]
async fn following_links_is_asked_for_and_bounded_by_the_caller() {
    let w = World::new(true);
    w.open("a").await;
    for name in ["a.md", "b.md", "c.md"] {
        w.memory(name, name);
        w.bind(name, &["a"]).await;
    }
    linked(&w, "a.md", "b.md", "cites").await;
    linked(&w, "b.md", "c.md", "cites").await;
    w.memory("c.md", "rewritten two hops away");

    assert!(
        reached(&w, "a.md", &gmr_runtime::Instructions::default())
            .await
            .is_empty(),
        "a walk nobody asked for is store reads nobody budgeted, on every call. The \
         caller says how far, and silence says not at all"
    );
    assert!(
        reached(&w, "a.md", &reaching(1)).await.is_empty(),
        "one hop reaches b.md, which has not moved. The depth is a bound on the walk and \
         not a hint"
    );
    assert_eq!(reached(&w, "a.md", &reaching(2)).await.len(), 1);
}

#[tokio::test]
async fn a_cycle_in_the_links_is_walked_once_and_not_forever() {
    let w = World::new(true);
    w.open("a").await;
    for name in ["a.md", "b.md"] {
        w.memory(name, name);
        w.bind(name, &["a"]).await;
    }
    linked(&w, "a.md", "b.md", "cites").await;
    linked(&w, "b.md", "a.md", "cites").await;
    w.memory("b.md", "rewritten, and it links back");

    let found = reached(&w, "a.md", &reaching(8)).await;
    assert_eq!(
        found.len(),
        1,
        "two memories citing each other is the ordinary shape of a corpus, not a defect. \
         The walk visits each record once; without that this call does not return"
    );
    assert_eq!(found[0].reference, Ref::new("git", "memories/b.md"));
}

#[tokio::test]
async fn an_utterance_reaches_nothing_because_links_run_between_records() {
    let w = World::new(true);
    w.open("a").await;
    let claim = gmr_core::Claim::said("turn-11");
    w.runtime
        .bind(
            gmr_core::Binding::on(claim.clone(), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();
    let out = w
        .runtime
        .ground(std::slice::from_ref(&claim), &reaching(3))
        .await
        .unwrap();
    assert!(
        out[0].reached.is_empty(),
        "a sentence an agent said is stored nowhere, so nothing links to or from it. What \
         it rests on is its anchors and its `depends`, not a citation graph"
    );
}

#[tokio::test]
async fn an_utterance_on_an_anchor_does_not_take_down_the_verb_that_walks_records() {
    let w = World::new(true);
    w.open("a").await;
    w.memory("m.md", "a note, stored");
    w.bind("m.md", &["a"]).await;
    w.runtime
        .bind(
            gmr_core::Binding::on(gmr_core::Claim::said("turn-12"), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let edges = w.runtime.changed_since(0, None).await.unwrap();
    assert!(
        edges.raised.is_some(),
        "`since` walks every binding on every anchor looking for records that moved. An \
         utterance is bound like anything else and is stored nowhere, so the walk met one \
         and asked it for a record -- which panicked the worker thread, through the SDK, \
         for any product that binds what its agent said"
    );

    let held = w.runtime.grounded(&AnchorKey::new("a")).await.unwrap();
    assert_eq!(
        held.memories.len(),
        1,
        "and the record-shaped verbs still answer about the one record there is"
    );
    assert_eq!(held.memories[0].reference, Ref::new("git", "memories/m.md"));
}

#[tokio::test]
async fn a_claim_on_several_anchors_looked_at_several_readings() {
    let w = World::new(true);
    w.open("a").await;
    w.open("b").await;

    let how = gmr_runtime::Instructions::default();
    let mut saw = Vec::new();
    for key in ["a", "b"] {
        saw.push(
            w.runtime
                .sample(&AnchorKey::new(key), &how)
                .await
                .unwrap()
                .fact_address
                .unwrap(),
        );
    }

    let claim = gmr_core::Claim::said("turn-13");
    w.runtime
        .bind(
            gmr_core::Binding::on(
                claim.clone(),
                vec![AnchorKey::new("a"), AnchorKey::new("b")],
            ),
            None,
            saw.iter().cloned().collect(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let out = w
        .runtime
        .ground(std::slice::from_ref(&claim), &how)
        .await
        .unwrap();
    for anchored in &out[0].on {
        let gmr_runtime::Anchored::On { key, evidence, .. } = anchored else {
            panic!("{anchored:?}")
        };
        assert!(
            evidence.shown.is_seen(),
            "an asserter reading four anchors looked at four readings, and `saw` held one \
             address. Whichever anchor it belonged to reported `seen` and every other one \
             reported `unseen` -- the shape of a delivery-path defect, invented by the \
             record itself. `{key}` said {:?}",
            evidence.shown
        );
    }
}

#[tokio::test]
async fn what_a_claim_asserted_comes_back_with_it() {
    let w = World::new(true);
    w.open("a").await;
    let stored = gmr_core::Claim::Said {
        id: gmr_core::SaidId::new("turn-14"),
        asserts: Some(serde_json::json!({ "text": "the roster is complete" })),
    };
    w.runtime
        .bind(
            gmr_core::Binding::on(stored.clone(), vec![AnchorKey::new("a")]),
            None,
            Default::default(),
            gmr_core::Source::SelfAttested,
        )
        .await
        .unwrap();

    let asked = gmr_core::Claim::said("turn-14");
    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&asked),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        out[0].claim, stored,
        "a caller looks a claim up by its id and gets back what it says, not the id it \
         handed in. Echoing the question leaves an auditor holding a verdict about a \
         sentence nothing will show them"
    );

    let unbound = gmr_core::Claim::said("never-said");
    let out = w
        .runtime
        .ground(
            std::slice::from_ref(&unbound),
            &gmr_runtime::Instructions::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        out[0].claim, unbound,
        "and one nothing was ever asserted about comes back as asked, because there is no \
         stored version of it to prefer"
    );
}
