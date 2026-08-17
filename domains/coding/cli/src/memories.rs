use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::AnchorDecl;

pub const NOTES_DIR: &str = "memories";

pub const NOTES_PROVIDER: &str = "git";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s],
            Self::Many(v) => v,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    Existing(String),
    Declared(Box<Spec>),
}

#[derive(Debug, Deserialize)]
struct Spec {
    key: String,
    probe: String,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    position: Option<Value>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    rules: Vec<String>,
    #[serde(default)]
    terminal: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    #[serde(default)]
    about: Option<OneOrMany>,
    #[serde(default)]
    anchors: Vec<Entry>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    watch: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum Want {
    Existing(String),
    Declared(Box<AnchorDecl>),
}

impl Want {
    pub fn key(&self) -> &str {
        match self {
            Self::Existing(k) => k,
            Self::Declared(d) => &d.key,
        }
    }
}

#[derive(Debug)]
pub struct Note {
    pub path: String,
    pub wants: Vec<Want>,
    pub watch: Option<Vec<String>>,
}

fn from_about(about: &str, catalog: &Catalog, shape: Option<&str>) -> Result<AnchorDecl, CliError> {
    let routed = crate::coord::route(about, shape, catalog)?;
    Ok(AnchorDecl {
        key: about.to_owned(),
        probe: routed.probe,
        params: json!({ "root": "." }),
        position: Some(routed.position),
        shape: Some(routed.shape),
        rules: Vec::new(),
        terminal: Vec::new(),
        retain_full: false,
        cadence_secs: None,
    })
}

fn from_spec(spec: Spec) -> AnchorDecl {
    AnchorDecl {
        key: spec.key,
        probe: spec.probe,
        params: spec.params.unwrap_or_else(|| json!({ "root": "." })),
        position: spec.position,
        shape: spec.shape,
        rules: spec.rules,
        terminal: spec.terminal,
        retain_full: false,
        cadence_secs: None,
    }
}

fn frontmatter_of(text: &str) -> Result<Option<Frontmatter>, String> {
    let rest = match text.strip_prefix("---\n") {
        Some(r) => r,
        None => return Ok(None),
    };
    let Some(end) = rest.find("\n---") else {
        return Err("frontmatter is never closed by `---`".to_owned());
    };
    let body = &rest[..end];
    if body.trim().is_empty() {
        return Ok(None);
    }
    serde_yaml_ng::from_str(body)
        .map(Some)
        .map_err(|e| format!("frontmatter is not valid YAML: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weight {
    Advisory,
    Breaks,
    Blocks,
}

pub struct Fault {
    pub note: String,
    pub key: Option<String>,
    pub code: &'static str,
    pub detail: String,
    pub weight: Weight,
}

impl Fault {
    pub fn breaks(&self) -> bool {
        self.weight >= Weight::Breaks
    }

    pub fn blocks(&self) -> bool {
        self.weight == Weight::Blocks
    }

    pub fn line(&self) -> String {
        format!("{}: {}", self.note, self.detail)
    }
}

fn superfluous(spec: &Spec, catalog: &Catalog) -> bool {
    if !spec.rules.is_empty() || !spec.terminal.is_empty() {
        return false;
    }
    if spec
        .params
        .as_ref()
        .is_some_and(|p| *p != json!({ "root": "." }))
    {
        return false;
    }
    let Ok(routed) = crate::coord::route(&spec.key, spec.shape.as_deref(), catalog) else {
        return false;
    };
    routed.probe == spec.probe && spec.position.as_ref() == Some(&routed.position)
}

fn note_of(root: &Path, rel: &str, catalog: &Catalog) -> (Option<Note>, Vec<Fault>) {
    let mut faults = Vec::new();
    let at = |code, detail, weight| Fault {
        note: rel.to_owned(),
        key: None,
        code,
        detail,
        weight,
    };

    let text = match std::fs::read_to_string(root.join(rel)) {
        Ok(t) => t,
        Err(e) => {
            faults.push(at(
                "unreadable",
                format!("cannot read this file: {e}"),
                Weight::Blocks,
            ));
            return (None, faults);
        }
    };
    let fm = match frontmatter_of(&text) {
        Ok(Some(fm)) => fm,
        Ok(None) => {
            faults.push(at(
                "unclaimed",
                "no frontmatter, so this note names no anchor and nothing observes whether \
                 what it says still holds"
                    .to_owned(),
                Weight::Breaks,
            ));
            return (None, faults);
        }
        Err(reason) => {
            faults.push(at("malformed", reason, Weight::Blocks));
            return (None, faults);
        }
    };

    let mut wants = Vec::new();
    let about = fm.about.map(OneOrMany::into_vec).unwrap_or_default();
    let subscribes = fm.watch.is_some() || fm.shape.is_some();
    if about.is_empty() && fm.anchors.is_empty() && subscribes {
        faults.push(at(
            "unclaimed",
            "`watch:` / `shape:` with no `about:` and no `anchors:` — they say how to observe \
             an anchor this note never names, so nothing observes whether what it says still \
             holds. An empty `anchors:` is how a note claims nothing on purpose"
                .to_owned(),
            Weight::Breaks,
        ));
    }
    for about in about {
        match from_about(&about, catalog, fm.shape.as_deref()) {
            Ok(decl) => wants.push(Want::Declared(Box::new(decl))),
            Err(e) => faults.push(Fault {
                key: Some(about),
                ..at("unrouted", e.to_string(), Weight::Blocks)
            }),
        }
    }
    for entry in fm.anchors {
        match &entry {
            Entry::Existing(key) => faults.push(Fault {
                key: Some(key.clone()),
                ..at(
                    "bare-key",
                    format!(
                        "`{key}` binds without declaring; nothing else in this repo declares \
                         anchors, so this one exists only if something already opened it"
                    ),
                    Weight::Breaks,
                )
            }),
            Entry::Declared(spec) if superfluous(spec, catalog) => faults.push(Fault {
                key: Some(spec.key.clone()),
                ..at(
                    "long-hand",
                    format!(
                        "`{}` states exactly what the coordinate already routes to; \
                         `about: {}` says the same thing",
                        spec.key, spec.key
                    ),
                    Weight::Advisory,
                )
            }),
            Entry::Declared(_) => {}
        }
        wants.push(match entry {
            Entry::Existing(key) => Want::Existing(key),
            Entry::Declared(spec) => Want::Declared(Box::new(from_spec(*spec))),
        });
    }
    faults.extend(tombstones(rel, &text));

    let note = (!wants.is_empty()).then(|| Note {
        path: rel.to_owned(),
        wants,
        watch: fm.watch,
    });
    (note, faults)
}

fn tombstones(rel: &str, text: &str) -> Option<Fault> {
    let named: Vec<String> = crate::shapes::RETIRED
        .iter()
        .filter(|w| text.contains(&format!("`{w}`")))
        .map(|w| format!("`{w}`"))
        .collect();
    (!named.is_empty()).then(|| Fault {
        note: rel.to_owned(),
        key: None,
        code: "retired",
        detail: format!(
            "names {}, which this build no longer has — stale, or deliberately \
             recording what it buried; only you can tell those apart",
            named.join(" ")
        ),
        weight: Weight::Advisory,
    })
}

pub struct Scanned {
    pub notes: Vec<Note>,
    pub faults: Vec<Fault>,
}

impl Scanned {
    pub fn blocked(&self) -> impl Iterator<Item = &Fault> {
        self.faults.iter().filter(|f| f.blocks())
    }

    pub fn blocked_key(&self, key: &str) -> Option<&Fault> {
        self.blocked().find(|f| f.key.as_deref() == Some(key))
    }
}

pub fn scan(root: &Path, catalog: &Catalog) -> Result<Scanned, CliError> {
    let mut rels = Vec::new();
    walk(root, &root.join(NOTES_DIR), &mut rels)?;
    rels.sort();

    let mut notes = Vec::new();
    let mut faults = Vec::new();
    for rel in rels {
        let (note, mut f) = note_of(root, &rel, catalog);
        notes.extend(note);
        faults.append(&mut f);
    }
    Ok(Scanned { notes, faults })
}

fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), CliError> {
    let Ok(entries) = std::fs::read_dir(at) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPES: &str = r#"
[probe.ast-map]
stage = { probe = "x" }
entrypoint = "probe"
sources = ["x"]
handles = ["rs", "ts", "tsx", "js", "py", "go"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "name"], facts = ["body", "line"] }
"#;

    fn world(notes: &[(&str, &str)]) -> (tempfile::TempDir, Catalog) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        std::fs::write(dir.path().join(".anchor/probes.toml"), RECIPES).unwrap();
        std::fs::write(dir.path().join("x"), "body").unwrap();
        for (path, body) in notes {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let catalog = Catalog::load(dir.path()).unwrap();
        (dir, catalog)
    }

    #[test]
    fn one_line_of_frontmatter_becomes_a_whole_anchor() {
        let (d, r) = world(&[(
            "memories/auth.md",
            "---\nabout: src/auth.ts#createSession\n---\n\n# note\n",
        )]);
        let notes = scan(d.path(), &r).unwrap().notes;
        assert_eq!(notes.len(), 1);

        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.key, "src/auth.ts#createSession");
        assert_eq!(decl.probe, "ast-map");
        assert_eq!(decl.shape.as_deref(), Some("contract"));
        assert_eq!(
            decl.position,
            Some(json!({"file": "src/auth.ts", "name": "createSession"}))
        );
    }

    #[test]
    fn a_note_picks_its_shape_and_what_it_wants_woken_for() {
        let (d, r) = world(&[(
            "memories/auth.md",
            "---\nabout: src/auth.ts#createSession\nshape: contract\nwatch: [logic]\n---\n",
        )]);
        let notes = scan(d.path(), &r).unwrap().notes;
        assert_eq!(
            notes[0].watch.as_deref(),
            Some(["logic".to_owned()].as_ref())
        );

        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.shape.as_deref(), Some("contract"));
    }

    #[test]
    fn a_coordinate_with_no_part_watches_the_whole_files_roster() {
        let (d, r) = world(&[("memories/a.md", "---\nabout: src/a.ts\n---\n")]);
        let notes = scan(d.path(), &r).unwrap().notes;
        assert!(notes[0].watch.is_none());

        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.shape.as_deref(), Some("roster"));
        assert_eq!(decl.position, Some(json!({ "file": "src/a.ts" })));
    }

    #[test]
    fn a_bare_key_binds_without_declaring_anything() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - surface::gmr-core\n  - modules::gmr-core\n---\n",
        )]);
        let notes = scan(d.path(), &r).unwrap().notes;
        let keys: Vec<_> = notes[0].wants.iter().map(Want::key).collect();
        assert_eq!(keys, vec!["surface::gmr-core", "modules::gmr-core"]);
        assert!(matches!(notes[0].wants[0], Want::Existing(_)));
    }

    #[test]
    fn an_extension_no_probe_reads_falls_to_the_derived_catchall() {
        let (d, r) = world(&[("memories/x.md", "---\nabout: schema/a.proto\n---\n")]);
        let notes = scan(d.path(), &r).unwrap().notes;
        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.probe, "addr-map");
    }

    #[test]
    fn a_note_without_frontmatter_is_not_a_note() {
        let (d, r) = world(&[("memories/plain.md", "# just prose\n")]);
        let scanned = scan(d.path(), &r).unwrap();
        assert!(scanned.notes.is_empty());
        assert!(scanned.blocked().next().is_none());
    }

    #[test]
    fn unclosed_frontmatter_is_broken_but_does_not_stop_the_scan() {
        let (d, r) = world(&[
            ("memories/bad.md", "---\nabout: a.rs\n"),
            ("memories/good.md", "---\nabout: src/a.ts\n---\n"),
        ]);
        let scanned = scan(d.path(), &r).unwrap();
        assert_eq!(
            scanned.notes.len(),
            1,
            "one note in the same scan is malformed, but the other still becomes an anchor"
        );
        let blocked: Vec<_> = scanned.blocked().collect();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].note, "memories/bad.md");
        assert_eq!(blocked[0].key, None);
        assert!(
            blocked[0].line().contains("bad.md"),
            "{}",
            blocked[0].line()
        );
    }

    #[test]
    fn a_broken_about_coordinate_still_names_the_key_it_would_have_been() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nabout: src/a.ts#thing\nshape: not-a-real-shape\n---\n",
        )]);
        let scanned = scan(d.path(), &r).unwrap();
        assert!(scanned.notes.is_empty());
        let blocked: Vec<_> = scanned.blocked().collect();
        assert_eq!(blocked.len(), 1);
        assert_eq!(
            blocked[0].key.as_deref(),
            Some("src/a.ts#thing"),
            "the about string is the key an anchor would have opened under, and it is known \
             before routing succeeds — losing it here is losing the only thing that could \
             later match this failure back to a journal entry"
        );
    }

    #[test]
    fn an_unrouted_about_coordinate_is_visible_to_both_the_opener_and_the_auditor() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nabout: src/a.ts#thing\nshape: not-a-real-shape\n---\n",
        )]);
        let scanned = scan(d.path(), &r).unwrap();
        assert_eq!(
            scanned.blocked().count(),
            1,
            "sync/check/status/accept read this — it decides which anchors open"
        );
        let unrouted: Vec<_> = scanned
            .faults
            .iter()
            .filter(|l| l.code == "unrouted")
            .collect();
        assert_eq!(
            unrouted.len(),
            1,
            "doctor reads this — before this merge, the same failure was invisible to it \
             because scan and lint were two independent walks and only one of them called \
             coord::route on `about:` at all"
        );
        assert!(
            unrouted[0].blocks(),
            "one record, weighed once: `blocks` is what stopped a want from existing and what              sync/check/status join on, `breaks` is what doctor exits 1 for. They used to be              two structs carrying the same failure under two spellings, and only a person              comparing them could tell they had not drifted apart"
        );
        assert!(unrouted[0].breaks());
        assert_eq!(scanned.blocked().count(), 1);
    }

    #[test]
    fn the_explicit_form_still_reaches_every_knob() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - key: surface::auth\n    probe: ast-map\n    shape: roster\n    position: { file: src/auth.ts, kind: function }\n---\n",
        )]);
        let notes = scan(d.path(), &r).unwrap().notes;
        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.key, "surface::auth");
        assert_eq!(
            decl.position,
            Some(json!({"file": "src/auth.ts", "kind": "function"}))
        );
    }

    fn lint(d: &std::path::Path, r: &Catalog) -> Result<Vec<Fault>, CliError> {
        Ok(scan(d, r)?.faults)
    }

    fn codes(d: &std::path::Path, r: &Catalog) -> Vec<(String, &'static str)> {
        lint(d, r)
            .unwrap()
            .into_iter()
            .map(|l| (l.note, l.code))
            .collect()
    }

    #[test]
    fn the_canonical_form_draws_no_complaint() {
        let (d, r) = world(&[(
            "memories/auth.md",
            "---\nabout: src/auth.ts#createSession\nwatch: [logic]\n---\n\n# note\n",
        )]);
        assert!(codes(d.path(), &r).is_empty());
    }

    #[test]
    fn a_note_that_deliberately_claims_nothing_is_left_alone() {
        let (d, r) = world(&[("memories/README.md", "---\nanchors:\n---\n\n# how to\n")]);
        assert!(codes(d.path(), &r).is_empty());
    }

    #[test]
    fn a_malformed_frontmatter_is_caught_rather_than_stopping_the_whole_lint() {
        let (d, r) = world(&[
            ("memories/bad.md", "---\nabout: [unterminated\n---\n"),
            (
                "memories/auth.md",
                "---\nabout: src/auth.ts#createSession\n---\n",
            ),
        ]);
        assert_eq!(
            codes(d.path(), &r),
            vec![("memories/bad.md".to_owned(), "malformed")],
            "one note's frontmatter fails to parse; the other, well-formed one is not \
             swept into the same failure and draws no complaint of its own"
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_note_with_no_frontmatter_names_no_anchor_and_is_caught() {
        let (d, r) = world(&[("memories/loose.md", "# just prose\n")]);
        assert_eq!(
            codes(d.path(), &r),
            vec![("memories/loose.md".to_owned(), "unclaimed")]
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_watch_with_nothing_to_watch_is_as_unclaimed_as_no_frontmatter_at_all() {
        let (d, r) = world(&[("memories/watched.md", "---\nwatch: [sig]\n---\n\n# note\n")]);
        assert!(
            scan(d.path(), &r).unwrap().notes.is_empty(),
            "it declares nothing, so it binds nothing"
        );
        assert_eq!(
            codes(d.path(), &r),
            vec![("memories/watched.md".to_owned(), "unclaimed")],
            "a note whose frontmatter parses but names no coordinate used to be the one \
             failure nothing reported: not an anchor, not a complaint, not a line of output \
             anywhere. Its author believes it is watched, and `watch:` alone reads exactly \
             like it would if it were"
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_bare_key_binds_without_declaring_and_is_caught() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - some::key\n---\n\n# note\n",
        )]);
        assert_eq!(
            codes(d.path(), &r),
            vec![("memories/x.md".to_owned(), "bare-key")]
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_long_hand_entry_the_coordinate_already_routes_to_is_advice_not_breakage() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - key: src/auth.ts#createSession\n    probe: ast-map\n             \n    position: { file: src/auth.ts, name: createSession }\n    shape: contract\n---\n",
        )]);
        let found = lint(d.path(), &r).unwrap();
        assert_eq!(
            found.len(),
            1,
            "{found:?}",
            found = found.iter().map(|l| l.code).collect::<Vec<_>>()
        );
        assert_eq!(found[0].code, "long-hand");
        assert!(!found[0].breaks());
        assert!(found[0].detail.contains("about: src/auth.ts#createSession"));
    }

    #[test]
    fn a_long_hand_entry_that_earns_its_keep_is_not_complained_about() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - key: src/auth.ts#createSession\n    probe: ast-map\n             \n    position: { file: src/auth.ts, name: createSession, kind: function }\n             \n    shape: contract\n---\n",
        )]);
        assert!(
            codes(d.path(), &r).is_empty(),
            "an extra position key is a reason"
        );

        let (d, r) = world(&[(
            "memories/y.md",
            "---\nanchors:\n  - key: k\n    probe: ast-map\n    rules:\n             \n      - 'true => { status: \"x\" }'\n---\n",
        )]);
        assert!(
            codes(d.path(), &r).is_empty(),
            "hand-written rules are a reason"
        );
    }
}
