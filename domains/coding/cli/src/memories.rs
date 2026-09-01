use std::path::Path;

use gmr::Ref;

use crate::notes::{Claim, Stated};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::probes::Catalog;
use crate::verbs::sync::AnchorDecl;

pub const NOTES_DIR: &str = "memories";

pub fn declaring(root: &Path) -> crate::notes::Notes {
    crate::notes::Notes::at(root, NOTES_DIR)
}

pub const RESOLVED_THROUGH: &str = "git";

pub fn addressed(reference: &Ref) -> String {
    reference.to_string()
}

fn addressed_to(provider: &str, external_id: &str) -> Result<Ref, CliError> {
    let named = |what: &str, e: gmr::core::NewtypeError| {
        CliError(format!(
            "`{provider}:{external_id}` names no record: its {what} {e}"
        ))
    };
    Ok(Ref {
        provider: gmr::ProviderId::try_new(provider).map_err(|e| named("provider", e))?,
        external_id: gmr::ExternalId::try_new(external_id).map_err(|e| named("id", e))?,
    })
}

fn registered(named: &str, external_id: &str, known: &[&str]) -> Result<Ref, CliError> {
    if !known.contains(&named) {
        return Err(CliError(format!(
            "no store named `{named}` is registered in this binary. Registered here: {}. \
             This run cannot say what `{named}:{external_id}` refers to, and recording it \
             against some other store would make a record nobody ever wrote read as one \
             that store deleted",
            match known.is_empty() {
                true => "none".to_owned(),
                false => known.join(", "),
            }
        )));
    }
    addressed_to(named, external_id)
}

pub fn located(text: &str, provider: Option<&str>, known: &[&str]) -> Result<Ref, CliError> {
    let carried = text
        .split_once(':')
        .filter(|(named, rest)| !rest.is_empty() && gmr::ProviderId::try_new(*named).is_ok());
    match (carried, provider) {
        (Some((named, rest)), None) => registered(named, rest, known),
        (Some((named, rest)), Some(want)) if want == named => registered(named, rest, known),
        (Some((named, _)), Some(want)) => Err(CliError(format!(
            "`{text}` is addressed to `{named}` and --provider says `{want}`. One of them is \
             not what you meant, and guessing which would bind this to a store you did not name"
        ))),
        (None, Some(want)) => registered(want, text, known),
        (None, None) => registered(RESOLVED_THROUGH, text, known),
    }
}

#[derive(Default)]
pub struct Names {
    sources: Vec<std::sync::Arc<crate::notes::Notes>>,
}

impl Names {
    pub fn over(sources: Vec<std::sync::Arc<crate::notes::Notes>>) -> Self {
        Self { sources }
    }

    pub fn named(&self, reference: &Ref) -> Option<String> {
        self.sources
            .iter()
            .find_map(|source| source.name_of(reference))
    }

    pub fn of(&self, reference: &Ref) -> String {
        self.named(reference)
            .unwrap_or_else(|| addressed(reference))
    }
}

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

fn stated_or_empty<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
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
    #[serde(default, deserialize_with = "stated_or_empty")]
    rules: Vec<String>,
    #[serde(default, deserialize_with = "stated_or_empty")]
    terminal: Vec<String>,
    #[serde(flatten)]
    settings: crate::settings::Declared,
}

#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    #[serde(default)]
    about: Option<OneOrMany>,
    #[serde(default, deserialize_with = "stated_or_empty")]
    anchors: Vec<Entry>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    watch: Option<Watch>,
    #[serde(default)]
    links: Option<std::collections::BTreeMap<String, OneOrMany>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Watch {
    Axes(Vec<String>),
    When(String),
}

const FRONTMATTER_WORDS: [&str; 5] = ["about", "anchors", "links", "shape", "watch"];

fn foreign_words(said: &Value) -> Vec<String> {
    let Some(map) = said.as_object() else {
        return Vec::new();
    };
    map.keys()
        .filter(|k| !FRONTMATTER_WORDS.contains(&k.as_str()))
        .map(|k| format!("`{k}`"))
        .collect()
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
    pub reference: Ref,
    pub wants: Vec<Want>,
    pub watch: Option<Watch>,
    pub links: Vec<(String, Ref)>,
}

pub fn linked_target(text: &str) -> Result<Ref, String> {
    if let Some((provider, rest)) = text.split_once(':') {
        if rest.is_empty() || gmr::ProviderId::try_new(provider).is_err() {
            return Err(format!(
                "`{text}` looks like an address but `{provider}:` is not a provider prefix \
                 this note format can carry"
            ));
        }
        return Ok(Ref::new(provider, rest));
    }
    if text.is_empty() || text.contains('/') || text.ends_with(".md") {
        return Err(format!(
            "`{text}` is neither a bare note name nor a `provider:id` address. A bare name \
             resolves to git:memories/<name>.md; anything with a path in it must say its \
             provider in full"
        ));
    }
    Ok(Ref::new("git", format!("memories/{text}.md")))
}

fn from_about(about: &str, catalog: &Catalog, shape: Option<&str>) -> Result<AnchorDecl, CliError> {
    let routed = crate::coord::route(about, shape, catalog)?;
    Ok(AnchorDecl {
        key: about.to_owned(),
        probe: routed.probe,
        params: routed.params,
        position: Some(routed.position),
        shape: Some(routed.shape),
        rules: Vec::new(),
        terminal: Vec::new(),
        watch: None,
        settings: crate::settings::Declared::default(),
    })
}

fn from_spec(spec: Spec) -> AnchorDecl {
    AnchorDecl {
        key: spec.key,
        probe: spec.probe,
        params: spec.params.unwrap_or_else(crate::coord::whole_repository),
        position: spec.position,
        shape: spec.shape,
        rules: spec.rules,
        terminal: spec.terminal,
        watch: None,
        settings: spec.settings,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weight {
    Advisory,
    Breaks,
    Blocks,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Fault {
    pub note: String,
    pub key: Option<String>,
    pub code: &'static str,
    pub detail: String,
    #[serde(skip)]
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

fn claims_of(
    declared: &Stated,
    source: &crate::notes::Notes,
    catalog: &Catalog,
) -> (Option<Note>, Vec<Fault>) {
    let record = &declared.record;
    let named = source
        .name_of(&record.reference)
        .unwrap_or_else(|| addressed(&record.reference));
    let rel = named.as_str();
    let text = String::from_utf8_lossy(&record.bytes);
    let mut faults = Vec::new();
    let at = |code, detail, weight| Fault {
        note: rel.to_owned(),
        key: None,
        code,
        detail,
        weight,
    };

    let claimed = declared.claim.clone();
    let said = match &claimed {
        Claim::Silent => {
            faults.push(at(
                "unclaimed",
                "no frontmatter, so this note names no anchor and nothing observes whether \
                 what it says still holds"
                    .to_owned(),
                Weight::Breaks,
            ));
            return (None, faults);
        }
        Claim::Malformed(reason) => {
            faults.push(at("malformed", reason.clone(), Weight::Blocks));
            return (None, faults);
        }
        Claim::Says(value) => value,
    };
    let foreign = foreign_words(said);
    if !foreign.is_empty() {
        faults.push(at(
            "unrecognised",
            format!(
                "names {}, which this note format has no words for. A header that is read \
                 but not understood declares nothing, so nothing observes whether what this \
                 note says still holds — and unlike a note with no frontmatter at all, this \
                 one looks from the outside like it declared something. The words are {}",
                foreign.join(" "),
                FRONTMATTER_WORDS.map(|w| format!("`{w}`")).join(" · ")
            ),
            Weight::Breaks,
        ));
    }
    let fm: Frontmatter = match serde_json::from_value(said.clone()) {
        Ok(fm) => fm,
        Err(e) => {
            faults.push(at(
                "malformed",
                format!("frontmatter is not shaped like a note's: {e}"),
                Weight::Blocks,
            ));
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

    let mut links = Vec::new();
    for (kind, targets) in fm.links.unwrap_or_default() {
        if kind.is_empty() {
            faults.push(at(
                "unlinkable",
                "a link kind is an empty string; an edge whose kind says nothing cannot be \
                 meant"
                    .to_owned(),
                Weight::Breaks,
            ));
            continue;
        }
        for target in targets.into_vec() {
            match linked_target(&target) {
                Ok(to) => links.push((kind.clone(), to)),
                Err(reason) => faults.push(Fault {
                    key: Some(target),
                    ..at("unlinkable", reason, Weight::Breaks)
                }),
            }
        }
    }
    links.sort();
    links.dedup();

    let note = (!wants.is_empty() || !links.is_empty()).then(|| Note {
        reference: record.reference.clone(),
        wants,
        watch: fm.watch,
        links,
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

    pub fn accounted_for<'a>(&mut self, keys: impl Iterator<Item = &'a str>) {
        let known: std::collections::BTreeSet<&str> = keys.collect();
        self.faults.retain(|f| {
            f.code != "bare-key" || !f.key.as_deref().is_some_and(|k| known.contains(k))
        });
    }
}

pub fn scan(root: &Path, catalog: &Catalog) -> Result<Scanned, CliError> {
    of(&declaring(root), catalog)
}

pub fn of(source: &crate::notes::Notes, catalog: &Catalog) -> Result<Scanned, CliError> {
    let declared = source.declared()?;
    let mut notes = Vec::new();
    let mut faults = Vec::new();
    for record in &declared {
        let (note, mut f) = claims_of(record, source, catalog);
        notes.extend(note);
        faults.append(&mut f);
    }
    Ok(Scanned { notes, faults })
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
    fn a_bare_key_that_something_else_declares_is_not_a_fault() {
        let (d, r) = world(&[(
            "memories/deploy.md",
            "---\nanchors:\n  - deploy::staging\n---\n",
        )]);
        let mut scanned = scan(d.path(), &r).unwrap();

        assert!(
            scanned.faults.iter().any(|f| f.code == "bare-key"),
            "the scan cannot know what declares a key, so it reports every bare one"
        );

        scanned.accounted_for(["deploy::staging"].into_iter());

        assert!(
            !scanned.faults.iter().any(|f| f.code == "bare-key"),
            "`anchors.toml` declaring the key, or an anchor already standing under it, is \
             exactly what the lint says is missing. Left standing it makes `sync` exit 1 on a \
             repository whose script probe and note are both correct"
        );
    }

    #[test]
    fn a_note_declares_typed_edges_and_bare_names_resolve_to_git_notes() {
        let (d, r) = world(&[(
            "memories/a.md",
            "---\nabout: x\nlinks:\n  rests-on: [positioning, \"mem0:9f8e\"]\n  contradicts: [b]\n---\nbody",
        )]);
        let scanned = scan(d.path(), &r).unwrap();
        let note = &scanned.notes[0];
        assert_eq!(
            note.links,
            vec![
                ("contradicts".to_owned(), Ref::new("git", "memories/b.md")),
                (
                    "rests-on".to_owned(),
                    Ref::new("git", "memories/positioning.md")
                ),
                ("rests-on".to_owned(), Ref::new("mem0", "9f8e")),
            ],
            "a bare name is the wikilink habit made runtime-real, and a full address \
             reaches any registered store — an edge is not a git-only capability"
        );
    }

    #[test]
    fn an_edge_target_that_is_neither_name_nor_address_is_refused_with_the_grammar() {
        let (d, r) = world(&[(
            "memories/a.md",
            "---\nabout: x\nlinks:\n  rests-on: [\"memories/b.md\"]\n---\nbody",
        )]);
        let scanned = scan(d.path(), &r).unwrap();
        assert!(
            scanned.faults.iter().any(|f| f.code == "unlinkable"),
            "`memories/b.md` as a bare name would resolve to memories/memories/b.md.md \
             and record an edge to a record that never existed; refusing is the only \
             answer that does not write a wrong address into an append-only store"
        );
    }

    #[test]
    fn a_note_carrying_only_edges_is_still_a_note() {
        let (d, r) = world(&[("memories/a.md", "---\nlinks:\n  rests-on: [b]\n---\nbody")]);
        let scanned = scan(d.path(), &r).unwrap();
        assert_eq!(scanned.notes.len(), 1);
        assert!(scanned.notes[0].wants.is_empty());
        assert_eq!(scanned.notes[0].links.len(), 1);
    }

    const STORES: [&str; 3] = ["git", "mem0", "claude-code"];

    #[test]
    fn every_address_this_cli_prints_can_be_handed_straight_back() {
        for reference in [
            Ref::new("git", "memories/auth.md"),
            Ref::new("mem0", "9f8e1d02-0000-4000-8000-000000000000"),
            Ref::new("claude-code", "feedback.md"),
        ] {
            assert_eq!(
                located(&addressed(&reference), None, &STORES).unwrap(),
                reference,
                "`addressed` is how every verb hands a record to whoever reads its output, \
                 and SKILL.md tells an agent to use that string verbatim. Without a parse \
                 that undoes it the encoding is one-way: `bind` and `reaffirm` refuse it as \
                 a record that does not exist, and `cobound` answers about an address nobody \
                 ever wrote — with no error anywhere"
            );
        }
    }

    #[test]
    fn a_prefix_that_is_not_a_provider_name_is_part_of_the_id() {
        for text in ["memories/a:b.md", "notes.d:x", "Mem0:9f8e"] {
            assert_eq!(
                located(text, None, &STORES).unwrap(),
                Ref::new("git", text),
                "an id is allowed to contain a colon. What may be taken as a prefix is \
                 settled by `ProviderId`'s grammar, so the same text is the same record in \
                 every build — a rule that consulted the registry instead would make \
                 `{text}` one record here and a different one wherever that store is \
                 configured"
            );
        }
    }

    #[test]
    fn a_store_this_run_never_registered_is_refused_rather_than_rewritten() {
        let e = located("mem0:9f8e", None, &["git"]).unwrap_err();

        assert!(e.0.contains("mem0"), "{}", e.0);
        assert!(
            e.0.contains("git"),
            "the refusal has to name what is registered, or the reader cannot tell a \
             typo from a feature this build lacks: {}",
            e.0
        );
        assert!(
            !e.0.is_empty(),
            "recording this against `git` instead is the failure this refusal exists for: \
             we could not resolve the store, which is our failure, and git would then \
             answer that no such path exists — the world's answer. The binding table only \
             ever grows, so that laundering is permanent and reads forever as a record \
             somebody deleted"
        );
    }

    #[test]
    fn a_bare_path_still_reaches_the_store_it_always_did() {
        assert_eq!(
            located("memories/auth.md", None, &STORES).unwrap(),
            Ref::new("git", "memories/auth.md")
        );
        assert_eq!(
            located("9f8e", Some("mem0"), &STORES).unwrap(),
            Ref::new("mem0", "9f8e")
        );
    }

    #[test]
    fn an_address_that_contradicts_the_flag_is_refused_rather_than_picked() {
        located("mem0:9f8e", Some("git"), &STORES).expect_err(
            "silently preferring either one binds this record to a store the caller did not \
             name, and the binding table is append-only",
        );
        located("mem0:9f8e", Some("mem0"), &STORES)
            .expect("saying the same thing twice is not a contradiction");
    }

    #[test]
    fn an_address_with_no_id_behind_it_is_refused_rather_than_bound() {
        let e = located("", None, &STORES).unwrap_err();
        assert!(
            e.0.contains("names no record"),
            "an empty id would bind an anchor to a record no provider can ever fetch, and \
             `read` would report it as `gone` forever with nothing to restore: {}",
            e.0
        );
    }

    #[test]
    fn a_record_no_source_names_still_says_which_store_it_is_in() {
        let (d, _) = world(&[]);
        let names = Names::over(vec![std::sync::Arc::new(declaring(d.path()))]);

        assert_eq!(
            names.of(&Ref::new("mem0", "4f3a91e2-8c7d")),
            "mem0:4f3a91e2-8c7d"
        );
        assert_eq!(names.of(&Ref::new("git", "memories/auth.md")), "auth");
    }

    #[test]
    fn a_name_is_available_wherever_the_book_is_and_nowhere_else() {
        let (d, _) = world(&[]);
        let reference = Ref::new("git", "memories/auth.md");

        assert_eq!(
            Names::default().of(&reference),
            "git:memories/auth.md",
            "a verb that was never handed the book falls back to the address, which is \
             honest. What it must not do is print a third spelling of its own — `check` \
             printed the address, `doctor` and `edges` printed the bare id, and the reader \
             had no way to tell that all three meant one record"
        );
        assert_eq!(
            Names::over(vec![std::sync::Arc::new(declaring(d.path()))]).of(&reference),
            "auth"
        );
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
        assert_eq!(notes[0].watch, Some(Watch::Axes(vec!["logic".to_owned()])));

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
        assert_eq!(
            blocked[0].note, "bad",
            "a fault names the note the way its author does. The address a store happens to \
             keep it at is the store's business, and for a store that addresses by uuid it \
             is nothing a reader could act on"
        );
        assert_eq!(blocked[0].key, None);
        assert!(blocked[0].line().contains("bad"), "{}", blocked[0].line());
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
            vec![("bad".to_owned(), "malformed")],
            "one note's frontmatter fails to parse; the other, well-formed one is not \
             swept into the same failure and draws no complaint of its own"
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_note_with_no_frontmatter_names_no_anchor_and_is_caught() {
        let (d, r) = world(&[("memories/loose.md", "# just prose\n")]);
        assert_eq!(codes(d.path(), &r), vec![("loose".to_owned(), "unclaimed")]);
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
            vec![("watched".to_owned(), "unclaimed")],
            "a note whose frontmatter parses but names no coordinate used to be the one \
             failure nothing reported: not an anchor, not a complaint, not a line of output \
             anywhere. Its author believes it is watched, and `watch:` alone reads exactly \
             like it would if it were"
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_header_in_another_products_format_is_louder_than_no_header_at_all() {
        let (d, r) = world(&[(
            "memories/claude.md",
            "---\nname: commit-messages-in-english\ndescription: \"be terse\"\nmetadata:\n               type: feedback\n---\n\n# note\n",
        )]);

        assert!(
            scan(d.path(), &r).unwrap().notes.is_empty(),
            "it names no coordinate this format knows, so it declares no anchor"
        );
        assert_eq!(
            codes(d.path(), &r),
            vec![("claude".to_owned(), "unrecognised")],
            "a note whose header is another tool's format used to be the quietest failure \
             here: serde dropped every word it did not know, the note declared nothing, and \
             nothing complained. That is worse than no frontmatter at all, which at least \
             reports `unclaimed` — this one looks from the outside like it declared \
             something, and the author has no reason to look again"
        );
        assert!(lint(d.path(), &r).unwrap()[0].breaks());
    }

    #[test]
    fn a_misspelt_word_is_foreign_like_any_other() {
        let (d, r) = world(&[("memories/typo.md", "---\nabuot: src/a.ts\n---\n\n# note\n")]);
        assert_eq!(
            codes(d.path(), &r),
            vec![("typo".to_owned(), "unrecognised")],
            "one transposed pair of letters and the note silently stopped declaring \
             anything; the same check that catches another tool's format catches this"
        );
    }

    #[test]
    fn a_note_that_declares_and_also_says_something_foreign_still_declares() {
        let (d, r) = world(&[(
            "memories/both.md",
            "---\nabout: src/a.ts\nauthor: someone\n---\n\n# note\n",
        )]);

        let scanned = scan(d.path(), &r).unwrap();
        assert_eq!(
            scanned.notes[0].wants.len(),
            1,
            "the words this format knows are still read; a foreign one beside them does not \
             cost the note its anchor"
        );
        assert_eq!(
            codes(d.path(), &r),
            vec![("both".to_owned(), "unrecognised")]
        );
    }

    #[test]
    fn a_bare_key_binds_without_declaring_and_is_caught() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - some::key\n---\n\n# note\n",
        )]);
        assert_eq!(codes(d.path(), &r), vec![("x".to_owned(), "bare-key")]);
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

    #[test]
    fn a_note_can_say_every_knob_the_toml_can_and_still_say_none_of_them() {
        let (d, c) = world(&[
            ("src/a.rs", "fn a() {}"),
            (
                "memories/tuned.md",
                "---\nanchors:\n  - key: tuned\n    probe: ast-map\n    \
                 position: { file: src/a.rs }\n    shape: roster\n    budget_ms: 1500\n---\n",
            ),
            ("memories/plain.md", "---\nabout: src/a.rs\n---\n"),
        ]);
        let scanned = scan(d.path(), &c).unwrap();
        let said = |note: &str| {
            scanned
                .notes
                .iter()
                .find(|n| n.reference.external_id.as_str().contains(note))
                .and_then(|n| n.wants.first())
                .map(|w| match w {
                    Want::Declared(decl) => decl.settings,
                    Want::Existing(_) => panic!("both notes declare"),
                })
                .unwrap()
        };
        assert_eq!(said("tuned").budget_ms, Some(1500));
        assert_eq!(
            said("plain"),
            crate::settings::Declared::default(),
            "`about:` is one line and can only ever mean the coordinate. It has to arrive \
             saying nothing about how the anchor runs — the alternative is a shorthand that \
             silently resets knobs somebody set on purpose, every time sync runs"
        );
    }
}
