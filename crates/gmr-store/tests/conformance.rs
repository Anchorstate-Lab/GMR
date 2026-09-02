use gmr_core::{
    Anchor, AnchorKey, Binding, Entry, Expr, FactAddress, Facts, Kind, Observation, Outcome,
    ProbeName, ProbeRef, ProbeVersion, ReasonClass, Ref, Rule, State, Transitions, Version,
    Versions, fold,
};
use gmr_store::{Asserted, BindingStore, ErrorCode, ErrorKind, Expected, Fence, Journal, Sealer};

fn at(n: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
}

fn probe() -> ProbeRef {
    ProbeRef::new(
        Kind::new("shell"),
        ProbeName::new("p"),
        serde_json::json!({}),
    )
}

fn transitions(names: &[&str]) -> Transitions {
    Transitions(
        names
            .iter()
            .map(|n| Rule {
                when: Expr::text(format!("changed(\"{n}\")")),
                to: Expr::text(format!("{{ {n}: obs.{n} }}")),
            })
            .collect(),
    )
}

fn anchor(key: &str, names: &[&str]) -> Anchor {
    Anchor {
        key: AnchorKey::new(key),
        probe: probe(),
        transitions: transitions(names),
        terminal: Default::default(),
        supersedes: None,
    }
}

fn observation(pairs: &[(&str, serde_json::Value)]) -> Observation {
    Observation {
        outcome: Outcome::Found {
            facts: Facts::new(
                pairs
                    .iter()
                    .map(|(n, v)| ((*n).to_owned(), v.clone()))
                    .collect::<serde_json::Map<_, _>>()
                    .into(),
            ),
        },
        fact_address: FactAddress::try_new("b".repeat(64)).unwrap(),
        versions: Versions {
            declaration: gmr_core::ContentHash::try_new("d".repeat(64)).unwrap(),
            derivation: gmr_core::Derivation {
                observes: Default::default(),
                version: ProbeVersion::try_new("a".repeat(64)).unwrap(),
                verifiability: gmr_core::Verifiability::Closed,
            },
            evaluator: "eval-1".to_owned(),
        },
    }
}

fn state_of(names: &[&str], v: &serde_json::Value) -> State {
    State::new(
        names
            .iter()
            .map(|n| ((*n).to_owned(), v.clone()))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    )
}

fn open_entry(key: &str, names: &[&str], v: serde_json::Value) -> Entry {
    Entry::Open {
        anchor: Box::new(anchor(key, names)),
        observation: observation(&names.iter().map(|n| (*n, v.clone())).collect::<Vec<_>>()),
        state: state_of(names, &v),
        at: at(0),
    }
}

async fn journal_hands_back_what_it_was_given<J: Journal>(j: &J) {
    let key = AnchorKey::new("core::modules");
    let entry = open_entry("core::modules", &["count"], serde_json::json!(5));

    let seq = j
        .append(&key, &entry, Fence::Unleased, Expected::Any)
        .await
        .unwrap();
    let back = j.entries(&key, 0).await.unwrap();

    assert_eq!(back.len(), 1);
    assert_eq!(back[0].0, seq);
    assert_eq!(
        back[0].1, entry,
        "stored and loaded entries must be identical"
    );
}

async fn journal_is_ordered_and_scoped_per_anchor<J: Journal>(j: &J) {
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");

    j.append(
        &a,
        &open_entry("a", &["x"], serde_json::json!(1)),
        Fence::Unleased,
        Expected::Any,
    )
    .await
    .unwrap();
    j.append(
        &b,
        &open_entry("b", &["x"], serde_json::json!(2)),
        Fence::Unleased,
        Expected::Any,
    )
    .await
    .unwrap();
    j.append(
        &a,
        &Entry::Attempt {
            reason: ReasonClass::Unreachable,
            code: None,
            message: "boom".into(),
            at: at(10),
        },
        Fence::Unleased,
        Expected::Any,
    )
    .await
    .unwrap();

    let entries = j.entries(&a, 0).await.unwrap();
    assert_eq!(entries.len(), 2, "b entries must not appear in a's journal");
    assert!(entries[0].0 < entries[1].0, "seq must be monotonic");

    let mut anchors = j.anchors().await.unwrap();
    anchors.sort();
    assert_eq!(anchors, vec![a, b]);
}

async fn journal_refuses_an_entry_folded_against_a_head_that_moved<J: Journal>(j: &J) {
    let key = AnchorKey::new("contended");
    let other = AnchorKey::new("elsewhere");
    let entry = open_entry("contended", &["x"], serde_json::json!(1));

    let first = j
        .append(&key, &entry, Fence::Unleased, Expected::Head(0))
        .await
        .unwrap();

    let err = j
        .append(&key, &entry, Fence::Unleased, Expected::Head(0))
        .await
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Constraint);
    assert_eq!(err.code, ErrorCode::HeadMoved);
    assert!(
        !err.kind.is_retryable(),
        "the entry has to be recomputed against the head that is actually there; \
         re-sending the same bytes cannot help"
    );

    let second = j
        .append(&key, &entry, Fence::Unleased, Expected::Head(first))
        .await
        .unwrap();

    j.append(&other, &entry, Fence::Unleased, Expected::Any)
        .await
        .unwrap();
    j.append(&key, &entry, Fence::Unleased, Expected::Head(second))
        .await
        .expect(
            "seq is global but a head is this anchor's own — an append elsewhere must not \
             invalidate work folded against this anchor, or every anchor would conflict with \
             every other one",
        );

    j.append(&key, &entry, Fence::Unleased, Expected::Any)
        .await
        .expect("an entry decided by nothing it read carries no expectation to break");

    assert_eq!(j.entries(&key, 0).await.unwrap().len(), 4);
}

async fn a_stored_log_folds_back_into_state<J: Journal>(j: &J) {
    let key = AnchorKey::new("folded");
    j.append(
        &key,
        &open_entry("folded", &["shape"], serde_json::json!("old")),
        Fence::Unleased,
        Expected::Any,
    )
    .await
    .unwrap();
    j.append(
        &key,
        &Entry::Transition {
            observation: observation(&[("shape", serde_json::json!("new"))]),
            state: state_of(&["shape"], &serde_json::json!("new")),
            at: at(10),
        },
        Fence::Unleased,
        Expected::Any,
    )
    .await
    .unwrap();

    let state = fold(&j.entries(&key, 0).await.unwrap()).unwrap();
    assert_eq!(state.state.as_value()["shape"], serde_json::json!("new"));
    assert_eq!(state.attempts(), 0);
}

fn asserted(binding: &Binding, version: &str, bound_at_seq: Option<gmr_core::Seq>) -> Asserted {
    Asserted {
        binding: binding.clone(),
        bound_version: Some(Version::new(version)),
        bound_at_seq,
        saw: Default::default(),
        source: gmr_core::Source::Adjudicated,
        at: chrono::Utc::now(),
    }
}

async fn bindings_record_the_version_they_bound<B: BindingStore>(b: &B) {
    let binding = Binding::on(
        Ref::new("git", "memories/core-modules.md"),
        vec![AnchorKey::new("core::modules")],
    );
    let bound_version = Version::new("blob-v1");
    b.bind(&asserted(&binding, bound_version.as_str(), Some(7)))
        .await
        .unwrap();

    let on = b
        .bindings_on(&[AnchorKey::new("core::modules")])
        .await
        .unwrap();
    assert_eq!(on.len(), 1);
    assert_eq!(on[0].binding, binding);
    assert_eq!(on[0].bound_version, Some(bound_version));
    assert_eq!(on[0].bound_at_seq, Some(7));
    assert_eq!(on[0].source, gmr_core::Source::Adjudicated);

    assert_eq!(
        b.binding_of(&binding.claim).await.unwrap().len(),
        1,
        "the reverse direction answers about the same one assertion"
    );
}

async fn what_the_asserter_was_looking_at_is_kept_beside_the_assertion<B: BindingStore>(b: &B) {
    let saw = gmr_core::FactAddress::try_new("a".repeat(64)).unwrap();
    let claim = gmr_core::Claim::Said {
        id: gmr_core::SaidId::new("turn-7"),
        asserts: Some(serde_json::json!({ "price_cents": 420 })),
    };
    let binding = Binding::on(claim.clone(), vec![AnchorKey::new("dish::icejelly")]);
    b.bind(&Asserted {
        binding,
        bound_version: None,
        bound_at_seq: Some(3),
        saw: std::collections::BTreeSet::from([saw.clone()]),
        source: gmr_core::Source::SelfAttested,
        at: chrono::Utc::now(),
    })
    .await
    .unwrap();

    let found = b.binding_of(&claim).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].saw.iter().collect::<Vec<_>>(),
        vec![&saw],
        "an assertion that cites no reading and one that cites a reading nobody took are \
         the same row without this column, and only the second is a defect"
    );
    assert_eq!(found[0].binding.claim, claim);
    assert_eq!(
        b.binding_of(&gmr_core::Claim::said("turn-7"))
            .await
            .unwrap()
            .len(),
        1,
        "one utterance is one claim: what it asserts rides along, it does not file a \
         separate row"
    );
}

async fn a_binding_stamped_with_no_seq_reads_back_as_none<B: BindingStore>(b: &B) {
    let binding = Binding::on(
        Ref::new("git", "memories/many.md"),
        vec![AnchorKey::new("a"), AnchorKey::new("b")],
    );
    b.bind(&asserted(&binding, "v", None)).await.unwrap();

    assert_eq!(
        b.binding_of(&binding.claim).await.unwrap()[0].bound_at_seq,
        None,
        "every row written before this column existed has no seq and never will -- the \
         table is append-only. A store that invented one would date a binding to a moment \
         nobody recorded, and `Holding` would report a move that may have happened before \
         it was ever bound"
    );
}

async fn asserting_a_second_anchor_does_not_take_the_first_away<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/moved.md");
    for anchor in ["from", "to"] {
        b.bind(&asserted(
            &Binding::on(reference.clone(), vec![AnchorKey::new(anchor)]),
            "v",
            None,
        ))
        .await
        .unwrap();
    }

    assert_eq!(
        b.bindings_on(&[AnchorKey::new("from")])
            .await
            .unwrap()
            .len(),
        1,
        "an assertion is an add, not a replacement. Under latest-wins this record silently \
         left `from` the moment something asserted it on `to` — which is how an agent \
         binding what it just wrote erased what a person had put there. Delivering one \
         anchor too many is a reader's judgement to make; delivering one too few is a \
         memory nobody is told about"
    );
    assert_eq!(
        b.bindings_on(&[AnchorKey::new("to")]).await.unwrap().len(),
        1
    );
}

async fn asserting_the_same_anchor_twice_still_delivers_it_once<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/twice.md");
    for v in ["v1", "v2"] {
        b.bind(&asserted(
            &Binding::on(reference.clone(), vec![AnchorKey::new("same")]),
            v,
            None,
        ))
        .await
        .unwrap();
    }

    let on = b.bindings_on(&[AnchorKey::new("same")]).await.unwrap();
    assert_eq!(
        on.iter()
            .flat_map(|r| r.binding.anchors.iter())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1,
        "the set is a set of anchors, so an agent re-asserting the same coordinate every \
         session leaves the delivered set exactly where it was"
    );
}

async fn a_revocation_kills_only_the_tags_it_named<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/orset.md");
    let at = AnchorKey::new("g");
    b.bind(&asserted(
        &Binding::on(reference.clone(), vec![at.clone()]),
        "v1",
        None,
    ))
    .await
    .unwrap();

    let first = b.bindings_on(std::slice::from_ref(&at)).await.unwrap();
    b.revoke(&gmr_store::Revocation {
        claim: reference.clone().into(),
        at: at.clone(),
        tags: vec![gmr_store::Tag {
            binding: first[0].seq,
            anchor: at.clone(),
        }],
        source: gmr_core::Source::Adjudicated,
        when: chrono::Utc::now(),
    })
    .await
    .unwrap();
    assert!(
        b.bindings_on(std::slice::from_ref(&at))
            .await
            .unwrap()
            .is_empty(),
        "a revocation that names a tag has to actually take it out of the delivered set"
    );

    b.bind(&asserted(
        &Binding::on(reference.clone(), vec![at.clone()]),
        "v2",
        None,
    ))
    .await
    .unwrap();
    assert_eq!(
        b.bindings_on(std::slice::from_ref(&at))
            .await
            .unwrap()
            .len(),
        1,
        "the later assertion is a tag the revocation never observed, so it wins. Without \
         this, a revocation is a permanent ban on a coordinate rather than a claim about \
         particular assertions — and an agent that re-derives a link the criteria now \
         support could never say so again"
    );
}

async fn a_revocation_does_not_reach_a_generation_it_was_not_made_at<B: BindingStore>(b: &B) {
    let reference = Ref::new("git", "memories/generations.md");
    let older = AnchorKey::new("older");
    let heir = AnchorKey::new("heir");
    b.bind(&asserted(
        &Binding::on(reference.clone(), vec![older.clone()]),
        "v1",
        None,
    ))
    .await
    .unwrap();
    let seq = b.bindings_on(std::slice::from_ref(&older)).await.unwrap()[0].seq;

    b.revoke(&gmr_store::Revocation {
        claim: reference.clone().into(),
        at: heir.clone(),
        tags: vec![gmr_store::Tag {
            binding: seq,
            anchor: older.clone(),
        }],
        source: gmr_core::Source::Adjudicated,
        when: chrono::Utc::now(),
    })
    .await
    .unwrap();

    assert!(
        b.bindings_on(&[heir.clone(), older.clone()])
            .await
            .unwrap()
            .is_empty(),
        "read from the heir, whose chain contains the generation this revocation was made \
         at, it applies"
    );
    assert_eq!(
        b.bindings_on(std::slice::from_ref(&older))
            .await
            .unwrap()
            .len(),
        1,
        "read from the older generation alone, it does not. The assertion was correct for \
         the criteria that stood there, and a revocation made under later criteria is not \
         a statement about it. Filtering revocations by the chain being read is what makes \
         that hold without anyone remembering to check"
    );
}

async fn sealing_is_content_addressed_and_idempotent<S: Sealer>(b: &S) {
    let bytes = "I accepted the move, but not the signature change".as_bytes();
    let a1 = b.seal(bytes).await.unwrap();
    let a2 = b.seal(bytes).await.unwrap();
    assert_eq!(
        a1, a2,
        "same bytes have the same address; repeated sealing is idempotent"
    );
    assert_eq!(b.sealed(&a1).await.unwrap().as_deref(), Some(bytes));
    assert_ne!(a1, b.seal("another rationale".as_bytes()).await.unwrap());
}

async fn links_are_scoped_to_the_from_reference<L: gmr_store::LinkStore>(l: &L) {
    let a = Ref::new("git", "memories/a.md");
    let b = Ref::new("git", "memories/b.md");
    let c = Ref::new("git", "memories/c.md");

    l.link(
        &a,
        &b,
        gmr_core::LinkKind("elaborates".into()),
        gmr_core::Source::Adjudicated,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    l.link(
        &a,
        &c,
        gmr_core::LinkKind("contradicts".into()),
        gmr_core::Source::Derived,
        chrono::Utc::now(),
    )
    .await
    .unwrap();

    let from_a = l.links_of(&a).await.unwrap();
    assert_eq!(from_a.len(), 2);
    assert!(from_a.iter().any(|link| link.to == b));
    assert!(from_a.iter().any(|link| link.to == c));

    assert!(
        l.links_of(&b).await.unwrap().is_empty(),
        "links are directed: b was linked to, not from"
    );

    let into_b = l.links_to(&b).await.unwrap();
    assert_eq!(
        into_b.len(),
        1,
        "the pointed-at end can discover who points at it"
    );
    assert_eq!(into_b[0].0, a, "the far end names the asserter of the edge");
    assert!(
        into_b[0].1.at.is_some(),
        "when the edge grew is readable, not lost"
    );
    assert!(l.links_to(&a).await.unwrap().is_empty());
}

async fn an_edge_carries_who_asserted_it<L: gmr_store::LinkStore>(l: &L) {
    let a = Ref::new("git", "memories/a.md");
    let b = Ref::new("git", "memories/b.md");
    l.link(
        &a,
        &b,
        gmr_core::LinkKind("rests-on".into()),
        gmr_core::Source::Derived,
        chrono::Utc::now(),
    )
    .await
    .unwrap();

    let held = l.links_of(&a).await.unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(
        held[0].source,
        gmr_core::Source::Derived,
        "a reader deciding whether to trust an edge needs who said it, the same \
         question independent() answers for a binding"
    );
}

async fn unlinking_names_only_the_rows_it_observed<L: gmr_store::LinkStore>(l: &L) {
    let a = Ref::new("git", "memories/a.md");
    let b = Ref::new("git", "memories/b.md");
    let kind = gmr_core::LinkKind("rests-on".into());
    l.link(
        &a,
        &b,
        kind.clone(),
        gmr_core::Source::Derived,
        chrono::Utc::now(),
    )
    .await
    .unwrap();

    let revoked = l
        .unlink(&gmr_store::LinkRevocation {
            from: a.clone(),
            to: b.clone(),
            kind: kind.clone(),
            asserted_as: Some(gmr_core::Source::Derived),
            source: gmr_core::Source::Derived,
            when: chrono::Utc::now(),
        })
        .await
        .unwrap();
    assert_eq!(revoked, 1);
    assert!(l.links_of(&a).await.unwrap().is_empty());

    l.link(
        &a,
        &b,
        kind.clone(),
        gmr_core::Source::SelfAttested,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(
        l.links_of(&a).await.unwrap().len(),
        1,
        "a revocation kills the rows it observed, never the edge as an idea: \
         a later assertion of the same edge is a new row and stands"
    );
}

async fn unlinking_derived_rows_leaves_an_agents_identical_edge_standing<
    L: gmr_store::LinkStore,
>(
    l: &L,
) {
    let a = Ref::new("git", "memories/a.md");
    let b = Ref::new("git", "memories/b.md");
    let kind = gmr_core::LinkKind("rests-on".into());
    l.link(
        &a,
        &b,
        kind.clone(),
        gmr_core::Source::Derived,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    l.link(
        &a,
        &b,
        kind.clone(),
        gmr_core::Source::SelfAttested,
        chrono::Utc::now(),
    )
    .await
    .unwrap();

    let revoked = l
        .unlink(&gmr_store::LinkRevocation {
            from: a.clone(),
            to: b.clone(),
            kind: kind.clone(),
            asserted_as: Some(gmr_core::Source::Derived),
            source: gmr_core::Source::Derived,
            when: chrono::Utc::now(),
        })
        .await
        .unwrap();
    assert_eq!(revoked, 1);

    let held = l.links_of(&a).await.unwrap();
    assert_eq!(
        held.len(),
        1,
        "declaration reconciliation owns only what declarations wrote; an agent \
         vouching for the same edge is a separate assertion it may not touch"
    );
    assert_eq!(held[0].source, gmr_core::Source::SelfAttested);
}

macro_rules! journal_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_journal {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryJournal::default()).await;
            })*
        }
        mod sqlite_journal {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.journal()).await;
            })*
        }
    };
}

macro_rules! bindings_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_bindings {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryBindings::default()).await;
            })*
        }
        mod sqlite_bindings {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.bindings()).await;
            })*
        }
    };
}

macro_rules! links_conformance {
    ($($name:ident),* $(,)?) => {
        mod memory_links {
            $(#[tokio::test] async fn $name() {
                super::$name(&gmr_store::testkit::MemoryBindings::default()).await;
            })*
        }
        mod sqlite_links {
            $(#[tokio::test] async fn $name() {
                let store = gmr_store::sqlite::open_in_memory().await.unwrap();
                super::$name(&store.links()).await;
            })*
        }
    };
}

journal_conformance!(
    journal_hands_back_what_it_was_given,
    journal_is_ordered_and_scoped_per_anchor,
    journal_refuses_an_entry_folded_against_a_head_that_moved,
    a_stored_log_folds_back_into_state,
);

bindings_conformance!(
    bindings_record_the_version_they_bound,
    what_the_asserter_was_looking_at_is_kept_beside_the_assertion,
    a_binding_stamped_with_no_seq_reads_back_as_none,
    asserting_a_second_anchor_does_not_take_the_first_away,
    asserting_the_same_anchor_twice_still_delivers_it_once,
    a_revocation_kills_only_the_tags_it_named,
    a_revocation_does_not_reach_a_generation_it_was_not_made_at,
    sealing_is_content_addressed_and_idempotent,
);

links_conformance!(
    links_are_scoped_to_the_from_reference,
    an_edge_carries_who_asserted_it,
    unlinking_names_only_the_rows_it_observed,
    unlinking_derived_rows_leaves_an_agents_identical_edge_standing,
);

async fn queue_contract<Q: gmr_store::Queue>(q: &Q) {
    use chrono::{Duration, TimeZone, Utc};
    use gmr_store::{Disposition, Ticket};

    let t0 = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let a = AnchorKey::new("a");
    let b = AnchorKey::new("b");
    q.enqueue(&a, t0).await.unwrap();
    q.enqueue(&b, t0 + Duration::seconds(100)).await.unwrap();

    let due = q.due(t0, Duration::seconds(60), 10).await.unwrap();
    assert_eq!(due.len(), 1, "b is not due yet");
    assert_eq!(due[0].anchor, a);
    assert_eq!(
        due[0].fence.epoch(),
        Some(1),
        "each lease increments the epoch, starting at 1"
    );

    let again = q
        .due(t0 + Duration::seconds(10), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert!(
        again.is_empty(),
        "an unexpired lease must not issue the same anchor twice"
    );

    let expired = q
        .due(t0 + Duration::seconds(61), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert_eq!(expired.len(), 1, "an expired lease can be leased again");
    assert_eq!(
        expired[0].fence.epoch(),
        Some(2),
        "epoch keeps increasing so old holders are fenced out of the journal"
    );

    q.settle(
        &expired[0],
        Disposition::Backoff { after_secs: 30 },
        t0 + Duration::seconds(62),
    )
    .await
    .unwrap();
    assert!(
        q.due(t0 + Duration::seconds(80), Duration::seconds(60), 10)
            .await
            .unwrap()
            .is_empty(),
        "backing off"
    );

    let back = q
        .due(t0 + Duration::seconds(100), Duration::seconds(60), 10)
        .await
        .unwrap();
    let names: Vec<&str> = back.iter().map(|t| t.anchor.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);

    for t in &back {
        q.settle(t, Disposition::Retire, t0 + Duration::seconds(101))
            .await
            .unwrap();
    }
    assert!(
        q.due(t0 + Duration::seconds(999_999), Duration::seconds(60), 10)
            .await
            .unwrap()
            .is_empty(),
        "retired anchors should not be dequeued"
    );

    q.enqueue(&a, t0 + Duration::seconds(1_000)).await.unwrap();
    let reborn = q
        .due(t0 + Duration::seconds(1_000), Duration::seconds(60), 10)
        .await
        .unwrap();
    assert_eq!(reborn.len(), 1);
    assert!(
        reborn[0].fence.epoch().unwrap() > 2,
        "deleting the counter on retire resets the token: got {}, while the journal has already seen 2",
        reborn[0].fence.epoch().unwrap()
    );

    let ghost = Ticket {
        anchor: AnchorKey::new("ghost"),
        fence: gmr_store::Fence::Held(1),
        lease_until: t0,
    };
    q.settle(&ghost, Disposition::Reschedule { after_secs: 1 }, t0)
        .await
        .unwrap();
}

#[tokio::test]
async fn queue_contract_on_testkit() {
    queue_contract(&gmr_store::testkit::MemoryQueue::default()).await;
}

#[tokio::test]
async fn queue_contract_on_sqlite() {
    let store = gmr_store::sqlite::open_in_memory().await.unwrap();
    queue_contract(&store.queue()).await;
}
