use std::path::Path;

use gmr::{
    Anchor, AnchorKey, AnchorView, OpenRequest, ProbeRef, Ref, RunSettings, Runtime, State,
    Transitions, Version,
};
use serde::Deserialize;

use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;

pub const DEFAULT_FILE: &str = ".anchor/anchors.toml";

pub struct Context {
    pub catalog: Catalog,
}

#[derive(Debug, Default, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub anchor: Vec<AnchorDecl>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnchorDecl {
    pub key: String,
    pub probe: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub position: Option<serde_json::Value>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub terminal: Vec<String>,
    #[serde(default)]
    pub watch: Option<crate::memories::Watch>,
    #[serde(flatten)]
    pub settings: crate::settings::Declared,
}

impl AnchorDecl {
    pub fn to_probe(&self, ctx: &Context) -> Result<ProbeRef, CliError> {
        rules::probe(
            ctx.catalog.kind_of(&self.probe),
            &self.probe,
            self.params.clone(),
        )
    }

    fn check_contract(&self, ctx: &Context) -> Result<(), CliError> {
        let reads = crate::contract::reads_of(&self.to_transitions()?)
            .map_err(|e| CliError(format!("{}: {e}", self.key)))?;
        let missing = crate::contract::unmet(&reads, &ctx.catalog.obs_of(&self.probe)?);
        match missing.is_empty() {
            true => Ok(()),
            false => Err(CliError(format!(
                "{}: rules read {}, which probe `{}` does not emit",
                self.key,
                missing.join(" · "),
                self.probe
            ))),
        }
    }

    pub fn to_transitions(&self) -> Result<Transitions, CliError> {
        match (&self.shape, self.rules.is_empty()) {
            (Some(_), false) => Err(CliError(format!(
                "{}: declare either `shape` or `rules`, not both",
                self.key
            ))),
            (Some(name), true) => rules::transitions(&crate::shapes::rules_of(
                crate::shapes::get(name).map_err(|e| CliError(format!("{}: {e}", self.key)))?,
            )),
            (None, _) => rules::transitions(&self.rules),
        }
    }

    fn initial(&self) -> Option<State> {
        self.position
            .clone()
            .map(|p| State::new(serde_json::json!({ "position": p })))
    }
}

pub fn read_declared(root: &Path, file: &str) -> Result<Declared, CliError> {
    let path = root.join(file);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(toml::from_str(&text)?),
        Err(_) if !path.exists() && file == DEFAULT_FILE => Ok(Declared::default()),
        Err(e) => Err(CliError(format!("cannot read `{file}`: {e}"))),
    }
}

#[derive(serde::Serialize)]
struct Written<'a> {
    key: &'a str,
    probe: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<&'a serde_json::Value>,
}

#[derive(serde::Serialize)]
struct Block<'a> {
    anchor: Vec<Written<'a>>,
}

pub fn declare(root: &Path, file: &str, decl: &AnchorDecl) -> Result<(), CliError> {
    let block = Block {
        anchor: vec![Written {
            key: &decl.key,
            probe: &decl.probe,
            shape: decl.shape.as_ref(),
            params: (!decl.params.is_null()).then_some(&decl.params),
            position: decl.position.as_ref(),
        }],
    };
    let written = toml::to_string(&block).map_err(|e| {
        CliError(format!(
            "cannot write a declaration for `{}`: {e}",
            decl.key
        ))
    })?;

    let path = root.join(file);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError(format!("cannot create {}: {e}", parent.display())))?;
    }
    let mut held = std::fs::read_to_string(&path).unwrap_or_default();
    if !held.is_empty() && !held.ends_with('\n') {
        held.push('\n');
    }
    std::fs::write(&path, format!("{held}{written}"))
        .map_err(|e| CliError(format!("cannot write {}: {e}", path.display())))
}

pub fn merged<'a>(
    declared: &'a Declared,
    notes: &'a [crate::memories::Note],
) -> Vec<&'a AnchorDecl> {
    let from_notes = notes
        .iter()
        .flat_map(|n| &n.wants)
        .filter_map(|w| match w {
            crate::memories::Want::Declared(d) => Some(d.as_ref()),
            crate::memories::Want::Existing(_) => None,
        })
        .filter(|d| !declared.anchor.iter().any(|t| t.key == d.key));

    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for decl in declared.anchor.iter().chain(from_notes) {
        if seen.contains(&decl.key.as_str()) {
            continue;
        }
        seen.push(&decl.key);
        out.push(decl);
    }
    out
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Synced {
    pub opened: Vec<String>,
    pub criteria_drifted: Vec<String>,
    pub instrument_swapped: Vec<String>,
    pub resettled: Vec<String>,
    pub bound: Vec<String>,
    pub renamed: Vec<String>,
    pub warnings: Vec<String>,
    pub dry_run: bool,
    pub scheduled: usize,
    pub broken: Vec<crate::memories::Fault>,
}

impl Synced {
    pub fn code(&self) -> i32 {
        i32::from(!self.broken.is_empty())
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    names: &crate::memories::Names,
    file: String,
    dry_run: bool,
    json: bool,
) -> Result<i32, CliError> {
    let synced = synced(rt, root, names, file, dry_run).await?;
    tell(&synced, json);
    Ok(synced.code())
}

pub async fn synced(
    rt: &Runtime,
    root: &Path,
    names: &crate::memories::Names,
    file: String,
    dry_run: bool,
) -> Result<Synced, CliError> {
    let declared = read_declared(root, &file)?;
    let ctx = Context {
        catalog: Catalog::load(root)?,
    };

    let mut scanned = crate::memories::scan(root, &ctx.catalog)?;
    let existing = rt.anchors().await?;
    scanned.accounted_for(
        declared
            .anchor
            .iter()
            .map(|d| d.key.as_str())
            .chain(existing.iter().map(AnchorKey::as_str)),
    );
    let notes = &scanned.notes;
    let breaking: Vec<crate::memories::Fault> = scanned
        .faults
        .iter()
        .filter(|f| f.breaks())
        .cloned()
        .collect();

    let mut steps = Vec::new();
    let mut opened = Vec::new();
    let mut drifted_criteria = Vec::new();
    let mut swapped = Vec::new();
    let mut resettled = Vec::new();

    for decl in merged(&declared, notes) {
        let key = rules::key(&decl.key)?;
        steps.push(Step::Schedule(key.clone()));
        if existing.contains(&key) {
            let view = rt.read(&key).await?;
            let facets = differs(&view.anchor, decl, &ctx)?;
            if !facets.is_empty() {
                drifted_criteria.push(format!("{} ({})", decl.key, facets.join(" · ")));
            }
            if !view.closed
                && let (Some(was), Ok(now)) = (&view.derivation, rt.instrument(&view.anchor.probe))
                && was.version != now.version
            {
                swapped.push(decl.key.clone());
            }
            let running = rt.settings_for(&key).await?;
            if let Some(next) = decl.settings.overlaid(&running) {
                steps.push(Step::Resettle(key, next));
                resettled.push(decl.key.clone());
            }
            continue;
        }
        decl.check_contract(&ctx)?;
        steps.push(Step::Open(Box::new(OpenRequest {
            key,
            probe: decl.to_probe(&ctx)?,
            transitions: decl.to_transitions()?,
            terminal: rules::terminal(&decl.terminal)?,
            initial: decl.initial(),
            settings: decl.settings.at_open(),
            supersedes: None,
        })));
        opened.push(decl.key.clone());
    }

    let (binds, bound, renamed) = align_bindings(rt, notes, names).await?;
    steps.extend(
        binds
            .into_iter()
            .map(|(reference, anchors, version, dropped)| {
                Step::Bind(reference, anchors, version, dropped)
            }),
    );

    let mut warnings = Vec::new();
    let mut scheduled = 0;
    if !dry_run {
        for step in steps {
            match step {
                Step::Schedule(key) => scheduled += usize::from(rt.ensure_scheduled(&key).await?),
                Step::Resettle(key, settings) => rt.set_settings(&key, &settings).await?,
                Step::Open(request) => {
                    let key = request.key.clone();
                    for w in rt.open(*request).await?.warnings {
                        warnings.push(format!("{key}: {w}"));
                    }
                }
                Step::Bind(reference, anchors, version, dropped) => {
                    if !dropped.is_empty() {
                        rt.revoke_on(&reference.clone().into(), &dropped, gmr::Source::Derived)
                            .await?;
                    }
                    rt.bind(
                        gmr::Binding::on(reference, anchors),
                        Some(version),
                        Default::default(),
                        gmr::Source::Derived,
                    )
                    .await?;
                }
            }
        }
    }

    Ok(Synced {
        opened,
        criteria_drifted: drifted_criteria,
        instrument_swapped: swapped,
        resettled,
        bound,
        renamed,
        warnings,
        dry_run,
        scheduled,
        broken: breaking,
    })
}

pub fn tell(s: &Synced, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(s).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        );
        return;
    }
    told(s);
}

fn told(s: &Synced) {
    println!(
        "{} anchors{}",
        s.opened.len(),
        if s.dry_run {
            " would be opened (--dry-run)"
        } else {
            " opened"
        }
    );
    for w in &s.warnings {
        println!("  ! {w}");
    }
    if !s.broken.is_empty() {
        println!(
            "\n{} notes did not become the anchor they meant to — everything else in \
             this repo synced anyway:",
            s.broken.len()
        );
        for f in &s.broken {
            println!("  ! {}", f.line());
        }
    }
    if !s.criteria_drifted.is_empty() {
        println!(
            "\n{} anchors have declarations that differ from their current criteria:",
            s.criteria_drifted.len()
        );
        for k in &s.criteria_drifted {
            println!("  != {k}");
        }
        println!(
            "\nChanging a probe or transition table is a criteria revision, not a refactor; sync will not do it for you.\n\
             Decide whether to accept it, then use revise so it leaves a sealed record."
        );
    }
    if !s.instrument_swapped.is_empty() {
        println!(
            "\n{} anchors last read with an instrument this build no longer has:",
            s.instrument_swapped.len()
        );
        for k in &s.instrument_swapped {
            println!("  ~= {k}");
        }
        println!(
            "\nThe declarations are unchanged; what moved is the rule behind the name.\n\
             Whatever those baselines are compared against next was measured differently,\n\
             and only you can say whether that still counts as the same reading.\n\
             \n    gmr rebase --all --why '...'\n\
             \nObserve will keep running either way — it just cannot tell you which of\n\
             the two things moved."
        );
    }
    if !s.bound.is_empty() {
        println!(
            "\n{} notes {} their anchors:",
            s.bound.len(),
            if s.dry_run {
                "would be bound to"
            } else {
                "bound to"
            }
        );
        for b in &s.bound {
            println!("  + {b}");
        }
    }
    if !s.renamed.is_empty() {
        println!(
            "\n{} notes dropped a key and gained an unseen one. That is either a\n\
             rename or a mistake, and sync will not guess which:",
            s.renamed.len()
        );
        for r in &s.renamed {
            println!("  ? {r}");
        }
        println!("\nClose the old anchor with a reason, or put the old key back.");
    }
    if !s.resettled.is_empty() {
        println!(
            "\n{} anchors {} a new retain/cadence from the declaration:",
            s.resettled.len(),
            if s.dry_run { "would take" } else { "took" }
        );
        for k in &s.resettled {
            println!("  ~= {k}");
        }
    }
}

struct Rename {
    dropped: Vec<AnchorKey>,
    gained: Vec<AnchorKey>,
}

impl Rename {
    fn joined(&self, keys: &[AnchorKey]) -> String {
        keys.iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn ambiguous(had: &[AnchorKey], want: &[AnchorKey], closed: &[AnchorKey]) -> Option<Rename> {
    let dropped: Vec<AnchorKey> = had
        .iter()
        .filter(|k| !want.contains(k) && !closed.contains(k))
        .cloned()
        .collect();
    let gained: Vec<AnchorKey> = want.iter().filter(|k| !had.contains(k)).cloned().collect();
    (!dropped.is_empty() && !gained.is_empty()).then_some(Rename { dropped, gained })
}

type Binding = (Ref, Vec<AnchorKey>, Version, Vec<AnchorKey>);

enum Step {
    Schedule(AnchorKey),
    Resettle(AnchorKey, RunSettings),
    Open(Box<OpenRequest>),
    Bind(Ref, Vec<AnchorKey>, Version, Vec<AnchorKey>),
}

async fn align_bindings(
    rt: &Runtime,
    notes: &[crate::memories::Note],
    names: &crate::memories::Names,
) -> Result<(Vec<Binding>, Vec<String>, Vec<String>), CliError> {
    let mut planned = Vec::new();
    let mut bound = Vec::new();
    let mut renamed = Vec::new();

    for note in notes {
        let reference = note.reference.clone();
        let named = names.of(&reference);
        let mut want: Vec<AnchorKey> = note
            .wants
            .iter()
            .map(|w| AnchorKey::new(w.key().to_owned()))
            .collect();
        want.sort();
        want.dedup();

        let current = rt.memory().binding_of(&reference.clone().into()).await?;
        let had = current.anchors().to_vec();
        let mut closed = Vec::new();
        for key in had.iter().filter(|k| !want.contains(k)) {
            if matches!(rt.read(key).await, Ok(view) if view.closed) {
                closed.push(key.clone());
            }
        }
        let looks_renamed = ambiguous(&had, &want, &closed).inspect(|rename| {
            renamed.push(format!(
                "{named}: dropped {}, gained {}",
                rename.joined(&rename.dropped),
                rename.joined(&rename.gained)
            ));
        });

        let version = rt.current_version(&reference).await?.ok_or_else(|| {
            CliError(format!(
                "no content provider could version `{named}` — nothing registered as `{}` \
                 can resolve it",
                reference.provider
            ))
        })?;
        let asking = gmr::Binding::on(reference.clone(), want.clone());
        let settled = had == want
            && current.says(
                &asking,
                Some(&version),
                &Default::default(),
                gmr::Source::Derived,
            );
        if settled {
            continue;
        }

        let dropped: Vec<AnchorKey> = match looks_renamed.is_some() {
            true => Vec::new(),
            false => had.iter().filter(|k| !want.contains(k)).cloned().collect(),
        };
        bound.push(named);
        planned.push((reference, want, version, dropped));
    }
    Ok((planned, bound, renamed))
}

pub fn differs(
    anchor: &Anchor,
    decl: &AnchorDecl,
    ctx: &Context,
) -> Result<Vec<&'static str>, CliError> {
    let mut facets = Vec::new();
    if anchor.probe != decl.to_probe(ctx)? {
        facets.push("probe");
    }
    if anchor.transitions != decl.to_transitions()? {
        facets.push("rules");
    }
    if anchor.terminal != rules::terminal(&decl.terminal)? {
        facets.push("terminal");
    }
    Ok(facets)
}

pub enum Standing<'a> {
    Matches,
    Drifted {
        decl: &'a AnchorDecl,
        facets: Vec<&'static str>,
    },
    Unreadable {
        reason: String,
    },
    Undeclared,
}

pub fn standing<'a>(
    view: &AnchorView,
    bound: bool,
    decls: &[&'a AnchorDecl],
    scanned: &crate::memories::Scanned,
    ctx: &Context,
) -> Result<Standing<'a>, CliError> {
    match decls.iter().find(|d| d.key == view.key.as_str()) {
        Some(decl) => {
            let facets = differs(&view.anchor, decl, ctx)?;
            Ok(match facets.is_empty() {
                true => Standing::Matches,
                false => Standing::Drifted { decl, facets },
            })
        }
        None => match scanned.blocked_key(view.key.as_str()) {
            Some(f) => Ok(Standing::Unreadable { reason: f.line() }),
            None if bound => Ok(Standing::Undeclared),
            None => Ok(Standing::Matches),
        },
    }
}

#[derive(Default)]
pub struct Bound(std::collections::BTreeSet<AnchorKey>);

impl Bound {
    pub async fn of(rt: &Runtime) -> Result<Self, CliError> {
        Ok(Self(
            rt.memory()
                .all()
                .await?
                .into_iter()
                .flat_map(|r| r.binding.anchors)
                .collect(),
        ))
    }

    pub fn holds(&self, key: &AnchorKey) -> bool {
        self.0.contains(key)
    }
}

#[derive(Default)]
pub struct Audit {
    pub drifted: Vec<(AnchorKey, String)>,
    pub unreadable: Vec<(AnchorKey, String)>,
    pub undeclared: Vec<AnchorKey>,
}

pub fn audit<'a>(
    views: impl IntoIterator<Item = &'a AnchorView>,
    bound: &Bound,
    decls: &[&AnchorDecl],
    scanned: &crate::memories::Scanned,
    ctx: &Context,
) -> Result<Audit, CliError> {
    let mut out = Audit::default();
    for view in views {
        if view.closed {
            continue;
        }
        match standing(view, bound.holds(&view.key), decls, scanned, ctx)? {
            Standing::Matches => {}
            Standing::Drifted { facets, .. } => {
                out.drifted.push((view.key.clone(), facets.join(" · ")))
            }
            Standing::Unreadable { reason } => out.unreadable.push((view.key.clone(), reason)),
            Standing::Undeclared => out.undeclared.push(view.key.clone()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr::{ContentProvider, ExternalId, Fetched, ProviderId};

    fn stated(key: &str, name: &str) -> AnchorDecl {
        AnchorDecl {
            key: key.to_owned(),
            probe: "ast-map".to_owned(),
            params: crate::coord::whole_repository(),
            position: Some(serde_json::json!({ "file": "src/a.ts", "name": name })),
            shape: Some("contract".to_owned()),
            rules: Vec::new(),
            terminal: Vec::new(),
            watch: None,
            settings: crate::settings::Declared::default(),
        }
    }

    #[test]
    fn a_written_declaration_is_the_one_that_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let decl = stated("src/a.ts#one", "one");

        declare(dir.path(), DEFAULT_FILE, &decl).unwrap();
        let held = read_declared(dir.path(), DEFAULT_FILE).unwrap();

        let read = &held.anchor[0];
        assert_eq!(read.key, decl.key);
        assert_eq!(read.probe, decl.probe);
        assert_eq!(read.shape, decl.shape);
        assert_eq!(
            read.position, decl.position,
            "position is what the probe is pointed at. A writer and a reader that \
             disagree about it declare an anchor watching somewhere else, and nothing \
             downstream can tell — the anchor simply reports about the wrong code"
        );
        assert_eq!(read.params, decl.params);
    }

    #[test]
    fn a_second_declaration_does_not_land_inside_the_first() {
        let dir = tempfile::tempdir().unwrap();
        declare(dir.path(), DEFAULT_FILE, &stated("src/a.ts#one", "one")).unwrap();
        declare(dir.path(), DEFAULT_FILE, &stated("src/a.ts#two", "two")).unwrap();

        let held = read_declared(dir.path(), DEFAULT_FILE).unwrap();

        assert_eq!(held.anchor.len(), 2, "both declarations must survive");
        assert_eq!(
            held.anchor[0].position,
            Some(serde_json::json!({ "file": "src/a.ts", "name": "one" })),
            "TOML puts a table under whichever array entry precedes it, so appending a \
             second entry whose tables outrank the first would silently repoint the \
             first anchor at the second's coordinate"
        );
        assert_eq!(
            held.anchor[1].position,
            Some(serde_json::json!({ "file": "src/a.ts", "name": "two" }))
        );
    }

    struct Versions(ProviderId);

    #[async_trait::async_trait]
    impl ContentProvider for Versions {
        fn provider(&self) -> &ProviderId {
            &self.0
        }

        async fn fetch(
            &self,
            _id: &ExternalId,
            _budget: &gmr::Budget,
        ) -> Result<Option<Fetched>, gmr::ContentError> {
            Ok(Some(Fetched {
                version: Version::new("v1"),
                bytes: Vec::new(),
            }))
        }
    }

    async fn runtime(dir: &std::path::Path) -> (Runtime, gmr::sqlite::SqliteStore) {
        let store = gmr::sqlite::open(dir.join("memory.db")).await.unwrap();
        let rt = Runtime::builder()
            .journal(std::sync::Arc::new(store.journal()))
            .bindings(std::sync::Arc::new(store.bindings()))
            .sealer(std::sync::Arc::new(store.sealer()))
            .links(std::sync::Arc::new(store.links()))
            .queue(std::sync::Arc::new(store.queue()))
            .settings(std::sync::Arc::new(store.settings()))
            .sightings(std::sync::Arc::new(store.sightings()))
            .provider(std::sync::Arc::new(Versions(ProviderId::new("git"))))
            .build();
        (rt, store)
    }

    #[tokio::test]
    async fn resolving_a_binding_does_not_write_it() {
        let dir = tempfile::tempdir().unwrap();
        let (rt, store) = runtime(dir.path()).await;
        let reference = Ref::new("git", "memories/a.md");
        let notes = vec![crate::memories::Note {
            reference: reference.clone(),
            wants: vec![crate::memories::Want::Existing("some::key".to_owned())],
            watch: None,
        }];

        let names = crate::memories::Names::over(vec![std::sync::Arc::new(
            crate::memories::declaring(dir.path()),
        )]);
        let (plan, bound, _) = align_bindings(&rt, &notes, &names).await.unwrap();

        assert_eq!(plan.len(), 1);
        assert_eq!(
            bound,
            vec!["a".to_owned()],
            "sync names a note the way every other verb does — its name, not the address \
             some store keeps it at. Two verbs spelling the same note differently is how a \
             reader learns to distrust both"
        );
        assert!(
            rt.memory()
                .binding_of(&reference.clone().into())
                .await
                .unwrap()
                .is_empty(),
            "resolution has to finish before the first write, because the journal is \
             append-only and there is no rollback. Writing as it went is how a sync that \
             failed on the last note left 346 anchors open with nothing bound to them — a \
             state `check` and `doctor` both read as perfectly fine"
        );

        for (reference, anchors, version, _) in plan {
            rt.bind(
                gmr::Binding::on(reference, anchors),
                Some(version),
                Default::default(),
                gmr::Source::Derived,
            )
            .await
            .unwrap();
        }
        assert!(
            !rt.memory()
                .binding_of(&reference.clone().into())
                .await
                .unwrap()
                .is_empty(),
            "and the plan really does bind when applied — without this the assertion above \
             would pass just as well against a fixture that could never bind at all"
        );
        store.close().await;
    }

    #[tokio::test]
    async fn a_binding_recorded_before_its_origin_was_known_is_re_derived_once() {
        let dir = tempfile::tempdir().unwrap();
        let (rt, store) = runtime(dir.path()).await;
        let reference = Ref::new("git", "memories/a.md");
        let notes = vec![crate::memories::Note {
            reference: reference.clone(),
            wants: vec![crate::memories::Want::Existing("some::key".to_owned())],
            watch: None,
        }];
        let names = crate::memories::Names::over(vec![std::sync::Arc::new(
            crate::memories::declaring(dir.path()),
        )]);

        rt.bind(
            gmr::Binding::on(reference.clone(), keys(&["some::key"])),
            Some(Version::new("v1")),
            Default::default(),
            gmr::Source::Unknown,
        )
        .await
        .unwrap();

        let (plan, _, _) = align_bindings(&rt, &notes, &names).await.unwrap();
        assert_eq!(
            plan.len(),
            1,
            "a row saying nothing about where it came from is not a derivation, so sync \
             owes the record one"
        );
        for (reference, anchors, version, _) in plan {
            rt.bind(
                gmr::Binding::on(reference, anchors),
                Some(version),
                Default::default(),
                gmr::Source::Derived,
            )
            .await
            .unwrap();
        }

        let (plan, _, _) = align_bindings(&rt, &notes, &names).await.unwrap();
        assert!(
            plan.is_empty(),
            "and exactly one. The row it was owed is now on the record; the older one stays \
             `unknown` because the table is append-only and its origin really is unknown. \
             Asking instead whether every assertion ever made was derived is a question about \
             an immutable past, so it can never come back true, and every sync over an \
             unchanged repository re-asserts the whole corpus forever"
        );
        store.close().await;
    }

    fn keys(names: &[&str]) -> Vec<AnchorKey> {
        names.iter().map(|n| AnchorKey::new(*n)).collect()
    }

    #[test]
    fn dropping_one_key_while_gaining_another_is_ambiguous() {
        assert!(
            ambiguous(&keys(&["old"]), &keys(&["new"]), &[]).is_some(),
            "a rename and a mistake look identical from here, so sync must not guess"
        );
    }

    #[test]
    fn closing_the_dropped_anchor_is_what_resolves_it() {
        assert!(
            ambiguous(&keys(&["old"]), &keys(&["new"]), &keys(&["old"])).is_none(),
            "sync tells the reader to close the old anchor with a reason. Closing left the \
             binding record untouched, so the same refusal came back every run and the \
             instruction could never be carried out"
        );
    }

    #[test]
    fn one_closed_drop_does_not_excuse_a_live_one() {
        let rename = ambiguous(
            &keys(&["closed", "live"]),
            &keys(&["new"]),
            &keys(&["closed"]),
        )
        .expect("the live drop is still an unanswered question");
        assert_eq!(rename.dropped, keys(&["live"]));
    }

    #[test]
    fn gaining_a_key_without_dropping_one_is_never_ambiguous() {
        assert!(ambiguous(&keys(&["a"]), &keys(&["a", "b"]), &[]).is_none());
    }

    #[test]
    fn dropping_a_key_without_gaining_one_is_never_ambiguous() {
        assert!(ambiguous(&keys(&["a", "b"]), &keys(&["a"]), &[]).is_none());
    }

    const ART: &str = "probe = \"ast-map\"";

    const RULES: &str = r#"
rules = [
  'obs.exact == false => { position: state.position, n: 0, status: "coordinate-missed" }',
  'not exists(state.n) => { position: state.position, n: obs.candidates, status: "counted" }',
  'obs.candidates != state.n => { position: state.position, n: obs.candidates, status: "recounted" }',
  'true => { position: state.position, n: state.n, status: "settled" }',
]
"#;

    fn decl(body: &str) -> AnchorDecl {
        let text = format!("[[anchor]]\nkey = \"k\"\n{body}");
        toml::from_str::<Declared>(&text)
            .unwrap()
            .anchor
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn hand_written_rules_and_a_named_shape_both_become_transitions() {
        assert!(
            decl(&format!("{ART}\nshape = \"roster\""))
                .to_transitions()
                .is_ok()
        );
        assert!(decl(&format!("{ART}{RULES}")).to_transitions().is_ok());
        assert_ne!(
            decl(&format!("{ART}\nshape = \"roster\""))
                .to_transitions()
                .unwrap(),
            decl(&format!("{ART}{RULES}")).to_transitions().unwrap()
        );
    }

    #[test]
    fn shape_and_rules_together_are_refused() {
        let e = decl(&format!("{ART}\nshape = \"roster\"{RULES}"))
            .to_transitions()
            .unwrap_err();
        assert!(e.to_string().contains("not both"), "{e}");
    }

    #[test]
    fn an_unknown_shape_names_the_anchor_that_asked_for_it() {
        let e = decl(&format!("{ART}\nshape = \"nope\""))
            .to_transitions()
            .unwrap_err();
        assert!(e.to_string().contains('k'), "{e}");
    }

    fn ctx(toml_body: &str) -> (tempfile::TempDir, Context) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        std::fs::write(dir.path().join(".anchor/probes.toml"), toml_body).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/probe.sh"), "echo '{}'").unwrap();
        let catalog = Catalog::load(dir.path()).unwrap();
        (dir, Context { catalog })
    }

    const AST_LIKE: &str = r#"
[probe.ast-like]
stage = { probe = "src/probe.sh" }
entrypoint = "probe"
sources = ["src"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "name"], facts = ["body", "line"] }
"#;

    #[test]
    fn the_declaration_carries_the_name_verbatim() {
        let (_d, c) = ctx(AST_LIKE);
        let probe = decl("probe = \"ast-like\"\nshape = \"roster\"")
            .to_probe(&c)
            .unwrap();
        assert_eq!(probe.name.as_str(), "ast-like");
    }

    #[test]
    fn a_version_where_a_name_belongs_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl(&format!("probe = \"{}\"", "d9".repeat(32)))
            .to_probe(&c)
            .unwrap_err();
        assert!(e.to_string().contains("probe name"), "{e}");
    }

    #[test]
    fn a_shape_the_probe_cannot_feed_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl("probe = \"ast-like\"\nshape = \"fingerprint\"")
            .check_contract(&c)
            .unwrap_err();
        assert!(e.to_string().contains("at.fingerprint"), "{e}");
    }

    #[test]
    fn roster_rides_the_same_probe_happily() {
        let (_d, c) = ctx(AST_LIKE);
        decl("probe = \"ast-like\"\nshape = \"roster\"")
            .check_contract(&c)
            .unwrap();
    }

    #[test]
    fn hand_written_rules_get_the_same_check_a_shape_gets() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl(
            "probe = \"ast-like\"\nrules = ['obs.facts.occurrences > 0 => { status: \"x\" }']",
        )
        .check_contract(&c)
        .unwrap_err();
        assert!(e.to_string().contains("facts.occurrences"), "{e}");
    }

    #[test]
    fn a_syntax_error_in_a_rule_is_caught_at_sync_time_not_observe_time() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl("probe = \"ast-like\"\nrules = ['obs.facts. => { status: \"x\" }']")
            .check_contract(&c)
            .unwrap_err();
        assert!(e.to_string().starts_with("k:"), "{e}");
    }

    #[test]
    fn a_knob_the_toml_does_not_name_arrives_unsaid_rather_than_defaulted() {
        let declared: Declared = toml::from_str(
            r#"
[[anchor]]
key = "a"
probe = "ast-map"
cadence_secs = 60
"#,
        )
        .unwrap();
        let said = declared.anchor[0].settings;
        assert_eq!(said.cadence_secs, Some(60));
        assert_eq!(
            (said.retain_full, said.budget_ms),
            (None, None),
            "`#[serde(flatten)]` is what carries the three knobs through both TOML and a \
             note's YAML, and it is the one step that could quietly turn `absent` back into \
             `false`/`None` — which is the whole difference between overlaying a declaration \
             and replacing the settings with it"
        );
        assert_eq!(
            said.at_open(),
            gmr::RunSettings {
                cadence_secs: Some(60),
                ..Default::default()
            }
        );
    }
}
