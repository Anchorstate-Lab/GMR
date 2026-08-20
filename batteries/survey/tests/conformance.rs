use std::collections::BTreeMap;

use gmr_survey::index::{Fault, Generation, Index, Indexed, Located, Row, Snapshot};
use gmr_survey::matching::Want;
use gmr_survey::walk::{Held, Stamp, sort_key};

fn at(n: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + n, 0).unwrap()
}

fn row(ord: u32, id: &str, pairs: &[(&str, &str)]) -> Row {
    Row {
        ord,
        id: id.to_owned(),
        coord: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        facts: serde_json::json!({ "line": ord }),
    }
}

fn file(rel: &str, hash: &str, rows: Vec<Row>) -> Indexed {
    Indexed {
        rel: rel.to_owned(),
        hash: hash.to_owned(),
        sort: sort_key(rel),
        stamp: None,
        rows,
    }
}

fn held(hash: &str) -> Held {
    Held {
        hash: hash.to_owned(),
        stamp: None,
    }
}

fn want(pairs: &[(&str, &str)]) -> Want {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn ids(found: &[Located]) -> Vec<String> {
    found.iter().map(|l| l.row.id.clone()).collect()
}

fn seen(snapshot: Option<Snapshot>) -> Vec<String> {
    ids(&snapshot
        .expect("the generation was written, so it is there to read")
        .rows)
}

fn tree() -> Vec<Indexed> {
    vec![
        file(
            "b.rs",
            "hb",
            vec![row(0, "b:one", &[("kind", "function"), ("name", "one")])],
        ),
        file(
            "b/x.rs",
            "hx",
            vec![
                row(0, "x:two", &[("kind", "function"), ("name", "two")]),
                row(40, "x:three", &[("kind", "type"), ("name", "three")]),
            ],
        ),
        file(
            "a.rs",
            "ha",
            vec![row(0, "a:four", &[("kind", "type"), ("name", "four")])],
        ),
        file(
            "bb.rs",
            "hbb",
            vec![row(0, "bb:five", &[("kind", "function"), ("name", "five")])],
        ),
    ]
}

async fn suite(index: &dyn Index) {
    let ast = Generation::of("ast-map", "v1");
    let addr = Generation::of("addr-map", "v1");

    assert_eq!(
        index.built(&ast).await.unwrap(),
        None,
        "a generation nobody has written to does not exist"
    );
    assert_eq!(
        index.seal(&ast, at(0)).await.unwrap_err().fault,
        Fault::Unopened,
        "sealing a generation that was never opened would record a completeness \
         nobody earned"
    );

    index.write(&ast, &tree()).await.unwrap();

    let built = index.built(&ast).await.unwrap().expect("writing opens it");
    assert_eq!((built.files, built.rows), (4, 5));
    assert!(
        !built.whole(),
        "a generation is open until someone says the walk finished"
    );

    assert_eq!(
        index.known(&ast).await.unwrap(),
        BTreeMap::from([
            ("a.rs".to_owned(), held("ha")),
            ("b.rs".to_owned(), held("hb")),
            ("b/x.rs".to_owned(), held("hx")),
            ("bb.rs".to_owned(), held("hbb")),
        ])
    );

    assert_eq!(
        seen(index.rows(&ast, "").await.unwrap()),
        ["a:four", "x:two", "x:three", "b:one", "bb:five"],
        "rows come back in the order the writer's sort key put them in, and `b/x.rs` \
         sorts before `b.rs` because that is what walking the tree does — a backend \
         ordering by the raw path would put them the other way round, and `nth` would \
         then name a different object"
    );

    assert_eq!(
        seen(index.rows(&ast, "b").await.unwrap()),
        ["x:two", "x:three"],
        "a root selects what is under it. `b.rs` and `bb.rs` both begin with the root's \
         letters and neither is beneath it — a backend testing a plain prefix passes \
         every other case in this suite and fails only here"
    );

    assert_eq!(
        seen(
            index
                .union(&ast, "", &want(&[("kind", "type")]))
                .await
                .unwrap()
        ),
        ["a:four", "x:three"],
        "the union keeps the order the rows were in"
    );
    assert_eq!(
        seen(
            index
                .union(&ast, "", &want(&[("name", "one"), ("kind", "type")]))
                .await
                .unwrap()
        ),
        ["a:four", "x:three", "b:one"],
        "a row is in the union when it matches any one wanted pair, not all of them"
    );
    let nothing = index
        .union(&ast, "", &want(&[("name", "gone")]))
        .await
        .unwrap()
        .expect("the generation is there; it is the coordinate that is not");
    assert!(
        nothing.rows.is_empty(),
        "nothing matching is an empty union, not an error"
    );
    assert!(
        !nothing.whole(),
        "an empty union out of a generation still being written says so. A caller that \
         reads only the rows cannot tell that from `looked, and it is not there`, and \
         those are the two answers this whole system exists to keep apart"
    );
    assert_eq!(
        seen(
            index
                .union(&ast, "b", &want(&[("kind", "function")]))
                .await
                .unwrap()
        ),
        ["x:two"],
        "the root narrows the union as well as the rows, by the same rule"
    );

    let first = index.rows(&ast, "").await.unwrap().unwrap();
    assert_eq!(
        first.rows[0].row.facts,
        serde_json::json!({ "line": 0 }),
        "facts come back as they went in"
    );
    assert_eq!(
        first.rows[0].rel, "a.rs",
        "a row knows which file it came out of"
    );

    index.seal(&ast, at(5)).await.unwrap();
    let built = index.built(&ast).await.unwrap().unwrap();
    assert_eq!(built.sealed_at, Some(at(5)));
    assert!(built.whole());
    assert_eq!(
        index
            .rows(&ast, "")
            .await
            .unwrap()
            .expect("still there")
            .sealed_at,
        Some(at(5)),
        "how stale the answer is has to arrive with the answer. Asking `built` for it \
         afterwards is a second call the caller can forget, and forgetting it is how a \
         snapshot from last week gets reported as the state of the tree"
    );

    index
        .write(&ast, &[file("a.rs", "ha2", vec![row(0, "a:renamed", &[])])])
        .await
        .unwrap();
    let built = index.built(&ast).await.unwrap().unwrap();
    assert_eq!(
        built.sealed_at, None,
        "a write reopens the generation: halfway through an update it is not the \
         snapshot anyone was promised, and saying otherwise is how a partial index \
         answers `found:false` about a file it has not read yet"
    );
    assert_eq!(
        index.known(&ast).await.unwrap()["a.rs"],
        held("ha2"),
        "writing the same path again replaces it rather than adding to it"
    );
    assert_eq!(index.built(&ast).await.unwrap().unwrap().files, 4);

    let stamps = Generation::of("stamp-check", "v1");
    let stamped = Held {
        hash: "hstamped".to_owned(),
        stamp: Some(Stamp {
            mtime_ns: 1_234_567_890,
            size: 42,
        }),
    };
    index
        .write(
            &stamps,
            &[Indexed {
                rel: "stamped.rs".to_owned(),
                hash: stamped.hash.clone(),
                sort: sort_key("stamped.rs"),
                stamp: stamped.stamp,
                rows: vec![],
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        index.known(&stamps).await.unwrap()["stamped.rs"],
        stamped,
        "the stamp a writer hands in comes back unchanged — it is the freshness fast \
         path's only source of truth, so a backend that drops or mangles it silently \
         turns every read after the first into a full re-hash"
    );
    index.discard(&stamps).await.unwrap();

    index.forget(&ast, &["b.rs".to_owned()]).await.unwrap();
    let built = index.built(&ast).await.unwrap().unwrap();
    assert_eq!((built.files, built.rows), (3, 4));
    assert!(!index.known(&ast).await.unwrap().contains_key("b.rs"));

    index.write(&addr, &tree()).await.unwrap();
    index.seal(&addr, at(9)).await.unwrap();
    assert_eq!(index.built(&addr).await.unwrap().unwrap().files, 4);
    assert_eq!(
        index.built(&ast).await.unwrap().unwrap().files,
        3,
        "two probes at the same paths are two indexes, not one"
    );

    let mut listed: Vec<String> = index
        .generations()
        .await
        .unwrap()
        .into_iter()
        .map(|(which, _)| which.probe().to_owned())
        .collect();
    listed.sort();
    assert_eq!(listed, ["addr-map", "ast-map"]);

    let empty = Generation::of("prose-map", "v1");
    index.write(&empty, &[]).await.unwrap();
    let opened = index
        .rows(&empty, "")
        .await
        .unwrap()
        .expect("a generation somebody opened is readable, however little is in it");
    assert!(opened.rows.is_empty());

    index.discard(&ast).await.unwrap();
    assert_eq!(index.built(&ast).await.unwrap(), None);
    assert!(
        index.rows(&ast, "").await.unwrap().is_none(),
        "a generation nobody has built reads as nothing at all, not as an index that \
         happens to be empty. Both used to come back as zero rows, and a probe cannot \
         answer `it is not there` off the first one without inventing the answer"
    );
    assert_eq!(
        index.built(&addr).await.unwrap().unwrap().files,
        4,
        "discarding one generation leaves its neighbours alone"
    );
}

#[tokio::test]
async fn the_reference_implementation_holds() {
    suite(&gmr_survey::testkit::Remembered::new()).await;
}

#[tokio::test]
async fn the_sqlite_backend_agrees_with_it() {
    let index = gmr_survey::sqlite::open_in_memory().await.unwrap();
    suite(&index).await;
    index.close().await;
}
